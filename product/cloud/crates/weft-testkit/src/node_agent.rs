//! `weft-agent` as a child process beside a [`crate::ControlHost`]: started with the control
//! host's address, a node id and its join token, the substrate socket of the test run and a
//! report file in a temporary directory of its own; stopped on drop.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::{profile_dir, ControlHost, TempDir};

/// The variable that names the substrate-daemon socket of the test run (root
/// `.cargo/config.toml`, `tools/test-setup.d/20-substrate.sh`).
pub const SOCKET_VARIABLE: &str = "WEFT_SUBSTRATE_SOCKET";

/// The `weft-agent` binary of the test build: `WEFT_AGENT` when set, else `weft-agent` in the
/// test binary's profile directory, which a workspace test build makes (`weft-agent` has
/// integration tests, so cargo builds its binaries).
pub fn node_agent_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("WEFT_AGENT") {
        return PathBuf::from(path);
    }
    let path = profile_dir().join("weft-agent");
    assert!(
        path.is_file(),
        "no node agent binary at {}: build the workspace tests first (cargo nextest run --workspace --no-run)",
        path.display()
    );
    path
}

/// The substrate-daemon socket of the test run.
pub fn substrate_socket() -> PathBuf {
    let path = std::env::var_os(SOCKET_VARIABLE).unwrap_or_else(|| {
        panic!("{SOCKET_VARIABLE} is unset: the setup hook tools/test-setup.d/20-substrate.sh starts the daemon")
    });
    PathBuf::from(path)
}

/// A running `weft-agent`.
#[derive(Debug)]
pub struct NodeAgent {
    child: Child,
    report: PathBuf,
    _dir: TempDir,
}

impl NodeAgent {
    /// Starts `weft-agent` for the node `node_id` with `join_token`, dialing `host`.
    pub fn start(host: &ControlHost, node_id: &str, join_token: &str) -> Self {
        let dir = TempDir::new();
        let report = dir.path().join("report.jsonl");
        let binary = node_agent_binary();
        let child = Command::new(&binary)
            .arg("--control")
            .arg(format!("http://{}", host.address()))
            .arg("--node-id")
            .arg(node_id)
            .arg("--join-token")
            .arg(join_token)
            .arg("--substrate")
            .arg(substrate_socket())
            .arg("--report")
            .arg(&report)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap_or_else(|error| panic!("cannot start {}: {error}", binary.display()));
        Self {
            child,
            report,
            _dir: dir,
        }
    }

    /// The lines of the report file so far, each a JSON value; a line still being written is
    /// left out.
    pub fn report(&self) -> Vec<Value> {
        let text = std::fs::read_to_string(&self.report).unwrap_or_default();
        let complete = text.rfind('\n').map_or("", |end| &text[..end]);
        complete
            .lines()
            .map(|line| {
                serde_json::from_str(line)
                    .unwrap_or_else(|error| panic!("not a report line ({error}): {line}"))
            })
            .collect()
    }

    /// Waits until the report lines satisfy `done`, at most `limit`: the lines then. Panics when
    /// the limit passes or the agent ends first.
    pub fn wait_for(&mut self, limit: Duration, done: impl Fn(&[Value]) -> bool) -> Vec<Value> {
        let start = Instant::now();
        loop {
            let lines = self.report();
            if done(&lines) {
                return lines;
            }
            if let Ok(Some(status)) = self.child.try_wait() {
                panic!("weft-agent ended ({status}) before its report was complete: {lines:?}");
            }
            assert!(
                start.elapsed() < limit,
                "weft-agent's report is not complete after {limit:?}: {lines:?}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for NodeAgent {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
