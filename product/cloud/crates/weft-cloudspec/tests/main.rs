//! The one integration-test binary of weft-cloudspec: cloudspec's three weft goldens, committed in
//! `tests/fixtures/cloudspec/` with `goldens.lock`, are read offline (no network, no cloudspec
//! binary, no process).

mod accept;
mod goldens;
mod refuse;

use std::path::PathBuf;

use serde_json::Value;

/// The root of the repository.
pub fn root() -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.ancestors()
        .nth(4)
        .expect("the crate lies at product/cloud/crates/<crate>")
        .to_path_buf()
}

/// The directory of the committed goldens.
pub fn fixtures() -> PathBuf {
    root().join("tests/fixtures/cloudspec")
}

/// The bytes of one golden, by its short name (`snippet`, `workstation`, `host-daemon`).
pub fn golden(name: &str) -> Vec<u8> {
    let path = fixtures().join(format!("{name}.submission.json"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// One golden as a JSON value.
pub fn golden_value(name: &str) -> Value {
    serde_json::from_slice(&golden(name)).expect("a golden is JSON")
}

/// A value written back as bytes, as a file would hold it.
pub fn bytes(v: &Value) -> Vec<u8> {
    serde_json::to_vec_pretty(v).expect("a JSON value serializes")
}

/// A directory of its own under the repository's `target/`, emptied.
pub fn scratch(name: &str) -> PathBuf {
    let dir = root().join("target/weft-cloudspec-tests").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}
