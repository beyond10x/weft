//! The local adapter of the node-layer ports [`Exec`] and [`NodeFacts`] (`docs/platform-ports.md`
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

use std::future::Future;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use b10x_substrate_sdk as sdk;
use b10x_substrate_sdk::{
    Client, Event, EventPage, ExecutionPolicy, OutputStream, ReadOnlyFile, SdkError, Signal,
    Workspace, MAX_EVENT_PAGE_ITEMS, MAX_IO_BYTES,
};
use serde_json::Value;

use crate::node::{
    Capacity, Exec, ExecError, ExecState, Exited, Facts, Isolation, IsolationLevel, NodeFacts,
    Process,
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

/// A connection to one `substrate-daemon`, with the current-thread runtime its client runs on.
///
/// A run follows its exec through the daemon's pulled event pages (`GET /v1/events` from a
/// cursor), never through an event stream: substrate grants each subject only four streams, and
/// a stream opened at the start of the history ends after sixteen pages of catch-up. The pull
/// starts at a position taken before the exec starts, so the exec's first event is on a page the
/// run reads, and each page holds every subject's events; the run keeps those of its own exec.
pub struct Confine {
    client: Client,
    /// A cursor of the daemon's event history at or before now: where the last run stopped, or
    /// the end of the history at `connect`.
    position: Mutex<Option<String>>,
    /// Dropped last, after the client whose tasks it holds.
    runtime: tokio::runtime::Runtime,
}

impl Confine {
    /// Connects to the daemon at `socket` and takes the end of its event history.
    ///
    /// # Errors
    ///
    /// The runtime cannot be built, or the daemon does not answer.
    pub fn connect(socket: &Path) -> Result<Self, ExecError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| Refusal::Unavailable {
                detail: format!("a runtime for the substrate client: {error}"),
            })?;
        let (client, cursor) = runtime.block_on(async {
            let client = Client::builder()
                .unix_socket(socket)
                .connect()
                .await
                .map_err(refused)?;
            let mut cursor = None;
            loop {
                let page = events_after(&client, cursor.as_deref()).await?;
                let full = page.events.len() >= PAGE_ITEMS;
                cursor = Some(page.next_cursor);
                if !full {
                    break;
                }
            }
            Ok::<_, ExecError>((client, cursor))
        })?;
        Ok(Self {
            client,
            position: Mutex::new(cursor),
            runtime,
        })
    }

    fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }

    fn position(&self) -> Option<String> {
        self.position
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn remember(&self, cursor: Option<String>) {
        *self
            .position
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = cursor;
    }

    async fn run_async(
        &self,
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
        let limits = process.limits;
        let policy = ExecutionPolicy::builder()
            .timeout(limits.timeout)
            .cpu_time(limits.cpu_time)
            .memory_bytes(limits.memory_bytes)
            .processes(limits.processes)
            .output_bytes(limits.output_bytes)
            .build()
            .map_err(refused)?;

        // A position taken before the start, so no event of this exec can precede it.
        let mut cursor = self.position();
        let workspace = self
            .client
            .workspace()
            .empty()
            .create()
            .await
            .map_err(refused)?;
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

        let wait = limits.timeout + EXIT_GRACE;
        let followed = tokio::time::timeout(wait, async {
            let mut state = None;
            loop {
                let page = events_after(&self.client, cursor.as_deref()).await?;
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
        self.remember(cursor);

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
}

impl Exec for Confine {
    fn run(
        &self,
        process: &Process,
        on_state: &mut dyn FnMut(ExecState),
    ) -> Result<Exited, ExecError> {
        self.block_on(self.run_async(process, on_state))
    }
}

impl NodeFacts for Confine {
    /// The node's CPUs as its capacity, and its isolation facts: what the daemon proves for its
    /// execs, and whether the host has KVM. The level is `Container`, the confinement substrate
    /// gives; a GPU is not passed through. A fact the daemon withholds stays `None`.
    fn observe(&self) -> Result<Facts, Refusal> {
        let machine = self
            .block_on(self.client.refresh_machine())
            .map_err(|error| Refusal::Unavailable {
                detail: error.to_string(),
            })?;
        let facts = &machine.facts;
        let cpus = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
        Ok(Facts {
            capacity: vec![Capacity {
                resource_class: "cpu".to_owned(),
                amount: u64::try_from(cpus).unwrap_or(u64::MAX),
            }],
            isolation: Isolation {
                levels: vec![IsolationLevel::Container],
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

/// A refusal of the daemon with its code; anything else (no answer, an answer outside its
/// contract, a request the client did not send) as `Unavailable`.
fn refused(error: SdkError) -> ExecError {
    match error {
        SdkError::Refusal(refusal) => ExecError::Refused {
            code: refusal.code,
            message: refusal.message,
        },
        SdkError::Builder { field } => ExecError::Shared(Refusal::Invalid {
            field: field.to_owned(),
            reason: "is required".to_owned(),
        }),
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
