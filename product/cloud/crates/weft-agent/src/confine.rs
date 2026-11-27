//! One process of an instance, run as one confined exec of `substrate-daemon` (`docs/implementation.md`
//! A6): the daemon's Unix socket, reached through `b10x-substrate-sdk`.
//!
//! [`Confine::run`] creates an empty workspace, starts the process there with its command, its
//! environment and the host files it is given read-only, reports each state the exec enters
//! (`accepted`, `running`, ...) from the daemon's event stream, and on `exec.exited` returns the
//! exit code and the captured output. The exec has no network: substrate runs every exec under
//! `--unshare-net` unless an egress aperture is named, and none is. The exec is retired and the
//! workspace destroyed when the run ends, also when it fails.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use b10x_substrate_sdk::{
    Client, Event, Exec, ExecutionPolicy, OutputStream, ReadOnlyFile, SdkError, Signal, Workspace,
    MAX_EVENT_PAGE_ITEMS, MAX_IO_BYTES,
};
use serde_json::Value;

pub use b10x_substrate_sdk::ExecState;

/// A host file bound read-only into the exec at `mount`: a program or a configuration file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFile {
    pub host_path: PathBuf,
    pub mount: String,
}

impl HostFile {
    pub fn new(host_path: impl Into<PathBuf>, mount: impl Into<String>) -> Self {
        Self {
            host_path: host_path.into(),
            mount: mount.into(),
        }
    }
}

/// The bounds of one exec. Substrate takes no default for any of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub timeout: Duration,
    pub cpu_time: Duration,
    pub memory_bytes: u64,
    pub processes: u32,
    pub output_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(20),
            cpu_time: Duration::from_secs(10),
            memory_bytes: 256 * 1024 * 1024,
            processes: 64,
            output_bytes: 1024 * 1024,
        }
    }
}

/// One process of an instance: its command line, its environment and the host files it reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub files: Vec<HostFile>,
    pub limits: Limits,
}

impl Process {
    pub fn new<I, S>(argv: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            argv: argv.into_iter().map(Into::into).collect(),
            env: BTreeMap::new(),
            files: Vec::new(),
            limits: Limits::default(),
        }
    }

    pub fn env(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(name.into(), value.into());
        self
    }

    pub fn file(mut self, file: HostFile) -> Self {
        self.files.push(file);
        self
    }

    pub fn limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }
}

/// The end of an exec that reached `exited`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exited {
    /// The exit code; absent when a signal ended the process.
    pub code: Option<u8>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// Substrate kept less than the process wrote (`output_bytes`).
    pub truncated: bool,
}

#[derive(Debug)]
pub enum Error {
    /// The daemon refused, could not be reached, or answered outside its contract.
    Substrate(SdkError),
    /// The process has no command.
    EmptyCommand,
    /// A host file has no path in UTF-8.
    HostPath(PathBuf),
    /// The exec ended in a state other than `exited` (`cancelled`, `expired`).
    Ended {
        state: ExecState,
        refusal: Option<String>,
    },
    /// The event stream closed before `exec.exited`.
    StreamClosed,
    /// No `exec.exited` within the exec's timeout and a grace.
    NoExit(Duration),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Substrate(error) => write!(f, "{error}"),
            Self::EmptyCommand => write!(f, "the process has no command"),
            Self::HostPath(path) => write!(f, "the host file {} has no UTF-8 path", path.display()),
            Self::Ended { state, refusal } => {
                write!(f, "the exec ended {state:?}, not exited")?;
                match refusal {
                    Some(refusal) => write!(f, ": {refusal}"),
                    None => Ok(()),
                }
            }
            Self::StreamClosed => write!(f, "the event stream closed before exec.exited"),
            Self::NoExit(after) => write!(f, "no exec.exited after {after:?}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<SdkError> for Error {
    fn from(error: SdkError) -> Self {
        Self::Substrate(error)
    }
}

/// How long past the exec's own timeout `run` waits for `exec.exited`.
const EXIT_GRACE: Duration = Duration::from_secs(10);

/// How long an exec that did not end gets after `SIGKILL` before it is retired.
const KILL_GRACE: Duration = Duration::from_secs(2);

/// A connection to one `substrate-daemon`.
pub struct Confine {
    client: Client,
}

impl Confine {
    pub async fn connect(socket: &Path) -> Result<Self, Error> {
        let client = Client::builder().unix_socket(socket).connect().await?;
        Ok(Self { client })
    }

    /// Runs `process` as one exec and returns its end once the daemon reports `exec.exited`.
    /// `on_state` gets each state the daemon's events record for the exec, in their order, once
    /// per change ([`state_of`]); `accepted` comes first.
    pub async fn run(
        &self,
        process: &Process,
        mut on_state: impl FnMut(ExecState),
    ) -> Result<Exited, Error> {
        let (program, args) = process.argv.split_first().ok_or(Error::EmptyCommand)?;
        let mut files = Vec::with_capacity(process.files.len());
        for file in &process.files {
            let host_path = file
                .host_path
                .to_str()
                .ok_or_else(|| Error::HostPath(file.host_path.clone()))?;
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
            .build()?;

        // The stream is opened before the start, so no event of this exec can precede it.
        let mut events = self.client.event_stream(None, MAX_EVENT_PAGE_ITEMS).await?;
        let workspace = self.client.workspace().empty().create().await?;
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
                return Err(error.into());
            }
        };
        let id = exec.id().to_owned();

        let wait = limits.timeout + EXIT_GRACE;
        let followed = tokio::time::timeout(wait, async {
            let mut state = None;
            loop {
                let Some(page) = events.next_page().await? else {
                    return Err(Error::StreamClosed);
                };
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
                        return Err(Error::Ended {
                            state: next,
                            refusal: None,
                        });
                    }
                }
            }
        })
        .await
        .unwrap_or(Err(Error::NoExit(wait)));

        let ended = match followed {
            Ok(()) => collect(&mut exec).await,
            Err(Error::Ended { state, .. }) => {
                let refusal = exec
                    .refresh()
                    .await
                    .ok()
                    .and_then(|observed| observed.refusal.as_ref().map(|r| format!("{r:?}")));
                Err(Error::Ended { state, refusal })
            }
            Err(error) => Err(error),
        };
        finish(exec, workspace).await;
        ended
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

async fn collect(exec: &mut Exec) -> Result<Exited, Error> {
    let observed = exec.refresh().await?.clone();
    if observed.state != ExecState::Exited {
        return Err(Error::Ended {
            state: observed.state,
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

async fn output(exec: &Exec, stream: OutputStream) -> Result<(Vec<u8>, bool), Error> {
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut offset = 0;
    loop {
        let page = exec.output_page(stream, offset, MAX_IO_BYTES).await?;
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
async fn finish(mut exec: Exec, workspace: Workspace) {
    if !exec.observation().state.terminal() {
        let _ = exec.signal(Signal::Kill, KILL_GRACE).await;
        let _ = exec.wait_for(KILL_GRACE).await;
    }
    let _ = exec.retire().await;
    let _ = workspace.destroy().await;
}
