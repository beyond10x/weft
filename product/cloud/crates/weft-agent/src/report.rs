//! The report file of `weft-agent --report <path>`: one JSON line per event the tests read, each
//! written whole and flushed before the agent goes on. This story writes the answer to each
//! `hello`: `{"hello": "opened", "session_id": "..."}` or `{"hello": "refused", "status": <http
//! status>}`.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use serde_json::Value;

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
}
