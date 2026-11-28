//! `follow` after a refused start (`story:skeleton-node-run-assignment`, the coordinator's answer to
//! the adversary's findings): a start the place refused with `retriable: true` is started again
//! in a later pass of the same run, at most [`ATTEMPTS`] times; a start refused `Unavailable` (it
//! may have started) or not retriable is not; a refusal it does not start again is a report line
//! with its code; a process refused for good closes its instance's
//! start, so the processes beside it run and the instance is not recorded `Running`. Also: an
//! instance in `Starting` that this run never verified is not started (a restart starts nothing
//! twice), and a `Service` replica runs under the longest limits the exec port accepts.
//!
//! Each case runs [`Follower`] on a thread of its own against a seeded `ControlHost`, with a stub
//! `exec` port that answers by the process's snippet. The followers loop until the test process
//! ends.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use weft_agent::follow::{Follower, PASS, RETRY_AFTER};
use weft_agent::report::Report;
use weft_client::{Client, Expect};
use weft_platform::node::{Exec, ExecError, ExecState, Exited, Limits, Process, RefusalClass};
use weft_platform::Refusal;
use weft_testkit::ControlHost;
use weft_types::shared::ServiceId;
use weft_types::workloads::InstanceState;

use crate::seeds::{self, Seed};

/// How many times the follower starts one process, the first time included.
const ATTEMPTS: usize = 5;

/// How long a case waits for the follower.
const LIMIT: Duration = Duration::from_secs(20);

/// Refused for good: not retriable.
const REFUSE: &str = "refuse";

/// Refused retriable (`exec.sandbox-unavailable`) the first time, run after.
const BUSY_ONCE: &str = "busy-once";

/// Refused `Unavailable` the first time; run if it is ever asked again, which it must not be.
const GONE_ONCE: &str = "gone-once";

/// Refused retriable every time.
const BUSY_ALWAYS: &str = "busy-always";

/// A stub `exec` port: each call is recorded with its snippet (the last word of `sh -c
/// <snippet>`) and its limits, and answered as the snippet's name says; any other snippet runs
/// to `exited` 0 with the snippet as its output.
struct Stub {
    calls: Mutex<Vec<(String, Limits)>>,
}

impl Stub {
    fn leaked() -> &'static Self {
        Box::leak(Box::new(Self {
            calls: Mutex::new(Vec::new()),
        }))
    }

    /// The calls with `snippet`.
    fn calls(&self, snippet: &str) -> usize {
        self.all().iter().filter(|(of, _)| of == snippet).count()
    }

    fn all(&self) -> Vec<(String, Limits)> {
        self.calls.lock().expect("the calls").clone()
    }
}

impl Exec for Stub {
    fn run(
        &self,
        process: &Process,
        on_state: &mut dyn FnMut(ExecState),
    ) -> Result<Exited, ExecError> {
        let snippet = process.argv.last().cloned().unwrap_or_default();
        let call = {
            let mut calls = self.calls.lock().expect("the calls");
            calls.push((snippet.clone(), process.limits));
            calls.iter().filter(|(of, _)| *of == snippet).count()
        };
        let busy = || ExecError::Refused {
            class: RefusalClass::Exhausted,
            code: "exec.sandbox-unavailable".to_owned(),
            message: "the stub place cannot make a sandbox now".to_owned(),
            retriable: true,
        };
        match snippet.as_str() {
            REFUSE => {
                return Err(ExecError::Refused {
                    class: RefusalClass::Refused,
                    code: "exec.denied".to_owned(),
                    message: "the stub place does not run this".to_owned(),
                    retriable: false,
                })
            }
            BUSY_ALWAYS => return Err(busy()),
            BUSY_ONCE if call == 1 => return Err(busy()),
            GONE_ONCE if call == 1 => {
                return Err(ExecError::Shared(Refusal::Unavailable {
                    detail: "the stub place did not answer".to_owned(),
                }))
            }
            _ => {}
        }
        on_state(ExecState::Accepted);
        on_state(ExecState::Running);
        on_state(ExecState::Exited);
        Ok(Exited {
            code: Some(0),
            stdout: format!("{snippet}\n").into_bytes(),
            stderr: Vec::new(),
            truncated: false,
        })
    }
}

/// A report file of this test process named `name`, absent.
fn report_path(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "follow-bookkeeping-{name}-{}.jsonl",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

/// Seeds `seed`, runs a follower of this node on a thread of its own with `stub` at the default
/// pacing, writing to the report named `name`: the control host and the report's path.
fn follow(seed: &Seed, stub: &'static Stub, name: &str) -> (ControlHost, PathBuf) {
    follow_paced(seed, stub, name, PASS, RETRY_AFTER)
}

/// [`follow`] with `pass` between two passes and `retry_after` passes before a refused start is
/// made again.
fn follow_paced(
    seed: &Seed,
    stub: &'static Stub,
    name: &str,
    pass: Duration,
    retry_after: u64,
) -> (ControlHost, PathBuf) {
    let host = ControlHost::start_seeded(seed.rows.clone());
    let report = report_path(name);
    let client: &'static Client = Box::leak(Box::new(Client::new(host.address())));
    let node_id: &'static str = Box::leak(seed.this.node_id.clone().into_boxed_str());
    let open: &'static Mutex<Report> = Box::leak(Box::new(Mutex::new(
        Report::open(&report).expect("the report file opens"),
    )));
    std::thread::spawn(move || {
        Follower::new(client, node_id, stub, open)
            .pacing(pass, retry_after)
            .follow()
    });
    (host, report)
}

/// The complete lines of the report file at `path`.
fn lines(path: &Path) -> Vec<Value> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let complete = text.rfind('\n').map_or("", |end| &text[..end]);
    complete
        .lines()
        .map(|line| serde_json::from_str(line).expect("a report line"))
        .collect()
}

/// Waits until the report at `path` satisfies `done`.
fn wait(path: &Path, what: &str, done: impl Fn(&[Value]) -> bool) -> Vec<Value> {
    let start = Instant::now();
    loop {
        let lines = lines(path);
        if done(&lines) {
            return lines;
        }
        assert!(start.elapsed() < LIMIT, "{what} after {LIMIT:?}: {lines:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Waits until the report at `path` holds a `pass` line numbered `pass` or higher.
fn wait_for_pass(path: &Path, pass: u64) -> Vec<Value> {
    wait(path, &format!("no pass {pass}"), |lines| {
        lines
            .iter()
            .filter(|line| line.get("started").is_none())
            .filter_map(|line| line["pass"].as_u64())
            .any(|number| number >= pass)
    })
}

/// The `state` of `instance_id` in `weft.workloads.Instances`.
fn state(host: &ControlHost, instance_id: &str) -> Value {
    let answer = host
        .client()
        .view("weft.workloads.Instances", &[])
        .ok_body(200);
    answer["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["instance_id"] == json!(instance_id))
        .unwrap_or_else(|| panic!("no instance {instance_id}: {answer}"))["state"]
        .clone()
}

/// The request lines of `operation` in the control host's log.
fn requests(host: &ControlHost, operation: &str) -> Vec<Value> {
    host.log_after(0)
        .into_iter()
        .filter(|line| line["kind"] == "request" && line["operation"] == operation)
        .collect()
}

/// The refusal lines of the report.
fn refusals(lines: &[Value]) -> Vec<&Value> {
    lines
        .iter()
        .filter(|line| line.get("refusal").is_some())
        .collect()
}

/// This node's instance with one process per snippet.
fn with_processes(snippets: &[&str]) -> Seed {
    seeds::two_nodes_changed(|instance| {
        instance.data.processes = snippets
            .iter()
            .enumerate()
            .map(|(index, snippet)| seeds::process(&format!("p{index}"), snippet))
            .collect();
    })
}

/// A process refused for good (not retriable) and one refused `Unavailable` (the place did not
/// answer, so it may have started the process) are each started once, and each refusal is one
/// report line with the instance and the refusal's code. The process beside them runs and its
/// exit is reported. The refusals close the instance's start: the instance is not recorded
/// `Running` (two of its processes never ran; recording the failure is `RecordInstanceFailed`,
/// outside this epic), and nothing waits for the refused processes: two passes past their retry
/// interval neither is called again.
#[test]
fn a_process_refused_for_good_is_reported_once_and_its_sibling_runs() {
    let seed = with_processes(&["echo weft", REFUSE, GONE_ONCE]);
    let this = seed.this.instance_id.clone();
    let stub = Stub::leaked();
    let (host, report) = follow(&seed, stub, "refused-for-good");

    let lines = wait_for_pass(&report, 1 + RETRY_AFTER + 2);
    assert!(
        lines.contains(&json!({"instance_id": this, "exit_code": 0, "stdout": "echo weft\n"})),
        "the process beside the refused ones did not run: {lines:?}"
    );
    let mut refused = refusals(&lines);
    refused.sort_by_key(|line| line["refusal"].to_string());
    assert_eq!(
        refused,
        vec![
            &json!({"instance_id": this, "refusal": "exec.denied"}),
            &json!({"instance_id": this, "refusal": "unavailable"}),
        ],
        "{lines:?}"
    );
    assert_eq!(
        (stub.calls(REFUSE), stub.calls(GONE_ONCE)),
        (1, 1),
        "{:?}",
        stub.all()
    );
    assert_eq!(state(&host, &this), json!("Starting"));
    assert_eq!(
        requests(&host, "weft.workloads.RecordInstanceRunning"),
        Vec::<Value>::new()
    );
}

/// A start refused retriable (`exec.sandbox-unavailable`) is started again in a later pass of the
/// same run; once it ran, beside a process that ran the first time, the instance is recorded
/// `Running` once, and no refusal is reported.
#[test]
fn a_process_refused_retriable_runs_in_a_later_pass_and_the_instance_runs() {
    let seed = with_processes(&["echo weft", BUSY_ONCE]);
    let this = seed.this.instance_id.clone();
    let stub = Stub::leaked();
    let (host, report) = follow(&seed, stub, "refused-retriable");

    let start = Instant::now();
    while state(&host, &this) != json!("Running") {
        assert!(
            start.elapsed() < LIMIT,
            "not Running after {LIMIT:?}: {:?}; {:?}",
            lines(&report),
            stub.all()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let lines = lines(&report);
    assert_eq!(
        (stub.calls("echo weft"), stub.calls(BUSY_ONCE)),
        (1, 2),
        "{:?}",
        stub.all()
    );
    assert_eq!(refusals(&lines), Vec::<&Value>::new(), "{lines:?}");
    let running = requests(&host, "weft.workloads.RecordInstanceRunning");
    assert_eq!(running.len(), 1, "{running:?}");
}

/// A start refused retriable every time is started [`ATTEMPTS`] times in all, then given up: one
/// refusal line with its code, and no further call over the passes after. The follower runs a
/// pass every 50 ms (the default is [`PASS`]) with the default [`RETRY_AFTER`], so the case counts
/// the same passes in a tenth of the time.
#[test]
fn a_start_refused_every_time_is_given_up_after_its_attempts() {
    let seed = with_processes(&[BUSY_ALWAYS]);
    let this = seed.this.instance_id.clone();
    let stub = Stub::leaked();
    let (host, report) = follow_paced(
        &seed,
        stub,
        "refused-always",
        Duration::from_millis(50),
        RETRY_AFTER,
    );

    let lines = wait(&report, "no refusal line", |lines| {
        !refusals(lines).is_empty()
    });
    assert_eq!(stub.calls(BUSY_ALWAYS), ATTEMPTS, "{:?}", stub.all());
    let given_up_in = lines
        .iter()
        .filter(|line| line.get("started").is_none())
        .filter_map(|line| line["pass"].as_u64())
        .max()
        .unwrap_or(0);
    let lines = wait_for_pass(&report, given_up_in + 6);
    assert_eq!(stub.calls(BUSY_ALWAYS), ATTEMPTS, "{:?}", stub.all());
    assert_eq!(
        refusals(&lines),
        vec![&json!({"instance_id": this, "refusal": "exec.sandbox-unavailable"})],
        "{lines:?}"
    );
    assert_eq!(state(&host, &this), json!("Starting"));
}

/// An instance that reads `Starting` when the follower first sees it was moved there by another
/// run of the agent (a restart) or another node agent: this run sent no `RecordArtifactVerified`
/// for it, so it starts nothing and sends nothing for it.
#[test]
fn an_instance_in_starting_this_run_never_verified_is_not_started() {
    let seed = seeds::two_nodes_changed(|instance| instance.state = InstanceState::Starting);
    let this = seed.this.instance_id.clone();
    let stub = Stub::leaked();
    let (host, report) = follow(&seed, stub, "starting-unseen");

    let lines = wait_for_pass(&report, 4);
    assert_eq!(stub.all(), Vec::new(), "{lines:?}");
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.get("started").is_some())
            .count(),
        0,
        "{lines:?}"
    );
    for operation in [
        "weft.workloads.RecordArtifactVerified",
        "weft.workloads.RecordInstanceRunning",
    ] {
        assert_eq!(
            requests(&host, operation),
            Vec::<Value>::new(),
            "{operation}"
        );
    }
    assert_eq!(state(&host, &this), json!("Starting"));
}

/// The limits of a process come from its instance: a `Service` replica (its instance names a
/// `service_id`) runs under the longest `timeout` and `cpu_time` the exec port accepts; an
/// instance of no service under the exec port's default limits.
#[test]
fn a_service_replica_runs_under_the_longest_limits_and_other_instances_under_the_default() {
    let service: ServiceId = seeds::service_id();
    let replica = seeds::two_nodes_changed(|instance| {
        instance.data.service_id = Some(service);
        instance.data.replica_index = Some(0);
    });
    let plain = seeds::two_nodes();
    let replica_stub = Stub::leaked();
    let plain_stub = Stub::leaked();
    let (_replica_host, replica_report) = follow(&replica, replica_stub, "limits-replica");
    let (_plain_host, plain_report) = follow(&plain, plain_stub, "limits-plain");

    for (report, this) in [
        (&replica_report, &replica.this.instance_id),
        (&plain_report, &plain.this.instance_id),
    ] {
        wait(report, "no exit line", |lines| {
            lines
                .iter()
                .any(|line| line["instance_id"] == json!(this) && line.get("exit_code").is_some())
        });
    }
    assert_eq!(
        replica_stub.all(),
        vec![(
            seeds::THIS_SNIPPET.to_owned(),
            Limits {
                timeout: Limits::LONGEST,
                cpu_time: Limits::LONGEST,
                ..Limits::default()
            }
        )]
    );
    assert_eq!(
        plain_stub.all(),
        vec![(seeds::THIS_SNIPPET.to_owned(), Limits::default())]
    );
}
