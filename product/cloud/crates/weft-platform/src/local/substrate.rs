//! The local adapter of the node-layer ports [`Exec`] and [`NodeFacts`] (cloudspec's [`docs/platform-ports.md`](https://github.com/beyond10x/cloudspec/blob/2ed72c2866bbd165da2de75250b2bfcb340a96ae/docs/platform-ports.md)
//! § 12.2): one `substrate-daemon` on this host, reached on its Unix socket through
//! `b10x-substrate-sdk`. This is the only source file of weft that names the SDK.
//!
//! [`Confine::run`] creates an empty workspace, starts the process there with its command, its
//! environment and the host files it is given read-only, reports each state the exec enters
//! (`accepted`, `running`, ...) from the daemon's event history, and on `exec.exited` returns the
//! exit code and the captured output. The exec has no network: substrate runs every exec under
//! `--unshare-net` unless an egress aperture is named, and none is. The exec is retired and the
//! workspace destroyed when the run ends, also when it fails.
//!
//! [`Confine::observe`] reads the daemon's machine facts (`GET /v1/machine`) and the host's own:
//! its CPUs and whether it has KVM.
//!
//! The ports are synchronous and the SDK is async. The adapter's client and its current-thread
//! runtime live on a worker thread of their own; `run` and `observe` send their work there and
//! wait for the answer, so they may be called from any thread, one inside a tokio runtime
//! included, and from `run`'s own `on_state`, which runs on the caller's thread.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use b10x_substrate_sdk as sdk;
use b10x_substrate_sdk::{
    Client, Event, EventPage, ExecutionPolicy, OutputStream, ReadOnlyFile, SdkError, Signal,
    Workspace, MAX_EVENT_PAGE_ITEMS, MAX_IO_BYTES,
};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;

use crate::node::{
    Capacity, Exec, ExecError, ExecState, Exited, Facts, Isolation, IsolationLevel, NodeFacts,
    Process, RefusalClass,
};
use crate::Refusal;

/// How long past the exec's own timeout `run` waits for `exec.exited`.
const EXIT_GRACE: Duration = Duration::from_secs(10);

/// How long an exec that did not end gets after `SIGKILL` before it is retired.
const KILL_GRACE: Duration = Duration::from_secs(2);

/// How long `run` waits before it asks the daemon for its next events.
const POLL: Duration = Duration::from_millis(50);

/// The most events one page of `GET /v1/events` holds.
const PAGE_ITEMS: usize = MAX_EVENT_PAGE_ITEMS as usize;

/// A connection to one `substrate-daemon`: its client and the current-thread runtime the client
/// runs on, both owned by the adapter's worker thread.
///
/// A run follows its exec through the daemon's pulled event pages (`GET /v1/events` from a
/// cursor), never through an event stream: substrate grants each subject only four streams, and
/// a stream opened at the start of the history ends after sixteen pages of catch-up. The pull
/// starts at a position taken before the exec starts, so the exec's first event is on a page the
/// run reads, and each page holds every subject's events; the run keeps those of its own exec.
pub struct Confine {
    /// Work for the worker; dropped first, which ends the worker once its work is done.
    jobs: Option<UnboundedSender<Job>>,
    worker: Option<JoinHandle<()>>,
    /// A cursor of the daemon's event history at or before now: where the last run stopped, or
    /// the end of the history at `connect`.
    position: Arc<Mutex<Option<String>>>,
}

/// One piece of work for the worker: given its client, a future it runs on its runtime. The
/// future need not be `Send`: it never leaves the worker's thread.
type Job = Box<dyn FnOnce(Client) -> Pin<Box<dyn Future<Output = ()>>> + Send>;

/// What a run tells the caller's thread: each state, then the end.
enum Report {
    State(ExecState),
    Ended(Result<Exited, ExecError>),
}

impl Confine {
    /// Connects to the daemon at `socket` and takes the end of its event history.
    ///
    /// # Errors
    ///
    /// The worker or its runtime cannot be started, or the daemon does not answer.
    pub fn connect(socket: &Path) -> Result<Self, ExecError> {
        let socket = socket.to_path_buf();
        let (ready, connected) = mpsc::channel();
        let (jobs, received) = tokio::sync::mpsc::unbounded_channel::<Job>();
        let worker = std::thread::Builder::new()
            .name("weft-substrate".to_owned())
            .spawn(move || work(&socket, &ready, received))
            .map_err(|error| Refusal::Unavailable {
                detail: format!("a thread for the substrate client: {error}"),
            })?;
        match connected.recv() {
            Ok(Ok(cursor)) => Ok(Self {
                jobs: Some(jobs),
                worker: Some(worker),
                position: Arc::new(Mutex::new(cursor)),
            }),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(_) => {
                let _ = worker.join();
                Err(worker_gone().into())
            }
        }
    }

    /// Sends `job` to the worker.
    fn send(&self, job: Job) -> Result<(), Refusal> {
        self.jobs
            .as_ref()
            .ok_or_else(worker_gone)?
            .send(job)
            .map_err(|_| worker_gone())
    }

    /// Runs `work` on the worker with its client and waits for its answer.
    fn call<T, W, F>(&self, work: W) -> Result<T, Refusal>
    where
        T: Send + 'static,
        W: FnOnce(Client) -> F + Send + 'static,
        F: Future<Output = T> + 'static,
    {
        let (answer, answered) = mpsc::channel();
        self.send(Box::new(move |client| {
            Box::pin(async move {
                let _ = answer.send(work(client).await);
            })
        }))?;
        answered.recv().map_err(|_| worker_gone())
    }
}

impl Drop for Confine {
    /// Ends the worker once the work it holds is done, and waits for it: the client and its
    /// runtime are dropped on the worker's thread, never inside a caller's runtime.
    fn drop(&mut self) {
        drop(self.jobs.take());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// The worker: builds the runtime, connects, answers `ready` with the end of the event history
/// or the refusal, then runs each job it receives as a task of its own until the adapter is
/// dropped, and the tasks still running to their ends.
fn work(
    socket: &Path,
    ready: &mpsc::Sender<Result<Option<String>, ExecError>>,
    mut received: tokio::sync::mpsc::UnboundedReceiver<Job>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = ready.send(Err(Refusal::Unavailable {
                detail: format!("a runtime for the substrate client: {error}"),
            }
            .into()));
            return;
        }
    };
    let client = match runtime.block_on(connect_async(socket)) {
        Ok((client, cursor)) => {
            let _ = ready.send(Ok(cursor));
            client
        }
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let tasks = tokio::task::LocalSet::new();
    runtime.block_on(tasks.run_until(async {
        while let Some(job) = received.recv().await {
            tokio::task::spawn_local(job(client.clone()));
        }
    }));
    runtime.block_on(tasks);
    runtime.block_on(async move { drop(client) });
}

/// The adapter's worker has ended: it panicked, or the adapter is being dropped.
fn worker_gone() -> Refusal {
    Refusal::Unavailable {
        detail: "the worker of the substrate client has ended".to_owned(),
    }
}

async fn connect_async(socket: &Path) -> Result<(Client, Option<String>), ExecError> {
    let client = Client::builder()
        .unix_socket(PathBuf::from(socket))
        .connect()
        .await
        .map_err(refused)?;
    let mut cursor = None;
    loop {
        let page = events_after(&client, cursor.as_deref()).await?;
        let full = page.events.len() >= PAGE_ITEMS;
        cursor = Some(page.next_cursor);
        if !full {
            return Ok((client, cursor));
        }
    }
}

fn read(position: &Mutex<Option<String>>) -> Option<String> {
    position
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

fn remember(position: &Mutex<Option<String>>, cursor: Option<String>) {
    *position
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = cursor;
}

/// The execution policy of `process`'s limits; limits outside substrate's bounds are `Invalid`.
fn policy(process: &Process) -> Result<ExecutionPolicy, ExecError> {
    let limits = process.limits;
    ExecutionPolicy::builder()
        .timeout(limits.timeout)
        .cpu_time(limits.cpu_time)
        .memory_bytes(limits.memory_bytes)
        .processes(limits.processes)
        .output_bytes(limits.output_bytes)
        .build()
        .map_err(|error| {
            Refusal::Invalid {
                field: "limits".to_owned(),
                reason: error.to_string(),
            }
            .into()
        })
}

async fn run_async(
    client: &Client,
    position: &Mutex<Option<String>>,
    process: &Process,
    on_state: &mut dyn FnMut(ExecState),
) -> Result<Exited, ExecError> {
    let (program, args) = process.argv.split_first().ok_or(ExecError::EmptyCommand)?;
    let mut files = Vec::with_capacity(process.files.len());
    for file in &process.files {
        let host_path = file
            .host_path
            .to_str()
            .ok_or_else(|| ExecError::HostPath(file.host_path.clone()))?;
        files.push(ReadOnlyFile {
            host_path: host_path.to_owned(),
            mount: file.mount.clone(),
        });
    }
    let policy = policy(process)?;

    // A position taken before the start, so no event of this exec can precede it.
    let mut cursor = read(position);
    let workspace = client.workspace().empty().create().await.map_err(refused)?;
    let mut command = workspace
        .command(program.clone())
        .args(args.iter().cloned())
        .policy(policy);
    for (name, value) in &process.env {
        command = command.env(name.clone(), value.clone());
    }
    for file in files {
        command = command.read_only_file(file);
    }
    let started = command.start().await;
    let mut exec = match started {
        Ok(exec) => exec,
        Err(error) => {
            let _ = workspace.destroy().await;
            return Err(refused(error));
        }
    };
    let id = exec.id().to_owned();

    let wait = process.limits.timeout + EXIT_GRACE;
    let followed = tokio::time::timeout(wait, async {
        let mut state = None;
        loop {
            let page = events_after(client, cursor.as_deref()).await?;
            let full = page.events.len() >= PAGE_ITEMS;
            cursor = Some(page.next_cursor);
            for event in &page.events {
                let Some(next) = state_of(event, &id) else {
                    continue;
                };
                if state != Some(next) {
                    state = Some(next);
                    on_state(next);
                }
                if next == ExecState::Exited {
                    return Ok(());
                }
                if next.terminal() {
                    return Err(ExecError::Ended {
                        state: next,
                        refusal: None,
                    });
                }
            }
            if !full {
                tokio::time::sleep(POLL).await;
            }
        }
    })
    .await
    .unwrap_or(Err(ExecError::NoExit(wait)));
    remember(position, cursor);

    let ended = match followed {
        Ok(()) => collect(&mut exec).await,
        Err(ExecError::Ended { state, .. }) => {
            let refusal = exec
                .refresh()
                .await
                .ok()
                .and_then(|observed| observed.refusal.as_ref().map(|r| format!("{r:?}")));
            Err(ExecError::Ended { state, refusal })
        }
        Err(error) => Err(error),
    };
    finish(exec, workspace).await;
    ended
}

impl Exec for Confine {
    fn run(
        &self,
        process: &Process,
        on_state: &mut dyn FnMut(ExecState),
    ) -> Result<Exited, ExecError> {
        let (report, reports) = mpsc::channel();
        let process = process.clone();
        let position = Arc::clone(&self.position);
        self.send(Box::new(move |client| {
            Box::pin(async move {
                let states = report.clone();
                let ended = run_async(&client, &position, &process, &mut |state| {
                    let _ = states.send(Report::State(state));
                })
                .await;
                let _ = report.send(Report::Ended(ended));
            })
        }))?;
        for next in reports {
            match next {
                Report::State(state) => on_state(state),
                Report::Ended(ended) => return ended,
            }
        }
        Err(worker_gone().into())
    }
}

impl NodeFacts for Confine {
    /// The node's CPUs as its capacity, and its isolation facts: what the daemon proves for its
    /// execs, and whether the host has KVM. The level is `Container` only when the daemon proves
    /// the exec floor ([`container`]); else the node states no level. A GPU is not passed
    /// through. A fact the daemon withholds stays `None`.
    fn observe(&self) -> Result<Facts, Refusal> {
        let machine = self
            .call(|client| async move { client.refresh_machine().await })?
            .map_err(|error| Refusal::Unavailable {
                detail: error.to_string(),
            })?;
        let facts = &machine.facts;
        let cpus = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
        let levels = if container(&machine) {
            vec![IsolationLevel::Container]
        } else {
            Vec::new()
        };
        Ok(Facts {
            capacity: vec![Capacity {
                resource_class: "cpu".to_owned(),
                amount: u64::try_from(cpus).unwrap_or(u64::MAX),
            }],
            isolation: Isolation {
                levels,
                cgroups_delegated: facts
                    .exec_cgroup_limits
                    .as_ref()
                    .map(|limits| limits.cpu && limits.memory && limits.processes),
                egress_denied: facts.exec_no_egress,
                kvm_present: Some(Path::new("/dev/kvm").exists()),
                gpu_passthrough: Some(false),
            },
        })
    }
}

/// The daemon proves the confinement of `Container` for its execs: the exec floor its start-up
/// probe proved (`exec.argv-only`, and `exec.namespaces` with every namespace,
/// `substrate@0.29.0:crates/substrate-host/src/probe.rs:341-344`), the facts without which it
/// refuses every start (`exec.cgroup-limits`, `exec.cgroup-kill`,
/// `crates/substrate-host/src/process.rs:2974-2979`), with all three controllers, and no egress,
/// which this adapter relies on (it names no aperture). A fact withheld is not proved.
fn container(machine: &sdk::Machine) -> bool {
    let facts = &machine.facts;
    facts.exec_argv_only == Some(true)
        && facts.exec_namespaces.as_ref().is_some_and(|namespaces| {
            namespaces.user
                && namespaces.mount
                && namespaces.pid
                && namespaces.ipc
                && namespaces.uts
                && namespaces.network
        })
        && facts
            .exec_cgroup_limits
            .as_ref()
            .is_some_and(|limits| limits.cpu && limits.memory && limits.processes)
        && facts.exec_cgroup_kill == Some(true)
        && facts.exec_no_egress == Some(true)
}

/// A refusal of the daemon with its class, code, message and whether it is retriable; anything
/// else (no answer, an answer outside its contract, a request the client did not send) as
/// `Unavailable`. Limits outside substrate's bounds never get here: [`policy`] refuses them.
fn refused(error: SdkError) -> ExecError {
    match error {
        SdkError::Refusal(refusal) => ExecError::Refused {
            class: match refusal.class {
                sdk::RefusalClass::Refused => RefusalClass::Refused,
                sdk::RefusalClass::Conflict => RefusalClass::Conflict,
                sdk::RefusalClass::Unserved => RefusalClass::Unserved,
                sdk::RefusalClass::Exhausted => RefusalClass::Exhausted,
                sdk::RefusalClass::Failed => RefusalClass::Failed,
            },
            code: refusal.code,
            message: refusal.message,
            retriable: refusal.retriable,
        },
        error => ExecError::Shared(Refusal::Unavailable {
            detail: error.to_string(),
        }),
    }
}

/// One page of the daemon's events after `cursor`, or from the oldest it keeps when `cursor` is
/// none. A cursor the daemon no longer serves (`event.retention-gap`: more events since than it
/// keeps; `event.source-mismatch`: a new event history) is read again from the oldest kept event:
/// the events of an exec started after the cursor are the newest and are kept.
async fn events_after(client: &Client, cursor: Option<&str>) -> Result<EventPage, ExecError> {
    match client.events(cursor, MAX_EVENT_PAGE_ITEMS).await {
        Err(SdkError::Refusal(refusal))
            if cursor.is_some()
                && matches!(
                    refusal.code.as_str(),
                    "event.retention-gap" | "event.source-mismatch"
                ) =>
        {
            client
                .events(None, MAX_EVENT_PAGE_ITEMS)
                .await
                .map_err(refused)
        }
        page => page.map_err(refused),
    }
}

/// The state of exec `id` an event records, or none when the event records none.
///
/// substrate 0.29.0 keeps an exec `accepted` from the reservation of its start until the start
/// answers, and records that state on the start operation only: `operation.accepted`, operation
/// kind `exec.start`, `resource` the exec. Every later state is on an event of the exec itself:
/// `exec.observed` (the start's answer, `running` or already ended) and one event per change
/// (`exec.running`, `exec.exited`, ...), each carrying the exec's observation with its `state`.
fn state_of(event: &Event, id: &str) -> Option<ExecState> {
    let observation = &event.observation;
    if event.transition == "operation.accepted" {
        let starts_it = observation.get("operation_kind").and_then(Value::as_str)
            == Some("exec.start")
            && observation.get("resource").and_then(Value::as_str) == Some(id);
        return starts_it.then_some(ExecState::Accepted);
    }
    if event.resource != id || !event.transition.starts_with("exec.") {
        return None;
    }
    match observation.get("state").and_then(Value::as_str)? {
        "accepted" => Some(ExecState::Accepted),
        "running" => Some(ExecState::Running),
        "exited" => Some(ExecState::Exited),
        "cancelled" => Some(ExecState::Cancelled),
        "expired" => Some(ExecState::Expired),
        "unknown" => Some(ExecState::Unknown),
        _ => None,
    }
}

/// weft's state for the SDK's.
fn state(state: sdk::ExecState) -> ExecState {
    match state {
        sdk::ExecState::Accepted => ExecState::Accepted,
        sdk::ExecState::Running => ExecState::Running,
        sdk::ExecState::Exited => ExecState::Exited,
        sdk::ExecState::Cancelled => ExecState::Cancelled,
        sdk::ExecState::Expired => ExecState::Expired,
        _ => ExecState::Unknown,
    }
}

async fn collect(exec: &mut sdk::Exec) -> Result<Exited, ExecError> {
    let observed = exec.refresh().await.map_err(refused)?.clone();
    if observed.state != sdk::ExecState::Exited {
        return Err(ExecError::Ended {
            state: state(observed.state),
            refusal: observed.refusal.as_ref().map(|r| format!("{r:?}")),
        });
    }
    let (stdout, stdout_truncated) = output(exec, OutputStream::Stdout).await?;
    let (stderr, stderr_truncated) = output(exec, OutputStream::Stderr).await?;
    Ok(Exited {
        code: observed.exit.as_ref().and_then(|exit| exit.code),
        stdout,
        stderr,
        truncated: stdout_truncated || stderr_truncated,
    })
}

async fn output(exec: &sdk::Exec, stream: OutputStream) -> Result<(Vec<u8>, bool), ExecError> {
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut offset = 0;
    loop {
        let page = exec
            .output_page(stream, offset, MAX_IO_BYTES)
            .await
            .map_err(refused)?;
        bytes.extend_from_slice(&page.bytes);
        truncated |= page.truncated;
        offset = page.next_offset;
        if page.eof {
            return Ok((bytes, truncated));
        }
    }
}

/// Kills the exec when it has not ended, retires it and destroys its workspace. A failure here
/// leaves both to the daemon's bounds and does not change the run's answer.
async fn finish(mut exec: sdk::Exec, workspace: Workspace) {
    if !exec.observation().state.terminal() {
        let _ = exec.signal(Signal::Kill, KILL_GRACE).await;
        let _ = exec.wait_for(KILL_GRACE).await;
    }
    let _ = exec.retire().await;
    let _ = workspace.destroy().await;
}
