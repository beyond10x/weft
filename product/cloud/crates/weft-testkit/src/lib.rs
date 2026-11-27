//! Helpers shared by the tests of weft's crates.

use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::Value;
use weft_client::Client;

pub mod in_process;
pub mod node_agent;
pub mod steps;

pub use in_process::InProcess;
pub use node_agent::NodeAgent;

/// The repository root: the directory of the root workspace manifest.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("product/<product>/crates/<name> lies four levels below the root")
        .to_path_buf()
}

/// The directory of the running test binary's profile (`target/debug`), where cargo puts the
/// binaries of the workspace.
fn profile_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("the test binary has a path");
    // target/<profile>/deps/<test binary>
    exe.parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| repo_root().join("target/debug"))
}

/// A directory of its own under the profile directory (`target/debug/testkit/`), removed on
/// drop. Never under `/tmp`.
#[derive(Debug)]
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// A new, empty directory.
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let name = format!(
            "{}-{}-{nanos}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        );
        let path = profile_dir().join("testkit").join(name);
        std::fs::create_dir_all(&path)
            .unwrap_or_else(|error| panic!("cannot create {}: {error}", path.display()));
        Self { path }
    }

    /// The directory.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Default for TempDir {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// The control host binary of the test build: `WEFT_CONTROL_HOST` when set, else `weft` beside
/// the test binary's profile directory, which `cargo nextest run --workspace --no-run` builds
/// with the feature `test-callers` (this crate's dev-dependency on `weft-bin` asks for it).
pub fn control_host_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("WEFT_CONTROL_HOST") {
        return PathBuf::from(path);
    }
    let path = profile_dir().join("weft");
    assert!(
        path.is_file(),
        "no control host binary at {}: build the workspace tests first (cargo nextest run --workspace --no-run)",
        path.display()
    );
    path
}

/// How long a start may take before the control host counts as failed.
const START_LIMIT: Duration = Duration::from_secs(30);

/// The test build of `weft-bin` on loopback: a temporary state file, `--port 0`, the log in its
/// temporary directory. Stopped on drop.
#[derive(Debug)]
pub struct ControlHost {
    child: Option<Child>,
    dir: Option<TempDir>,
    address: SocketAddr,
}

impl ControlHost {
    /// Starts a control host on a new, empty state file.
    pub fn start() -> Self {
        Self::start_in(TempDir::new())
    }

    /// Starts a control host on a state file that holds `rows`, written through the storage port
    /// before the start.
    pub fn start_seeded(rows: Vec<weft_impl::Row>) -> Self {
        let dir = TempDir::new();
        seed(&state_path(&dir), rows);
        Self::start_in(dir)
    }

    fn start_in(dir: TempDir) -> Self {
        let binary = control_host_binary();
        let mut child = Command::new(&binary)
            .arg("--state")
            .arg(state_path(&dir))
            .arg("--log")
            .arg(log_path(&dir))
            .arg("--port")
            .arg("0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap_or_else(|error| panic!("cannot start {}: {error}", binary.display()));
        let stdout = child.stdout.take().expect("stdout is piped");
        let (lines, received) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if lines.send(line).is_err() {
                    break;
                }
            }
        });
        let line = match received.recv_timeout(START_LIMIT) {
            Ok(line) => line,
            Err(_) => {
                let _ = child.kill();
                let status = child.wait();
                panic!("the control host printed no listening line: it ended, or {START_LIMIT:?} passed ({status:?})");
            }
        };
        let address = parse_listening(&line).unwrap_or_else(|| {
            let _ = child.kill();
            panic!("not a listening line: {line:?}")
        });
        assert!(
            line.contains("callers=test-header"),
            "the control host at {} is not the test build (feature test-callers of weft-bin): {line}",
            binary.display()
        );
        Self {
            child: Some(child),
            dir: Some(dir),
            address,
        }
    }

    /// Stops this control host and starts another on the same state file and log.
    pub fn restart(mut self) -> Self {
        self.kill();
        let dir = self.dir.take().expect("the directory is held until drop");
        Self::start_in(dir)
    }

    /// Stops this control host; its state file and log stay until drop, for a read through the
    /// storage port ([`weft_impl::Ports::open`]) of what no view shows.
    pub fn stop(&mut self) {
        self.kill();
    }

    /// A client of this control host.
    pub fn client(&self) -> Client {
        Client::new(self.address)
    }

    /// The address the control host listens on.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// The state file.
    pub fn state_path(&self) -> PathBuf {
        state_path(self.dir.as_ref().expect("the directory is held until drop"))
    }

    /// The control host log.
    pub fn log_path(&self) -> PathBuf {
        log_path(self.dir.as_ref().expect("the directory is held until drop"))
    }

    /// The highest `seq` of the log, `0` before the first line.
    pub fn last_seq(&self) -> u64 {
        weft_client::last_seq(&self.log_path()).expect("the log reads")
    }

    /// The log lines after `seq`, in order.
    pub fn log_after(&self, seq: u64) -> Vec<Value> {
        weft_client::log_after(&self.log_path(), seq).expect("the log reads")
    }

    fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for ControlHost {
    fn drop(&mut self) {
        self.kill();
    }
}

fn state_path(dir: &TempDir) -> PathBuf {
    dir.path().join("state.db")
}

fn log_path(dir: &TempDir) -> PathBuf {
    dir.path().join("control-host.log")
}

/// The address of the line `listening <address> callers=<mode>`.
fn parse_listening(line: &str) -> Option<SocketAddr> {
    let mut words = line.split_whitespace();
    (words.next() == Some("listening"))
        .then(|| words.next())
        .flatten()
        .and_then(|address| address.parse().ok())
}

/// Writes `rows` into the state file at `state` through the storage port.
pub fn seed(state: &Path, rows: Vec<weft_impl::Row>) {
    let mut ports = weft_impl::Ports::open(state)
        .unwrap_or_else(|error| panic!("cannot open {}: {error}", state.display()));
    for row in rows {
        ports.put_row(row);
    }
}

/// A `weft.accounts.Tenant` row named `name` in state `Active`, with a new identity: the
/// identity and the row.
pub fn tenant_row(name: &str) -> (String, weft_impl::Row) {
    let id = weft_impl::context::mint();
    let row = weft_impl::Row::Tenant(weft_types::accounts::TenantSnapshot {
        state: weft_types::accounts::TenantState::Active,
        data: weft_types::accounts::TenantData {
            tenant_id: weft_types::shared::TenantId(weft_types::primitives::Uuid(id.clone())),
            name: weft_types::accounts::Name(name.to_owned()),
        },
    });
    (id, row)
}
