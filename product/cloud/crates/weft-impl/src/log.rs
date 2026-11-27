//! The control host log: one JSON line per answered request, per published event and per binding
//! answer, each with a `seq` that counts up from 1 over the life of the file (a reopened log goes
//! on from its last line). A reader takes the highest `seq` before an act and then only the
//! lines after it.
//!
//! ```text
//! {"seq": 1, "kind": "request", "operation": "weft.accounts.CreateTenant", "actor": "weft.actors.ClusterOperator", "status": 202}
//! {"seq": 2, "kind": "event", "name": "weft.accounts.TenantCreated"}
//! {"seq": 3, "kind": "binding", "binding": "r1-create-replica", "refusal": "...", "result": "retried"}
//! ```

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::Mutex;

use serde_json::{json, Value};

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

/// The log, appended to by one writer at a time.
#[derive(Debug)]
pub struct Log {
    inner: Mutex<Inner>,
}

#[derive(Debug)]
struct Inner {
    file: File,
    seq: u64,
}

impl Log {
    /// Opens the log at `path` for appending, creating it when absent; the next line's `seq`
    /// follows the file's last one.
    ///
    /// # Errors
    ///
    /// What the file system refuses, or a last line that is not a log line.
    pub fn open(path: &Path) -> io::Result<Self> {
        let seq = match std::fs::read_to_string(path) {
            Ok(text) => match text.lines().rev().find(|line| !line.trim().is_empty()) {
                Some(line) => serde_json::from_str::<Value>(line)
                    .ok()
                    .and_then(|value| value["seq"].as_u64())
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "{}: the last line is not a log line: {line}",
                                path.display()
                            ),
                        )
                    })?,
                None => 0,
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
            Err(error) => return Err(error),
        };
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            inner: Mutex::new(Inner { file, seq }),
        })
    }

    /// The `seq` of the last line written, `0` before the first.
    pub fn last_seq(&self) -> u64 {
        self.lock().seq
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

    /// Writes the line of a binding's answer: the refusal that stopped it, if any, and what
    /// became of the event.
    pub fn binding(&self, binding: &str, refusal: Option<&str>, result: BindingResult) -> u64 {
        let mut line = json!({"kind": "binding", "binding": binding, "result": result.name()});
        if let Some(refusal) = refusal {
            line["refusal"] = json!(refusal);
        }
        self.write(line)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write(&self, line: Value) -> u64 {
        let mut inner = self.lock();
        let seq = inner.seq + 1;
        let mut members = serde_json::Map::new();
        members.insert("seq".to_owned(), json!(seq));
        if let Value::Object(rest) = line {
            members.extend(rest);
        }
        let mut text = Value::Object(members).to_string();
        text.push('\n');
        inner
            .file
            .write_all(text.as_bytes())
            .unwrap_or_else(|error| panic!("control host log: cannot write: {error}"));
        inner.seq = seq;
        seq
    }
}
