//! The report file of `weft-agent --report <path>`: one JSON line per event the tests read, each
//! written whole and flushed before the agent goes on. The lines:
//!
//! - the answer to each `hello`: `{"hello": "opened", "session_id": "..."}` or
//!   `{"hello": "refused", "status": <http status>}`;
//! - a start: `{"started": "<instance_id>", "pass": <n>}`, `n` the work-list pass that started
//!   it ([`Report::started`]);
//! - the end of a pass over the work list: `{"pass": <n>}`, `n` counting from 1
//!   ([`Report::pass`]);
//! - a finished process: `{"instance_id": "...", "exit_code": <code or null>, "stdout": "..."}`,
//!   its exit code (`null` when a signal ended it) and its output as text ([`Report::exited`]).

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use serde_json::{json, Value};
use weft_platform::node::Exited;

/// An open report file, appended to.
#[derive(Debug)]
pub struct Report {
    file: File,
}

impl Report {
    /// Opens the report file at `path` for appending, creating it when absent.
    ///
    /// # Errors
    ///
    /// What the file system refuses.
    pub fn open(path: &Path) -> io::Result<Self> {
        Ok(Self {
            file: OpenOptions::new().create(true).append(true).open(path)?,
        })
    }

    /// Writes `line` as one line.
    ///
    /// # Errors
    ///
    /// What the file system refuses.
    pub fn line(&mut self, line: &Value) -> io::Result<()> {
        let mut text = line.to_string();
        text.push('\n');
        self.file.write_all(text.as_bytes())?;
        self.file.flush()
    }

    /// The agent started the processes of `instance_id` in pass `pass`.
    ///
    /// # Errors
    ///
    /// What the file system refuses.
    pub fn started(&mut self, instance_id: &str, pass: u64) -> io::Result<()> {
        self.line(&json!({"started": instance_id, "pass": pass}))
    }

    /// The agent finished pass `pass` over its work list.
    ///
    /// # Errors
    ///
    /// What the file system refuses.
    pub fn pass(&mut self, pass: u64) -> io::Result<()> {
        self.line(&json!({"pass": pass}))
    }

    /// A process of `instance_id` ended as `exited`.
    ///
    /// # Errors
    ///
    /// What the file system refuses.
    pub fn exited(&mut self, instance_id: &str, exited: &Exited) -> io::Result<()> {
        self.line(&json!({
            "instance_id": instance_id,
            "exit_code": exited.code,
            "stdout": String::from_utf8_lossy(&exited.stdout),
        }))
    }
}
