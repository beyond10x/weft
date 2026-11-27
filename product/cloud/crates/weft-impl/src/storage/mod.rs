//! The storage port on SQLite (decision A4): one database file, one table per entity the
//! generated behaviours read or write, named by the entity's qualified name. A row is the entity's
//! snapshot (state and data) in the form of [`codec`], keyed by its identity in the same form;
//! `list` answers in key order, the order the generated memory store keeps.
//!
//! The port's methods cannot fail (`weft_types::behaviour::…Storage`), so a state file SQLite
//! cannot read or write ends the process with a panic naming the table: a control host whose
//! store fails must not answer as if it had written.

pub mod codec;

use std::path::Path;
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

/// The state file.
pub struct Store {
    connection: Mutex<Connection>,
}

impl Store {
    /// Opens the state file at `path`, creating it and every table it lacks.
    ///
    /// # Errors
    ///
    /// What SQLite refuses.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let fail =
            |error: rusqlite::Error| Error(format!("state file {}: {error}", path.display()));
        let connection = Connection::open(path).map_err(fail)?;
        connection
            .execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA busy_timeout = 5000;")
            .map_err(fail)?;
        let mut schema = String::from("BEGIN;\n");
        for entity in crate::ENTITIES.iter().chain([&JOIN_TOKENS]) {
            schema.push_str(&format!(
                "CREATE TABLE IF NOT EXISTS {} (key TEXT PRIMARY KEY NOT NULL, row TEXT NOT NULL) WITHOUT ROWID;\n",
                table(entity)
            ));
        }
        schema.push_str("COMMIT;");
        connection.execute_batch(&schema).map_err(fail)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn connection(&self) -> MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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
}

fn table(entity: &str) -> String {
    format!("\"{}\"", entity.replace('"', "\"\""))
}

fn key_text<K: Codec>(key: &K) -> String {
    key.encode().to_string()
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
