//! The node-layer ports of the first build (cloudspec's [`docs/platform-ports.md`](https://github.com/beyond10x/cloudspec/blob/2ed72c2866bbd165da2de75250b2bfcb340a96ae/docs/platform-ports.md) § 5): N1 [`Exec`], one
//! confined process that runs to its end, N2 [`NodeFacts`], what the node observes about
//! itself, and N3 [`Workloads`], long-lived supervised workloads; no adapter of a place serves N3
//! yet, only the in-memory stub of [`crate::local::stub`]. All are synchronous: `weft-agent` is a
//! blocking program, and an adapter that needs a runtime keeps its own.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use crate::Refusal;

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

impl Limits {
    /// The longest `timeout` and `cpu_time` the exec port accepts: substrate's
    /// `MAX_EXEC_DURATION`, 24 hours. A case of weft-platform's tests holds the two equal and
    /// runs an exec with it; a longer one is refused `Invalid`.
    pub const LONGEST: Duration = Duration::from_secs(24 * 60 * 60);
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

/// The state of an exec, in lifecycle order: `accepted`, `running`, then one of the ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecState {
    Accepted,
    Running,
    Exited,
    Cancelled,
    Expired,
    Unknown,
}

impl ExecState {
    /// The exec has ended: `exited`, `cancelled` or `expired`. `unknown` is not an end.
    pub const fn terminal(self) -> bool {
        matches!(self, Self::Exited | Self::Cancelled | Self::Expired)
    }
}

/// The end of an exec that reached `exited`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exited {
    /// The exit code; absent when a signal ended the process.
    pub code: Option<u8>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// The place kept less than the process wrote (`output_bytes`).
    pub truncated: bool,
}

/// The class of a refusal of the place (`{class, code, message, retriable}`,
/// cloudspec's [`docs/platform-ports.md`](https://github.com/beyond10x/cloudspec/blob/2ed72c2866bbd165da2de75250b2bfcb340a96ae/docs/platform-ports.md) § 3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefusalClass {
    Refused,
    Conflict,
    Unserved,
    Exhausted,
    Failed,
}

/// The refusals of [`Exec::run`] (cloudspec's [`docs/platform-ports.md`](https://github.com/beyond10x/cloudspec/blob/2ed72c2866bbd165da2de75250b2bfcb340a96ae/docs/platform-ports.md) § 5 N1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecError {
    /// The process has no command.
    EmptyCommand,
    /// A host file has no path in UTF-8.
    HostPath(PathBuf),
    /// The exec ended in a state other than `exited` (`cancelled`, `expired`).
    Ended {
        state: ExecState,
        refusal: Option<String>,
    },
    /// No `exited` within the exec's timeout and a grace.
    NoExit(Duration),
    /// The place refused the request with its own code, such as `exec.sandbox-unavailable`,
    /// carried as the place states it (cloudspec's [`docs/platform-ports.md`](https://github.com/beyond10x/cloudspec/blob/2ed72c2866bbd165da2de75250b2bfcb340a96ae/docs/platform-ports.md) § 3.4).
    Refused {
        class: RefusalClass,
        code: String,
        message: String,
        /// The place says the same request may succeed later.
        retriable: bool,
    },
    /// A refusal every port shares: the place did not answer (`Unavailable`), or an input broke
    /// a bound it states (`Invalid`).
    Shared(Refusal),
}

impl fmt::Display for ExecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCommand => write!(f, "the process has no command"),
            Self::HostPath(path) => write!(f, "the host file {} has no UTF-8 path", path.display()),
            Self::Ended { state, refusal } => {
                write!(f, "the exec ended {state:?}, not exited")?;
                match refusal {
                    Some(refusal) => write!(f, ": {refusal}"),
                    None => Ok(()),
                }
            }
            Self::NoExit(after) => write!(f, "no exec.exited after {after:?}"),
            Self::Refused {
                class,
                code,
                message,
                retriable,
            } => {
                let again = if *retriable { ", retriable" } else { "" };
                write!(f, "refused {code} ({class:?}{again}): {message}")
            }
            Self::Shared(refusal) => write!(f, "{refusal}"),
        }
    }
}

impl std::error::Error for ExecError {}

impl From<Refusal> for ExecError {
    fn from(refusal: Refusal) -> Self {
        Self::Shared(refusal)
    }
}

/// N1 `exec`: one confined process that runs to its end.
pub trait Exec {
    /// Runs `process` as one exec of the place and returns its end once the place reports it
    /// `exited`. `on_state` gets each state the place records for the exec, in their order, once
    /// per change; `accepted` comes first. `on_state` runs on the caller's thread and may call
    /// the same adapter again. One call is one operation of the place: a start that is not
    /// answered is refused `Unavailable`, never started again.
    ///
    /// # Errors
    ///
    /// The refusals of [`ExecError`].
    fn run(
        &self,
        process: &Process,
        on_state: &mut dyn FnMut(ExecState),
    ) -> Result<Exited, ExecError>;
}

/// An isolation level of the specification (`weft.shared.IsolationLevel`), weakest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IsolationLevel {
    ProcessSandbox,
    Container,
    UserKernel,
    MicroVm,
}

impl IsolationLevel {
    /// The variant's name in the specification.
    pub const fn name(self) -> &'static str {
        match self {
            Self::ProcessSandbox => "ProcessSandbox",
            Self::Container => "Container",
            Self::UserKernel => "UserKernel",
            Self::MicroVm => "MicroVm",
        }
    }
}

/// An amount of one resource class the node offers (`weft.shared.ResourceAmount`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capacity {
    pub resource_class: String,
    pub amount: u64,
}

/// What the node proves about the confinement of its processes (`weft.shared.IsolationFacts`).
/// A fact the place withholds is `None`, never `false`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(
    clippy::struct_field_names,
    reason = "the names are the fields of weft.shared.IsolationFacts"
)]
pub struct Isolation {
    pub levels: Vec<IsolationLevel>,
    pub cgroups_delegated: Option<bool>,
    pub egress_denied: Option<bool>,
    pub kvm_present: Option<bool>,
    pub gpu_passthrough: Option<bool>,
}

/// What the node observed about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    pub capacity: Vec<Capacity>,
    pub isolation: Isolation,
}

/// N2 `node-facts`: what the node observes about itself. Reads only.
pub trait NodeFacts {
    /// The node's capacity and isolation facts, observed now.
    ///
    /// # Errors
    ///
    /// [`Refusal::Unavailable`]: the place did not answer.
    fn observe(&self) -> Result<Facts, Refusal>;
}

/// The key of one workload on the node: the assignment that holds it and the name of the process
/// in the instance's `processes` (`weft.node.LocalProcess`: `assignment_id`, `process`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkloadKey {
    pub assignment: String,
    pub process: String,
}

impl WorkloadKey {
    pub fn new(assignment: impl Into<String>, process: impl Into<String>) -> Self {
        Self {
            assignment: assignment.into(),
            process: process.into(),
        }
    }
}

impl fmt::Display for WorkloadKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.assignment, self.process)
    }
}

/// How a process ended on its own: with an exit code, or by a signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProcessExit {
    Code(u8),
    Signal,
}

/// What a stop reaches (`weft.workloads.KillScope`): `Main` the main process only, `Group` every
/// process the workload started (the specification's default), `Mixed` the stop signal to the
/// main process and the kill to all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum KillScope {
    Main,
    #[default]
    Group,
    Mixed,
}

/// How a process is stopped (`weft.workloads.Stop`): the stop signal, then the kill of `kill`
/// once `grace` passed without an end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StopRule {
    pub grace: Duration,
    pub kill: KillScope,
}

impl StopRule {
    /// The grace of a workload that names none: systemd's `DefaultTimeoutStopSec`, 90 seconds.
    pub const DEFAULT_GRACE: Duration = Duration::from_secs(90);

    pub const fn new(grace: Duration, kill: KillScope) -> Self {
        Self { grace, kill }
    }
}

impl Default for StopRule {
    fn default() -> Self {
        Self::new(Self::DEFAULT_GRACE, KillScope::Group)
    }
}

/// Whether a process is started again after it ended (`weft.workloads.RestartMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RestartMode {
    #[default]
    Never,
    OnFailure,
    Always,
}

/// The restart rule of a workload (`weft.workloads.ProcessRestart`): the policy, the wait before
/// the start again, and the exit codes after which the process stays down. An adapter applies it
/// to the exits of the command; a caller that decides restarts itself (`weft-agent`, which reports
/// each with `weft.node.RestartProcess`) passes the default, `Never`, and calls
/// [`Workloads::restart`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct RestartRule {
    pub policy: RestartMode,
    pub delay: Duration,
    pub keep_down_on_exit: Vec<u8>,
}

impl RestartRule {
    pub const fn new(policy: RestartMode, delay: Duration) -> Self {
        Self {
            policy,
            delay,
            keep_down_on_exit: Vec::new(),
        }
    }

    pub fn keep_down_on_exit(mut self, codes: impl IntoIterator<Item = u8>) -> Self {
        self.keep_down_on_exit.extend(codes);
        self
    }

    /// The process is started again after `exit`: never under `Never`, nor after a code in
    /// `keep_down_on_exit`; under `OnFailure` after any end but code 0; under `Always` after
    /// every end.
    pub fn restarts_after(&self, exit: ProcessExit) -> bool {
        let kept_down =
            matches!(exit, ProcessExit::Code(code) if self.keep_down_on_exit.contains(&code));
        if kept_down {
            return false;
        }
        match self.policy {
            RestartMode::Never => false,
            RestartMode::OnFailure => exit != ProcessExit::Code(0),
            RestartMode::Always => true,
        }
    }
}

/// A command run before the command of the workload, in list order
/// (`weft.workloads.PrestartStep`). `privileged` runs it with the host's full rights, not as the
/// workload's user; an adapter that cannot refuses the start `NotServed` with the operation
/// `prestart.privileged`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PrestartStep {
    pub argv: Vec<String>,
    pub privileged: bool,
}

impl PrestartStep {
    pub fn new<I, S>(argv: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            argv: argv.into_iter().map(Into::into).collect(),
            privileged: false,
        }
    }

    pub const fn privileged(mut self, privileged: bool) -> Self {
        self.privileged = privileged;
        self
    }
}

/// One long-lived workload: its command line, environment, working directory and host user, the
/// prestart steps run before each start of the command, how it is stopped and whether the place
/// starts it again after an exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkloadSpec {
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    /// Absent: the place's default.
    pub working_dir: Option<PathBuf>,
    /// The host user the command runs as (`weft.workloads.RunAs`); absent: the place's own.
    pub run_as: Option<String>,
    pub prestart: Vec<PrestartStep>,
    pub stop: StopRule,
    pub restart: RestartRule,
}

impl WorkloadSpec {
    pub fn new<I, S>(argv: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            argv: argv.into_iter().map(Into::into).collect(),
            env: BTreeMap::new(),
            working_dir: None,
            run_as: None,
            prestart: Vec::new(),
            stop: StopRule::default(),
            restart: RestartRule::default(),
        }
    }

    pub fn env(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(name.into(), value.into());
        self
    }

    pub fn working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    pub fn run_as(mut self, user: impl Into<String>) -> Self {
        self.run_as = Some(user.into());
        self
    }

    pub fn prestart(mut self, step: PrestartStep) -> Self {
        self.prestart.push(step);
        self
    }

    pub const fn stop(mut self, stop: StopRule) -> Self {
        self.stop = stop;
        self
    }

    pub fn restart(mut self, restart: RestartRule) -> Self {
        self.restart = restart;
        self
    }

    /// The bounds every adapter checks before it runs anything: a command, and a command in
    /// each prestart step.
    ///
    /// # Errors
    ///
    /// [`Refusal::Invalid`] on the field `argv` or `prestart[<n>].argv`.
    pub fn check(&self) -> Result<(), WorkloadError> {
        let empty = |field: String| {
            Err(WorkloadError::Shared(Refusal::Invalid {
                field,
                reason: "no command".to_owned(),
            }))
        };
        if self.argv.is_empty() {
            return empty("argv".to_owned());
        }
        match self.prestart.iter().position(|step| step.argv.is_empty()) {
            Some(step) => empty(format!("prestart[{step}].argv")),
            None => Ok(()),
        }
    }
}

/// The state of a workload. `starting`: accepted, the prestart steps or the command are being
/// started; `running`; `exited`: the command ended on its own and stays down; `restarting`: it
/// ended or a restart was asked, and it starts again; `failed`: a start that did not reach
/// `running`; `stopped`: ended by a stop; `unknown`: the place lost track of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkloadState {
    Starting,
    Running,
    Exited,
    Restarting,
    Failed,
    Stopped,
    Unknown,
}

/// Why a start did not reach `running`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StartFailure {
    /// Prestart step `step` (from 0) ended with `exit` other than code 0; the later steps and the
    /// command did not run.
    Prestart { step: usize, exit: ProcessExit },
    /// The command did not start (no such program, a user that does not exist).
    Command { reason: String },
}

/// How a stop ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StopEnd {
    /// The process ended within the grace.
    WithinGrace,
    /// The grace ran out and the kill reached this scope.
    Killed(KillScope),
    /// No process ran: it had exited, failed or been stopped, or waited for a restart.
    NotRunning,
}

/// What the place observed about a workload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub state: WorkloadState,
    /// The last end of the command on its own, kept across restarts and stops.
    pub last_exit: Option<ProcessExit>,
    /// The starts again since the first start: by the restart rule or by [`Workloads::restart`].
    pub restarts: u32,
    /// The end of the last stop; present in state `stopped`.
    pub stopped: Option<StopEnd>,
    /// Present in state `failed`.
    pub failure: Option<StartFailure>,
}

/// The refusals of [`Workloads`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkloadError {
    /// The place holds no workload under this key.
    UnknownWorkload(WorkloadKey),
    /// A refusal every port shares: the place did not answer (`Unavailable`), an operation or a
    /// field it does not serve (`NotServed`, such as `prestart.privileged`), or an input that
    /// breaks a bound it states (`Invalid`).
    Shared(Refusal),
}

impl fmt::Display for WorkloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownWorkload(key) => write!(f, "no workload {key}"),
            Self::Shared(refusal) => write!(f, "{refusal}"),
        }
    }
}

impl std::error::Error for WorkloadError {}

impl From<Refusal> for WorkloadError {
    fn from(refusal: Refusal) -> Self {
        Self::Shared(refusal)
    }
}

/// N3 `workloads`: long-lived supervised workloads on the node, each under its [`WorkloadKey`].
/// Each call is one operation of the place and answers the workload's observation after it.
pub trait Workloads {
    /// Starts the workload: the prestart steps in list order, then the command. A second start
    /// of a key that is starting, running or restarting answers its observation and starts
    /// nothing; a start of a key that exited, failed or was stopped starts it anew with `spec`.
    ///
    /// # Errors
    ///
    /// [`WorkloadError::Shared`]: `Invalid` for a spec that fails [`WorkloadSpec::check`],
    /// `NotServed` for a field the place cannot run, `Unavailable`.
    fn start(&self, key: &WorkloadKey, spec: &WorkloadSpec) -> Result<Observation, WorkloadError>;

    /// Stops the workload with `rule`: the stop signal, the kill of its scope after the grace. A
    /// workload that waits for a restart stays down.
    ///
    /// # Errors
    ///
    /// [`WorkloadError::UnknownWorkload`], [`WorkloadError::Shared`] (`Unavailable`).
    fn stop(&self, key: &WorkloadKey, rule: &StopRule) -> Result<Observation, WorkloadError>;

    /// Starts the workload again with its spec, stopping a running command first with the
    /// spec's stop rule; the prestart steps run again.
    ///
    /// # Errors
    ///
    /// [`WorkloadError::UnknownWorkload`], [`WorkloadError::Shared`] (`Unavailable`).
    fn restart(&self, key: &WorkloadKey) -> Result<Observation, WorkloadError>;

    /// The workload's state, observed now.
    ///
    /// # Errors
    ///
    /// [`WorkloadError::UnknownWorkload`], [`WorkloadError::Shared`] (`Unavailable`).
    fn observe(&self, key: &WorkloadKey) -> Result<Observation, WorkloadError>;
}
