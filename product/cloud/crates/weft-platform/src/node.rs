//! The node-layer ports of the first build (`docs/platform-ports.md` § 5): N1 [`Exec`], one
//! confined process that runs to its end, and N2 [`NodeFacts`], what the node observes about
//! itself. Both are synchronous: `weft-agent` is a blocking program, and an adapter that needs a
//! runtime keeps its own.

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
/// `docs/platform-ports.md` § 3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefusalClass {
    Refused,
    Conflict,
    Unserved,
    Exhausted,
    Failed,
}

/// The refusals of [`Exec::run`] (`docs/platform-ports.md` § 5 N1).
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
    /// carried as the place states it (`docs/platform-ports.md` § 3.4).
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
