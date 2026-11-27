//! A control host in this process: a [`weft_bin::Host`] on its own state file, served on a
//! loopback port by a thread, with no controller loop: a test runs [`weft_bin::Host::tick`]
//! and [`weft_bin::Host::replay`] itself, so it decides when the controllers and the bindings
//! act.

use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use weft_client::Client;

use crate::TempDir;

/// A control host in this process. The state file and the log stay until drop; the serving
/// thread ends with the test process.
pub struct InProcess {
    host: Arc<weft_bin::Host>,
    client: Client,
    dir: TempDir,
}

impl InProcess {
    /// A control host on a new, empty state file.
    pub fn start() -> Self {
        Self::start_seeded(Vec::new())
    }

    /// A control host on a state file that holds `rows`, written through the storage port
    /// before it opens.
    pub fn start_seeded(rows: Vec<weft_impl::Row>) -> Self {
        let dir = TempDir::new();
        let state = dir.path().join("state.db");
        if !rows.is_empty() {
            crate::seed(&state, rows);
        }
        let host = Arc::new(
            weft_bin::Host::open(&state, &dir.path().join("control-host.log"))
                .unwrap_or_else(|error| panic!("the control host opens: {error}")),
        );
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port binds");
        let client = Client::new(listener.local_addr().expect("the listener's address"));
        let serving = Arc::clone(&host);
        std::thread::spawn(move || serving.serve(&listener, true));
        Self { host, client, dir }
    }

    /// The host.
    pub fn host(&self) -> &weft_bin::Host {
        &self.host
    }

    /// A client of the host.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// The ports of the host's system: its rows, as no view shows them.
    pub fn ports(&self) -> weft_impl::Ports {
        self.host.ports().clone()
    }

    /// The control host log.
    pub fn log_path(&self) -> PathBuf {
        self.dir.path().join("control-host.log")
    }

    /// The highest `seq` of the log, `0` before the first line.
    pub fn last_seq(&self) -> u64 {
        self.host.log().last_seq()
    }

    /// The log lines after `seq`, in order.
    pub fn log_after(&self, seq: u64) -> Vec<Value> {
        weft_client::log_after(&self.log_path(), seq).expect("the log reads")
    }
}
