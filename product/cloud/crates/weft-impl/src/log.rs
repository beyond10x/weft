//! The control host log: one JSON line per answered request, per published event and per binding
//! answer, each with a `seq` that counts up from 1 over the life of the state file. A reader
//! takes the highest `seq` before an act and then only the lines after it.
//!
//! ```text
//! {"seq": 1, "kind": "request", "operation": "weft.accounts.CreateTenant", "actor": "weft.actors.ClusterOperator", "status": 202}
//! {"seq": 2, "kind": "event", "name": "weft.accounts.TenantCreated"}
//! {"seq": 3, "kind": "binding", "binding": "r1-create-replica", "refusal": "...", "result": "retried", "attempt": 1}
//! ```
//!
//! Each line is written first to the state file (table `weft.impl.Log`), in the transaction of
//! the command it records when one is open ([`crate::storage::Store::transaction`]), so a
//! command's rows and its lines commit together. The log file is their copy: a line reaches it
//! once its transaction commits ([`Log::flush`]). On open the file is brought up to the state
//! file: an incomplete last line (a write cut off) is dropped with a note, and the lines the file
//! lacks are appended. A log file that goes past the state file belongs to another one and is
//! refused. The log file is locked (`flock`) for the life of the [`Log`].

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::Mutex;

use serde_json::{json, Value};

use crate::Ports;

/// What became of a binding's attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingResult {
    /// The invoked command took effect.
    Applied,
    /// The event was dropped.
    Dropped,
    /// The event is held for another attempt.
    Retried,
}

impl BindingResult {
    fn name(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Dropped => "dropped",
            Self::Retried => "retried",
        }
    }
}

/// The log of the state file behind `ports`, copied to one file.
pub struct Log {
    ports: Ports,
    inner: Mutex<Inner>,
}

struct Inner {
    file: File,
    /// Lines committed or about to commit that the file does not hold yet.
    pending: Vec<String>,
}

impl Log {
    /// Opens the log file at `path` (created when absent) for the state file behind `ports`, and
    /// brings it up to the state file: the log and a note for each repair made.
    ///
    /// # Errors
    ///
    /// What the file system refuses; a log file another control host holds; a last complete
    /// line that is not a log line; a log file that goes past the state file.
    pub fn open(path: &Path, ports: &Ports) -> io::Result<(Self, Vec<String>)> {
        let invalid = |message: String| io::Error::new(io::ErrorKind::InvalidData, message);
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(path)?;
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!("{} is held by another control host", path.display()),
                ))
            }
            Err(TryLockError::Error(error)) => return Err(error),
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let mut notes = Vec::new();
        let complete = bytes
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |at| at + 1);
        if complete < bytes.len() {
            notes.push(format!(
                "{}: dropped an incomplete last line, a write cut off: {}",
                path.display(),
                String::from_utf8_lossy(&bytes[complete..])
            ));
            file.set_len(complete as u64)?;
        }
        let text = String::from_utf8_lossy(&bytes[..complete]);
        let in_file = match text.lines().rev().find(|line| !line.trim().is_empty()) {
            Some(line) => serde_json::from_str::<Value>(line)
                .ok()
                .and_then(|value| value["seq"].as_u64())
                .ok_or_else(|| {
                    invalid(format!(
                        "{}: the last line is not a log line: {line}",
                        path.display()
                    ))
                })?,
            None => 0,
        };
        let (in_state, missing) = ports.store().log_after(in_file);
        if in_file > in_state {
            return Err(invalid(format!(
                "{}: the log goes to seq {in_file}, past the last line of its state file ({in_state}): it is the log of another state file",
                path.display()
            )));
        }
        if !missing.is_empty() {
            notes.push(format!(
                "{}: appended {} line(s) the state file holds and the log lacked",
                path.display(),
                missing.len()
            ));
        }
        let log = Self {
            ports: ports.clone(),
            inner: Mutex::new(Inner {
                file,
                pending: missing,
            }),
        };
        log.flush();
        Ok((log, notes))
    }

    /// The `seq` of the last line written, `0` before the first.
    pub fn last_seq(&self) -> u64 {
        self.ports.store().last_log_seq()
    }

    /// Writes the line of an answered request: the operation's qualified name (`None` for a
    /// path that names none), the caller's actor (`None` for no caller) and the status.
    pub fn request(&self, operation: Option<&str>, actor: Option<&str>, status: u16) -> u64 {
        self.write(
            json!({"kind": "request", "operation": operation, "actor": actor, "status": status}),
        )
    }

    /// Writes the line of a published event.
    pub fn event(&self, name: &str) -> u64 {
        self.write(json!({"kind": "event", "name": name}))
    }

    /// Writes the line of a binding's answer: the refusal that stopped it, if any, what became
    /// of the event, and the attempt's number for the event, from 1.
    pub fn binding(
        &self,
        binding: &str,
        refusal: Option<&str>,
        result: BindingResult,
        attempt: u32,
    ) -> u64 {
        let mut line = json!({"kind": "binding", "binding": binding, "result": result.name(), "attempt": attempt});
        if let Some(refusal) = refusal {
            line["refusal"] = json!(refusal);
        }
        self.write(line)
    }

    /// Forgets the lines written in the open transaction, which is about to roll back: they never
    /// reach the file.
    pub fn discard(&self) {
        self.lock().pending.clear();
    }

    /// Copies the committed lines the file lacks to it. Called after a transaction commits; a
    /// line written outside one is copied at once.
    ///
    /// # Panics
    ///
    /// When the file cannot be written: the lines stay in the state file, and the next open
    /// appends them.
    pub fn flush(&self) {
        let mut inner = self.lock();
        if inner.pending.is_empty() {
            return;
        }
        let mut text = String::new();
        for line in inner.pending.drain(..) {
            text.push_str(&line);
            text.push('\n');
        }
        inner
            .file
            .write_all(text.as_bytes())
            .unwrap_or_else(|error| panic!("control host log: cannot write: {error}"));
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write(&self, line: Value) -> u64 {
        let store = self.ports.store();
        let (seq, text) = store.append_log(|seq| {
            let mut members = serde_json::Map::new();
            members.insert("seq".to_owned(), json!(seq));
            if let Value::Object(rest) = line {
                members.extend(rest);
            }
            Value::Object(members).to_string()
        });
        self.lock().pending.push(text);
        if !store.in_transaction() {
            self.flush();
        }
        seq
    }
}
