//! Digests of the inputs a `simulate` run read, recorded in its result (`run.digests`), so that a
//! `replay` of one of its search points can refuse to rerun inputs that changed since the run.
//!
//! The experiment is digested key by key (each top-level key of its YAML, as parsed: comments and
//! layout do not count), so a refusal names the key that changed. The spec is digested file by
//! file: `ess-inputs.yaml` and every file it lists, byte for byte.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn sha256(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut hex = String::with_capacity(7 + 64);
    hex.push_str("sha256:");
    for b in hash {
        hex.push_str(&format!("{b:02x}"));
    }
    hex
}

/// The digest of each top-level key of the experiment file.
pub fn experiment(file: &Path) -> Result<BTreeMap<String, String>, String> {
    let text =
        std::fs::read_to_string(file).map_err(|e| format!("reading {}: {e}", file.display()))?;
    let doc: Value = ess_yaml::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    let map = doc
        .as_object()
        .ok_or_else(|| format!("{}: not a map of keys", file.display()))?;
    Ok(map
        .iter()
        .map(|(k, v)| (k.clone(), sha256(v.to_string().as_bytes())))
        .collect())
}

/// The digest of `ess-inputs.yaml` of the spec directory and of every file it lists.
pub fn spec(dir: &Path) -> Result<BTreeMap<String, String>, String> {
    #[derive(serde::Deserialize)]
    struct Inputs {
        specification: Vec<String>,
    }
    let read = |name: &str| {
        let path = dir.join(name);
        std::fs::read(&path).map_err(|e| format!("reading {}: {e}", path.display()))
    };
    let list = read("ess-inputs.yaml")?;
    let text = String::from_utf8_lossy(&list);
    let inputs: Inputs = ess_yaml::from_str(&text)
        .map_err(|e| format!("{}: {e}", dir.join("ess-inputs.yaml").display()))?;
    let mut out = BTreeMap::new();
    out.insert("ess-inputs.yaml".to_owned(), sha256(&list));
    for file in &inputs.specification {
        out.insert(file.clone(), sha256(&read(file)?));
    }
    Ok(out)
}

/// The value of `run.digests`: `{"experiment": {key: digest}, "spec": {file: digest}}`.
pub fn record(spec_dir: &Path, experiment_file: &Path) -> Result<Value, String> {
    Ok(json!({"experiment": experiment(experiment_file)?, "spec": spec(spec_dir)?}))
}

/// What differs between the digests a run recorded and the inputs as they are now, one line each;
/// empty when nothing does.
pub fn changed(
    recorded: &Value,
    spec_dir: &Path,
    experiment_file: &Path,
) -> Result<Vec<String>, String> {
    let now = record(spec_dir, experiment_file)?;
    let mut lines = Vec::new();
    for (part, what, place) in [
        ("experiment", "key", experiment_file.display().to_string()),
        ("spec", "file", spec_dir.display().to_string()),
    ] {
        let empty = serde_json::Map::new();
        let was = recorded[part].as_object().unwrap_or(&empty);
        let is = now[part].as_object().unwrap_or(&empty);
        let names: std::collections::BTreeSet<&String> = was.keys().chain(is.keys()).collect();
        for name in names {
            let line = match (was.get(name), is.get(name)) {
                (Some(a), Some(b)) if a == b => continue,
                (Some(_), Some(_)) => "changed since the run",
                (Some(_), None) => "removed since the run",
                (None, _) => "added since the run",
            };
            lines.push(format!("{part} {what} `{name}` of {place}: {line}"));
        }
    }
    Ok(lines)
}
