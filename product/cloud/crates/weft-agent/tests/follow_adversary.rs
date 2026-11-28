//! Adversary cases of `follow` (`story:skeleton-node-run-assignment`): the follower's own promise
//! in `src/follow.rs` that "a work list or a command the control host did not answer is tried
//! again in the next pass", and its rule that an instance in `Starting` "was started", driven
//! through a lost answer, a lost request and a refused exec.
//!
//! Each case runs [`Follower`] on a thread of its own against a seeded `ControlHost`, with a stub
//! `exec` port; a relay between the follower and the control host loses one exchange where a case
//! says so. The followers loop until the test process ends.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use weft_agent::follow::Follower;
use weft_agent::report::Report;
use weft_client::{Client, Expect};
use weft_platform::node::{Exec, ExecError, ExecState, Exited, Process, RefusalClass};
use weft_testkit::ControlHost;

use crate::seeds;

/// How long a case waits for the follower.
const LIMIT: Duration = Duration::from_secs(20);

/// A stub `exec` port: counts its calls; refuses the first `refuse` of them as the place would
/// refuse a sandbox it cannot make now (retriable), and runs every later one to `exited` 0.
struct Stub {
    calls: AtomicUsize,
    refuse: usize,
}

impl Stub {
    fn leaked(refuse: usize) -> &'static Self {
        Box::leak(Box::new(Self {
            calls: AtomicUsize::new(0),
            refuse,
        }))
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Exec for Stub {
    fn run(
        &self,
        _process: &Process,
        on_state: &mut dyn FnMut(ExecState),
    ) -> Result<Exited, ExecError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call < self.refuse {
            return Err(ExecError::Refused {
                class: RefusalClass::Exhausted,
                code: "exec.sandbox-unavailable".to_owned(),
                message: "the stub place cannot make a sandbox now".to_owned(),
                retriable: true,
            });
        }
        on_state(ExecState::Accepted);
        on_state(ExecState::Running);
        on_state(ExecState::Exited);
        Ok(Exited {
            code: Some(0),
            stdout: b"stub\n".to_vec(),
            stderr: Vec::new(),
            truncated: false,
        })
    }
}

/// A report file of this test process named `name`, absent.
fn report_path(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "follow-adversary-{name}-{}.jsonl",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

/// Runs a follower of `node_id` on a thread of its own, through `control`, with `exec`, writing
/// to `report`. It runs until the test process ends.
fn spawn_follower(control: SocketAddr, node_id: &str, exec: &'static Stub, report: &Path) {
    let client: &'static Client = Box::leak(Box::new(Client::new(control)));
    let node_id: &'static str = Box::leak(node_id.to_owned().into_boxed_str());
    let report: &'static Mutex<Report> = Box::leak(Box::new(Mutex::new(
        Report::open(report).expect("the report file opens"),
    )));
    std::thread::spawn(move || Follower::new(client, node_id, exec, report).follow());
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

/// The highest number of a `{"pass": n}` line.
fn last_pass(lines: &[Value]) -> u64 {
    lines
        .iter()
        .filter(|line| line.get("started").is_none())
        .filter_map(|line| line["pass"].as_u64())
        .max()
        .unwrap_or(0)
}

/// Waits until the report at `path` holds a `pass` line numbered `pass` or higher.
fn wait_for_pass(path: &Path, pass: u64) -> Vec<Value> {
    let start = Instant::now();
    loop {
        let lines = lines(path);
        if last_pass(&lines) >= pass {
            return lines;
        }
        assert!(
            start.elapsed() < LIMIT,
            "no pass {pass} after {LIMIT:?}: {lines:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
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

/// What the relay loses of the one exchange it loses.
#[derive(Debug, Clone, Copy)]
enum Loss {
    /// The request reaches the control host; its answer is not passed back.
    Answer,
    /// The request never reaches the control host.
    Request,
}

/// A relay to `upstream` that loses the first exchange whose request line names `target` as
/// `loss` says, and passes every other exchange on. The connection of a lost exchange is closed
/// without an answer, as a control host that went away, or a timeout, leaves it.
struct Relay {
    address: SocketAddr,
    lost: Arc<AtomicUsize>,
}

impl Relay {
    fn start(upstream: SocketAddr, target: &'static str, loss: Loss) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("the relay binds");
        let address = listener.local_addr().expect("the relay's address");
        let lost = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&lost);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let counted = Arc::clone(&counted);
                std::thread::spawn(move || {
                    let _ = relay(stream, upstream, target, loss, &counted);
                });
            }
        });
        Self { address, lost }
    }

    fn lost(&self) -> usize {
        self.lost.load(Ordering::SeqCst)
    }
}

fn relay(
    mut client: TcpStream,
    upstream: SocketAddr,
    target: &str,
    loss: Loss,
    lost: &AtomicUsize,
) -> std::io::Result<()> {
    let request = message(&mut client)?;
    let first = request
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or_default();
    let names = String::from_utf8_lossy(first).contains(target);
    let hit = names
        && lost
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok();
    if hit && matches!(loss, Loss::Request) {
        return Ok(());
    }
    let mut up = TcpStream::connect(upstream)?;
    up.write_all(&request)?;
    up.flush()?;
    let answer = message(&mut up)?;
    if hit {
        return Ok(());
    }
    client.write_all(&answer)?;
    client.flush()
}

/// One HTTP message read from `stream`: its head and its `Content-Length` body.
fn message(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    stream.set_read_timeout(Some(LIMIT))?;
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        if let Some(end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buffer[..end]).to_ascii_lowercase();
            let length = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if buffer.len() >= end + 4 + length {
                buffer.truncate(end + 4 + length);
                return Ok(buffer);
            }
        }
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Ok(buffer);
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
}

/// The follower's promise: "a command the control host did not answer is tried again in the next
/// pass". `RecordArtifactVerified` took effect (the instance reads `Starting`) and its answer
/// was lost on the way back. The next pass reads the instance in `Starting`, which the follower
/// takes as "was started", so its process is never run, in this run or after a restart.
#[test]
fn a_lost_answer_to_record_artifact_verified_still_starts_the_process() {
    let seed = seeds::two_nodes();
    let host = ControlHost::start_seeded(seed.rows.clone());
    let this = seed.this.instance_id.clone();
    let relay = Relay::start(
        host.address(),
        "/weft/commands/RecordArtifactVerified ",
        Loss::Answer,
    );
    let stub = Stub::leaked(0);
    let report = report_path("lost-verify-answer");
    spawn_follower(relay.address, &seed.this.node_id, stub, &report);

    let lines = wait_for_pass(&report, 6);
    assert_eq!(relay.lost(), 1, "the relay lost no answer: {lines:?}");
    let state = state(&host, &this);
    assert_eq!(
        stub.calls(),
        1,
        "RecordArtifactVerified took effect (the instance reads {state}), its answer was lost, \
         and after 5 more passes the follower never ran the instance's process: {lines:?}"
    );
}

/// The same promise for `RecordInstanceRunning`: its request is lost once, the process ran and
/// exited 0, and the follower never sends it again, so the instance reads `Starting` for good.
#[test]
fn a_lost_record_instance_running_is_sent_again() {
    let seed = seeds::two_nodes();
    let host = ControlHost::start_seeded(seed.rows.clone());
    let this = seed.this.instance_id.clone();
    let relay = Relay::start(
        host.address(),
        "/weft/commands/RecordInstanceRunning ",
        Loss::Request,
    );
    let stub = Stub::leaked(0);
    let report = report_path("lost-running-request");
    spawn_follower(relay.address, &seed.this.node_id, stub, &report);

    let lines = wait_for_pass(&report, 6);
    assert_eq!(relay.lost(), 1, "the relay lost no request: {lines:?}");
    assert!(
        lines.contains(&json!({"instance_id": this, "exit_code": 0, "stdout": "stub\n"})),
        "the process did not run: {lines:?}"
    );
    assert_eq!(
        state(&host, &this),
        json!("Running"),
        "the process ran and exited 0, one RecordInstanceRunning was lost, and 5 passes later \
         it was never sent again: {lines:?}"
    );
}

/// A start the `exec` port refused, retriable (`exec.sandbox-unavailable`): the instance was moved
/// to `Starting` by `RecordArtifactVerified` and nothing ran. Neither the same run nor a new run
/// of the agent (a restart: a second follower with an empty `taken` set) starts it again, and
/// nothing records the failure: the instance reads `Starting` with no process, for good.
#[test]
fn a_refused_exec_is_started_again_by_the_run_or_after_a_restart() {
    let seed = seeds::two_nodes();
    let host = ControlHost::start_seeded(seed.rows.clone());
    let this = seed.this.instance_id.clone();
    let stub = Stub::leaked(1);

    let first = report_path("refused-exec-first");
    spawn_follower(host.address(), &seed.this.node_id, stub, &first);
    let first_lines = wait_for_pass(&first, 4);
    assert_eq!(
        stub.calls(),
        1,
        "the first run made no exec: {first_lines:?}"
    );

    let second = report_path("refused-exec-restart");
    spawn_follower(host.address(), &seed.this.node_id, stub, &second);
    let second_lines = wait_for_pass(&second, 4);

    let state = state(&host, &this);
    assert!(
        stub.calls() >= 2 && state == json!("Running"),
        "the exec port refused the start once (retriable); after 4 more passes of that run and \
         4 passes of a new run the port was called {} time(s) and the instance reads {state}. \
         first run: {first_lines:?}; new run: {second_lines:?}",
        stub.calls()
    );
}

/// The refusals of the two NodeAgent commands this story implemented, which no other case of the
/// suite sends (the happy path takes only `starting` and `running-ready`): each refusal answers
/// its declared outcome and moves nothing. It is the case that catches a dropped or reordered
/// guard of `record_artifact_verified.rs` or `record_instance_running.rs`.
#[test]
fn the_guards_of_record_artifact_verified_and_record_instance_running_refuse_as_declared() {
    let seed = seeds::two_nodes();
    let host = ControlHost::start_seeded(seed.rows.clone());
    let client = host.client();
    let this = &seed.this;
    let other = &seed.other;
    let digest = |instance_id: &str| {
        let answer = client.view("weft.workloads.Instances", &[]).ok_body(200);
        answer["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .find(|row| row["instance_id"] == json!(instance_id))
            .expect("the instance")["artifact_digest"]
            .clone()
    };
    let this_digest = digest(&this.instance_id);
    let other_digest = digest(&other.instance_id);
    let send = |command: &str, input: Value| -> String {
        let answer = client
            .command(command, weft_agent::agent::NODE_AGENT, json!({}), input)
            .expect("the control host answers");
        answer.body["outcome"]
            .as_str()
            .unwrap_or_else(|| panic!("{command}: no outcome in {} {}", answer.status, answer.body))
            .to_owned()
    };
    let verify = |instance_id: &str, node_id: &str, digest: &Value| {
        send(
            "weft.workloads.RecordArtifactVerified",
            json!({"instance_id": instance_id, "node_id": node_id, "artifact_digest": digest}),
        )
    };
    let running = |instance_id: &str, node_id: &str| {
        send(
            "weft.workloads.RecordInstanceRunning",
            json!({"instance_id": instance_id, "node_id": node_id}),
        )
    };

    assert_eq!(
        verify(&this.instance_id, &other.node_id, &this_digest),
        "wrong-node"
    );
    assert_eq!(
        verify(&this.instance_id, &this.node_id, &other_digest),
        "digest-mismatch"
    );
    assert_eq!(
        running(&this.instance_id, &this.node_id),
        "wrong-state",
        "RecordInstanceRunning of a Scheduled instance"
    );
    assert_eq!(state(&host, &this.instance_id), json!("Scheduled"));

    assert_eq!(
        verify(&this.instance_id, &this.node_id, &this_digest),
        "starting"
    );
    assert_eq!(
        verify(&this.instance_id, &this.node_id, &this_digest),
        "wrong-state",
        "a second RecordArtifactVerified"
    );
    assert_eq!(running(&this.instance_id, &other.node_id), "wrong-node");
    assert_eq!(state(&host, &this.instance_id), json!("Starting"));
    assert_eq!(running(&this.instance_id, &this.node_id), "running-ready");
    assert_eq!(
        running(&this.instance_id, &this.node_id),
        "wrong-state",
        "a second RecordInstanceRunning"
    );
    assert_eq!(state(&host, &this.instance_id), json!("Running"));
    assert_eq!(state(&host, &other.instance_id), json!("Scheduled"));
}

/// The request lines of `operation` in the control host's log.
fn requests(host: &ControlHost, operation: &str) -> Vec<Value> {
    host.log_after(0)
        .into_iter()
        .filter(|line| line["kind"] == "request" && line["operation"] == operation)
        .collect()
}

/// The seed of two nodes with this node's instance of restart mode `Never`
/// (`story:cloudspec-submission-job-exit`).
fn never() -> seeds::Seed {
    seeds::two_nodes_changed(|instance| {
        instance.data.restart_policy.mode = weft_types::workloads::RestartMode::Never;
    })
}

/// `RecordInstanceExited` is sent again in the next pass until it is answered
/// (`story:cloudspec-submission-job-exit`): its first request is lost, and the instance of restart
/// mode `Never` whose process exited 0 still reads `Exited`, from the one request that reached
/// the control host.
#[test]
fn a_lost_record_instance_exited_is_sent_again() {
    let seed = never();
    let host = ControlHost::start_seeded(seed.rows.clone());
    let this = seed.this.instance_id.clone();
    let relay = Relay::start(
        host.address(),
        "/weft/commands/RecordInstanceExited ",
        Loss::Request,
    );
    let stub = Stub::leaked(0);
    let report = report_path("lost-exited-request");
    spawn_follower(relay.address, &seed.this.node_id, stub, &report);

    let lines = wait_for_pass(&report, 6);
    assert_eq!(relay.lost(), 1, "the relay lost no request: {lines:?}");
    assert_eq!(
        state(&host, &this),
        json!("Exited"),
        "the process exited 0, one RecordInstanceExited was lost, and 5 passes later it was \
         never sent again: {lines:?}"
    );
    assert_eq!(
        requests(&host, "weft.workloads.RecordInstanceExited").len(),
        1,
        "{lines:?}"
    );
}

/// `RecordInstanceExited` waits for an answered `RecordInstanceRunning`
/// (`story:cloudspec-submission-job-exit`): the first `RecordInstanceRunning` is lost, so the
/// exit is sent only after the one sent again in a later pass, and the instance reads `Exited`.
#[test]
fn record_instance_exited_waits_for_an_answered_record_instance_running() {
    let seed = never();
    let host = ControlHost::start_seeded(seed.rows.clone());
    let this = seed.this.instance_id.clone();
    let relay = Relay::start(
        host.address(),
        "/weft/commands/RecordInstanceRunning ",
        Loss::Request,
    );
    let stub = Stub::leaked(0);
    let report = report_path("exited-after-running");
    spawn_follower(relay.address, &seed.this.node_id, stub, &report);

    let lines = wait_for_pass(&report, 6);
    assert_eq!(relay.lost(), 1, "the relay lost no request: {lines:?}");
    assert_eq!(state(&host, &this), json!("Exited"), "{lines:?}");
    let running = requests(&host, "weft.workloads.RecordInstanceRunning");
    let exited = requests(&host, "weft.workloads.RecordInstanceExited");
    assert_eq!(
        (running.len(), exited.len()),
        (1, 1),
        "{running:?} {exited:?}"
    );
    assert!(
        running[0]["seq"].as_u64() < exited[0]["seq"].as_u64(),
        "{running:?} {exited:?}"
    );
}
