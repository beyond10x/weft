//! The storage port on SQLite (decision A4): one database file, one table per entity the
//! generated behaviours read or write, named by the entity's qualified name. A row is the entity's
//! snapshot (state and data) in the form of [`codec`], keyed by the text of its identity; `list`
//! answers in the bytes' order of that text, the order the generated memory store keeps
//! (`MemoryKey::Text`, a `String`: `build.rs` refuses an entity the memory store orders by any
//! other key).
//!
//! One writer: [`Store::open`] holds an exclusive lock (`flock`) on `<state>.lock` for the life
//! of the store and refuses a state file another store holds. The control host log's lines live
//! in the same file (table `weft.impl.Log`, see [`crate::log`]), so a command's rows and its log
//! lines commit in one [`Store::transaction`], and so do the events the hand-run bindings hold
//! for another attempt (table `weft.impl.Held`, see [`crate::bindings`]) and the waits of the
//! controllers' steps (table `weft.impl.StepWait`, [`StepWait`]).
//!
//! The port's methods cannot fail (`weft_types::behaviour::…Storage`), so a state file SQLite
//! cannot read or write ends the process with a panic naming the table: a control host whose
//! store fails must not answer as if it had written. An open transaction is rolled back then.

pub mod codec;

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rusqlite::{params, Connection, OptionalExtension};

use codec::Codec;

/// Why a state file could not be opened.
#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// The table beside the nodes that keeps the SHA-256 of each node's join token, never the token
/// (E1, restored by hand: see `src/behaviours/provision_pool_node.rs`).
const JOIN_TOKENS: &str = "weft.impl.JoinTokenDigest";

/// The table of the control host log's lines, by `seq`.
const LOG: &str = "weft.impl.Log";

/// The table of the events the hand-run bindings hold for another attempt, in the order held.
const HELD: &str = "weft.impl.Held";

/// The table of the waits of the controllers' steps: per step and subject, the attempts that
/// failed and the ticks until the next ([`StepWait`]).
const STEP_WAITS: &str = "weft.impl.StepWait";

/// The state file.
pub struct Store {
    connection: Mutex<Connection>,
    /// The locked `<state>.lock`, held until the store is dropped.
    _lock: File,
}

impl Store {
    /// Opens the state file at `path`, creating it and every table it lacks, and takes its lock.
    ///
    /// # Errors
    ///
    /// A state file another store holds, or what SQLite or the file system refuses.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let lock = lock(path)?;
        let fail =
            |error: rusqlite::Error| Error(format!("state file {}: {error}", path.display()));
        let connection = Connection::open(path).map_err(fail)?;
        connection
            .execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA busy_timeout = 5000;")
            .map_err(fail)?;
        let mut schema = String::from("BEGIN;\n");
        for entity in crate::ENTITIES
            .iter()
            .chain([&JOIN_TOKENS, &crate::documents::TABLE])
        {
            schema.push_str(&format!(
                "CREATE TABLE IF NOT EXISTS {} (key TEXT COLLATE BINARY PRIMARY KEY NOT NULL, row TEXT NOT NULL) WITHOUT ROWID;\n",
                table(entity)
            ));
        }
        schema.push_str(&format!(
            "CREATE TABLE IF NOT EXISTS {} (seq INTEGER PRIMARY KEY NOT NULL, line TEXT NOT NULL);\nCREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY NOT NULL, binding TEXT NOT NULL, event TEXT NOT NULL, attempts INTEGER NOT NULL, wait INTEGER NOT NULL);\nCREATE TABLE IF NOT EXISTS {} (step TEXT NOT NULL, subject TEXT NOT NULL, attempts INTEGER NOT NULL, wait INTEGER NOT NULL, PRIMARY KEY (step, subject)) WITHOUT ROWID;\nCOMMIT;",
            table(LOG),
            table(HELD),
            table(STEP_WAITS)
        ));
        connection.execute_batch(&schema).map_err(fail)?;
        Ok(Self {
            connection: Mutex::new(connection),
            _lock: lock,
        })
    }

    fn connection(&self) -> MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn execute(&self, sql: &str) {
        self.connection()
            .execute_batch(sql)
            .unwrap_or_else(|error| panic!("state file: {sql} failed: {error}"));
    }

    /// Whether a [`Store::transaction`] is open.
    pub fn in_transaction(&self) -> bool {
        !self.connection().is_autocommit()
    }

    /// Runs `work` in one transaction (a savepoint, nested in an open one): every row it writes,
    /// and every log line, commits together when it answers `true` beside its value, and none
    /// does when it answers `false` or panics.
    pub fn transaction<T>(&self, work: impl FnOnce() -> (T, bool)) -> T {
        self.execute("SAVEPOINT work");
        let open = Open(self);
        let (value, commit) = work();
        std::mem::forget(open);
        if commit {
            self.execute("RELEASE work");
        } else {
            self.execute("ROLLBACK TO work; RELEASE work");
        }
        value
    }

    /// The row of `entity` keyed `key`.
    pub fn get<K: Codec, R: Codec>(&self, entity: &str, key: &K) -> Option<R> {
        let text: Option<String> = self
            .connection()
            .prepare_cached(&format!("SELECT row FROM {} WHERE key = ?1", table(entity)))
            .and_then(|mut statement| {
                statement
                    .query_row(params![key_text(key)], |row| row.get(0))
                    .optional()
            })
            .unwrap_or_else(|error| failed(entity, "read", &error));
        text.map(|text| decode(entity, &text))
    }

    /// Stores `row` of `entity` under `key`, replacing what was held.
    pub fn put<K: Codec, R: Codec>(&self, entity: &str, key: &K, row: &R) {
        self.connection()
            .prepare_cached(&format!(
                "INSERT INTO {} (key, row) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET row = excluded.row",
                table(entity)
            ))
            .and_then(|mut statement| statement.execute(params![key_text(key), row.encode().to_string()]))
            .unwrap_or_else(|error| failed(entity, "write", &error));
    }

    /// Removes the row of `entity` keyed `key`.
    pub fn delete<K: Codec>(&self, entity: &str, key: &K) {
        self.connection()
            .prepare_cached(&format!("DELETE FROM {} WHERE key = ?1", table(entity)))
            .and_then(|mut statement| statement.execute(params![key_text(key)]))
            .unwrap_or_else(|error| failed(entity, "delete", &error));
    }

    /// Every row of `entity`, in key order.
    pub fn list<R: Codec>(&self, entity: &str) -> Vec<R> {
        let texts: Vec<String> = self
            .connection()
            .prepare_cached(&format!("SELECT row FROM {} ORDER BY key", table(entity)))
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get(0))?
                    .collect::<Result<Vec<String>, _>>()
            })
            .unwrap_or_else(|error| failed(entity, "list", &error));
        texts.iter().map(|text| decode(entity, text)).collect()
    }

    /// Keeps `digest`, the SHA-256 of the join token issued for `node`, replacing an earlier one.
    pub fn set_join_token_digest(&self, node: &weft_types::shared::NodeId, digest: &str) {
        self.put(JOIN_TOKENS, node, &digest.to_owned());
    }

    /// The SHA-256 of the join token issued for `node`, where one was issued.
    pub fn join_token_digest(&self, node: &weft_types::shared::NodeId) -> Option<String> {
        self.get(JOIN_TOKENS, node)
    }

    /// Appends the log line `line(seq)` under the next `seq`, in the open transaction if any:
    /// the `seq` and the line's text.
    pub fn append_log(&self, line: impl FnOnce(u64) -> String) -> (u64, String) {
        let connection = self.connection();
        let last: i64 = connection
            .prepare_cached(&format!("SELECT COALESCE(MAX(seq), 0) FROM {}", table(LOG)))
            .and_then(|mut statement| statement.query_row([], |row| row.get(0)))
            .unwrap_or_else(|error| failed(LOG, "read", &error));
        let seq = u64::try_from(last).unwrap_or(0) + 1;
        let text = line(seq);
        connection
            .prepare_cached(&format!(
                "INSERT INTO {} (seq, line) VALUES (?1, ?2)",
                table(LOG)
            ))
            .and_then(|mut statement| statement.execute(params![last + 1, text]))
            .unwrap_or_else(|error| failed(LOG, "write", &error));
        (seq, text)
    }

    /// The `seq` of the last log line, `0` before the first.
    pub fn last_log_seq(&self) -> u64 {
        self.log_after(u64::MAX).0
    }

    /// The log lines after `seq`, in order, and the last `seq`.
    pub fn log_after(&self, seq: u64) -> (u64, Vec<String>) {
        let connection = self.connection();
        let last: i64 = connection
            .prepare_cached(&format!("SELECT COALESCE(MAX(seq), 0) FROM {}", table(LOG)))
            .and_then(|mut statement| statement.query_row([], |row| row.get(0)))
            .unwrap_or_else(|error| failed(LOG, "read", &error));
        let from = i64::try_from(seq).unwrap_or(i64::MAX);
        let lines = connection
            .prepare_cached(&format!(
                "SELECT line FROM {} WHERE seq > ?1 ORDER BY seq",
                table(LOG)
            ))
            .and_then(|mut statement| {
                statement
                    .query_map(params![from], |row| row.get(0))?
                    .collect::<Result<Vec<String>, _>>()
            })
            .unwrap_or_else(|error| failed(LOG, "read", &error));
        (u64::try_from(last).unwrap_or(0), lines)
    }

    /// Every held event, in the order held.
    pub fn held(&self) -> Vec<Held> {
        self.connection()
            .prepare_cached(&format!(
                "SELECT id, binding, event, attempts, wait FROM {} ORDER BY id",
                table(HELD)
            ))
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| {
                        Ok(Held {
                            id: row.get(0)?,
                            binding: row.get(1)?,
                            event: row.get(2)?,
                            attempts: row.get(3)?,
                            wait: row.get(4)?,
                        })
                    })?
                    .collect::<Result<Vec<Held>, _>>()
            })
            .unwrap_or_else(|error| failed(HELD, "list", &error))
    }

    /// Holds `event` (its text) of `binding` after `attempts` attempts, to be attempted again
    /// `wait` ticks on, in the open transaction if any.
    pub fn hold(&self, binding: &str, event: &str, attempts: u32, wait: u32) {
        self.connection()
            .prepare_cached(&format!(
                "INSERT INTO {} (binding, event, attempts, wait) VALUES (?1, ?2, ?3, ?4)",
                table(HELD)
            ))
            .and_then(|mut statement| statement.execute(params![binding, event, attempts, wait]))
            .unwrap_or_else(|error| failed(HELD, "write", &error));
    }

    /// Sets the attempts and the wait of the held event `id`.
    pub fn set_held(&self, id: i64, attempts: u32, wait: u32) {
        self.connection()
            .prepare_cached(&format!(
                "UPDATE {} SET attempts = ?2, wait = ?3 WHERE id = ?1",
                table(HELD)
            ))
            .and_then(|mut statement| statement.execute(params![id, attempts, wait]))
            .unwrap_or_else(|error| failed(HELD, "write", &error));
    }

    /// Lets go of the held event `id`.
    pub fn release(&self, id: i64) {
        self.connection()
            .prepare_cached(&format!("DELETE FROM {} WHERE id = ?1", table(HELD)))
            .and_then(|mut statement| statement.execute(params![id]))
            .unwrap_or_else(|error| failed(HELD, "delete", &error));
    }

    /// Makes every held event due at the next tick: the wait of each is 1.
    pub fn make_held_due(&self) {
        self.connection()
            .prepare_cached(&format!("UPDATE {} SET wait = 1", table(HELD)))
            .and_then(|mut statement| statement.execute([]))
            .unwrap_or_else(|error| failed(HELD, "write", &error));
    }

    /// The waits of the step `step`, by subject.
    pub fn step_waits(&self, step: &str) -> Vec<StepWait> {
        self.connection()
            .prepare_cached(&format!(
                "SELECT subject, attempts, wait FROM {} WHERE step = ?1 ORDER BY subject",
                table(STEP_WAITS)
            ))
            .and_then(|mut statement| {
                statement
                    .query_map(params![step], |row| {
                        Ok(StepWait {
                            subject: row.get(0)?,
                            attempts: row.get(1)?,
                            wait: row.get(2)?,
                        })
                    })?
                    .collect::<Result<Vec<StepWait>, _>>()
            })
            .unwrap_or_else(|error| failed(STEP_WAITS, "list", &error))
    }

    /// Sets the wait of `subject` of the step `step`: `attempts` failed, the next `wait` ticks on,
    /// in the open transaction if any.
    pub fn set_step_wait(&self, step: &str, subject: &str, attempts: u32, wait: u32) {
        self.connection()
            .prepare_cached(&format!(
                "INSERT INTO {} (step, subject, attempts, wait) VALUES (?1, ?2, ?3, ?4) ON CONFLICT (step, subject) DO UPDATE SET attempts = excluded.attempts, wait = excluded.wait",
                table(STEP_WAITS)
            ))
            .and_then(|mut statement| statement.execute(params![step, subject, attempts, wait]))
            .unwrap_or_else(|error| failed(STEP_WAITS, "write", &error));
    }

    /// Lets go of the wait of `subject` of the step `step`.
    pub fn clear_step_wait(&self, step: &str, subject: &str) {
        self.connection()
            .prepare_cached(&format!(
                "DELETE FROM {} WHERE step = ?1 AND subject = ?2",
                table(STEP_WAITS)
            ))
            .and_then(|mut statement| statement.execute(params![step, subject]))
            .unwrap_or_else(|error| failed(STEP_WAITS, "delete", &error));
    }
}

/// The wait of one subject of a controllers' step (table `weft.impl.StepWait`): a subject whose
/// command failed is attempted again after a wait that doubles, as a held event is
/// ([`crate::bindings::wait_after`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepWait {
    /// The subject's identity (the row of the step's view).
    pub subject: String,
    /// How many of its attempts failed in a row.
    pub attempts: u32,
    /// The ticks until its next attempt: it is attempted by the tick that finds it at 1.
    pub wait: u32,
}

/// An event a hand-run binding holds for another attempt (table `weft.impl.Held`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// Its place in the order held.
    pub id: i64,
    /// The binding's `id` in `ess/components.yaml`.
    pub binding: String,
    /// The event, in the form of [`codec`].
    pub event: String,
    /// How many times the binding attempted it.
    pub attempts: u32,
    /// The ticks until its next attempt: it is attempted by the tick that finds it at 1.
    pub wait: u32,
}

/// An open transaction, rolled back when dropped before its commit (a panic in its work).
struct Open<'a>(&'a Store);

impl Drop for Open<'_> {
    fn drop(&mut self) {
        let _ = self
            .0
            .connection()
            .execute_batch("ROLLBACK TO work; RELEASE work");
    }
}

/// `<state>.lock`, created when absent and locked exclusively.
fn lock(state: &Path) -> Result<File, Error> {
    let mut path = state.as_os_str().to_owned();
    path.push(".lock");
    let path = PathBuf::from(path);
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|error| Error(format!("state file lock {}: {error}", path.display())))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(Error(format!(
            "state file {} is held by another control host (lock {})",
            state.display(),
            path.display()
        ))),
        Err(TryLockError::Error(error)) => Err(Error(format!(
            "state file lock {}: {error}",
            path.display()
        ))),
    }
}

fn table(entity: &str) -> String {
    format!("\"{}\"", entity.replace('"', "\"\""))
}

/// The text of a key: a key whose form is a string is that string, so SQLite's `BINARY` order
/// is the bytes' order the memory store sorts its `MemoryKey::Text` keys in.
fn key_text<K: Codec>(key: &K) -> String {
    match key.encode() {
        codec::Value::String(text) => text,
        other => other.to_string(),
    }
}

fn decode<R: Codec>(entity: &str, text: &str) -> R {
    let value: codec::Value = serde_json::from_str(text)
        .unwrap_or_else(|error| panic!("state file: a row of {entity} is not JSON: {error}"));
    R::decode(&value)
        .unwrap_or_else(|error| panic!("state file: a row of {entity} does not decode: {error}"))
}

fn failed(entity: &str, what: &str, error: &rusqlite::Error) -> ! {
    panic!("state file: cannot {what} a row of {entity}: {error}")
}
