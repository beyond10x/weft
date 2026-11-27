//! Helpers shared by the tests of weft's crates.

use std::path::{Path, PathBuf};

/// The repository root: the directory of the root workspace manifest.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("product/<product>/crates/<name> lies four levels below the root")
        .to_path_buf()
}
