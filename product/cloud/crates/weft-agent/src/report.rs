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
//!   its exit code (`null` when a signal ended it) and its output as text ([`Report::exited`]);
//! - a start of a process refused and not made again: `{"instance_id": "...", "refusal":
//!   "<code>"}`, the place's code or the exec port's, or the `workloads` port's for a process
//!   workload ([`Report::refused`]);
//! - for a process workload run through the `workloads` port (`story:slot-node-process`), each
//!   with `"process": "<name>"`: a start that did not reach running, `"failed": "prestart"` with
//!   `step` and `exit_code` or `"failed": "command"` with `reason` ([`Report::process_failed`]);
//!   an exit, `exit_code` (`null` for a signal) and `kept_down` ([`Report::process_exited`]); a
//!   restart, `"restarted": <n>` ([`Report::process_restarted`]); a stop, `"stopped":
//!   "within-grace" | "killed" | "not-running"` ([`Report::process_stopped`]).

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use serde_json::{json, Value};
use weft_platform::node::{Exited, ProcessExit, StartFailure, StopEnd};

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

    /// The exec port refused a start of a process of `instance_id` with `code`, and the agent
    /// does not start it again.
    ///
    /// # Errors
    ///
    /// What the file system refuses.
    pub fn refused(&mut self, instance_id: &str, code: &str) -> io::Result<()> {
        self.line(&json!({"instance_id": instance_id, "refusal": code}))
    }

    /// The start of `process` of `instance_id` through the `workloads` port did not reach
    /// `running`: a prestart step ended other than with code 0, or the command did not start.
    ///
    /// # Errors
    ///
    /// What the file system refuses.
    pub fn process_failed(
        &mut self,
        instance_id: &str,
        process: &str,
        failure: &StartFailure,
    ) -> io::Result<()> {
        self.line(&match failure {
            StartFailure::Prestart { step, exit } => json!({
                "instance_id": instance_id,
                "process": process,
                "failed": "prestart",
                "step": step,
                "exit_code": code(*exit),
            }),
            StartFailure::Command { reason } => json!({
                "instance_id": instance_id,
                "process": process,
                "failed": "command",
                "reason": reason,
            }),
        })
    }

    /// `process` of `instance_id` ended on its own with `exit`; `kept_down`: it is not started
    /// again (the restart policy, or a code in `keep_down_on_exit`).
    ///
    /// # Errors
    ///
    /// What the file system refuses.
    pub fn process_exited(
        &mut self,
        instance_id: &str,
        process: &str,
        exit: ProcessExit,
        kept_down: bool,
    ) -> io::Result<()> {
        self.line(&json!({
            "instance_id": instance_id,
            "process": process,
            "exit_code": code(exit),
            "kept_down": kept_down,
        }))
    }

    /// The agent started `process` of `instance_id` again, its `restarts`-th restart.
    ///
    /// # Errors
    ///
    /// What the file system refuses.
    pub fn process_restarted(
        &mut self,
        instance_id: &str,
        process: &str,
        restarts: u32,
    ) -> io::Result<()> {
        self.line(&json!({"instance_id": instance_id, "process": process, "restarted": restarts}))
    }

    /// The node's records (`weft.node`) refused `record` (`StartProcess`, `RestartProcess`,
    /// `StopProcess`) of `process` of `instance_id`, or could not be reached, as `refusal` says;
    /// the process runs either way.
    ///
    /// # Errors
    ///
    /// What the file system refuses.
    pub fn record_refused(
        &mut self,
        instance_id: &str,
        process: &str,
        record: &str,
        refusal: &str,
    ) -> io::Result<()> {
        self.line(&json!({
            "instance_id": instance_id,
            "process": process,
            "record": record,
            "refused": refusal,
        }))
    }

    /// The agent stopped `process` of `instance_id`, which ended as `end` says.
    ///
    /// # Errors
    ///
    /// What the file system refuses.
    pub fn process_stopped(
        &mut self,
        instance_id: &str,
        process: &str,
        end: Option<StopEnd>,
    ) -> io::Result<()> {
        let end = end.map(|end| match end {
            StopEnd::WithinGrace => "within-grace",
            StopEnd::Killed(_) => "killed",
            StopEnd::NotRunning => "not-running",
        });
        self.line(&json!({"instance_id": instance_id, "process": process, "stopped": end}))
    }
}

/// The exit code of `exit`; `None` when a signal ended the process.
fn code(exit: ProcessExit) -> Option<u8> {
    match exit {
        ProcessExit::Code(code) => Some(code),
        ProcessExit::Signal => None,
    }
}
