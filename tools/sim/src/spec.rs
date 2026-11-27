//! Compiles a specification directory the way `ess specify compile --path <dir>` does: the files
//! `ess-inputs.yaml` lists, parsed together, assembled and resolved.

use std::path::Path;

use ess_compiler::source::SourceMap;
use ess_compiler::EssIr;

#[derive(serde::Deserialize)]
struct Inputs {
    specification: Vec<String>,
}

pub fn load(dir: &Path) -> Result<EssIr, String> {
    load_without(dir, &[])
}

/// As [`load`], with the named bindings left out (an experiment's `lost_bindings`). A name no
/// file declares is an error, so a typo does not run the full model unnoticed.
pub fn load_without(dir: &Path, lost: &[String]) -> Result<EssIr, String> {
    load_with(dir, lost, None).map(|(ir, _)| ir)
}

/// As [`load_without`]; with `retry_bound`, every binding that retries without a bound retries at
/// most that many times (the first attempt included). Returns the names of the bindings bounded.
///
/// The interpreter retries an unbounded binding up to 65,536 times in one step; on a store a setup
/// built, each attempt is a full command decision, and one such step took a minute (weft-sim
/// explore --from, 2026-11-26). The bound keeps a walk going; a binding that reaches it is a
/// finding of the run, not a silent drop.
pub fn load_with(
    dir: &Path,
    lost: &[String],
    retry_bound: Option<u32>,
) -> Result<(EssIr, Vec<String>), String> {
    let inputs_path = dir.join("ess-inputs.yaml");
    let text = std::fs::read_to_string(&inputs_path)
        .map_err(|e| format!("reading {}: {e}", inputs_path.display()))?;
    let inputs: Inputs =
        ess_yaml::from_str(&text).map_err(|e| format!("{}: {e}", inputs_path.display()))?;
    let mut texts = Vec::new();
    for file in &inputs.specification {
        let path = dir.join(file);
        let body = std::fs::read_to_string(&path)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        texts.push((file.clone(), body));
    }
    let every: Vec<&str> = texts.iter().map(|(_, t)| t.as_str()).collect();
    let results = ess_domain::spec::RawSpecFile::parse_all(&every);
    let mut parsed = Vec::new();
    let mut sources = SourceMap::new();
    let mut problems = Vec::new();
    for ((name, body), result) in texts.iter().zip(results) {
        let source = ess_domain::system::Source::new(name.clone());
        sources.insert(source.as_str(), body.as_str());
        match result {
            Ok(raw) => parsed.push((source, raw)),
            Err(error) => problems.push(format!("{name}: {error}")),
        }
    }
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    let mut found = 0;
    for (_, raw) in &mut parsed {
        let before = raw.bindings.len();
        raw.bindings.retain(|b| !lost.contains(&b.name));
        found += before - raw.bindings.len();
    }
    if found != lost.len() {
        return Err(format!(
            "lost_bindings {lost:?}: {found} of them are declared"
        ));
    }
    let mut bounded = Vec::new();
    if let Some(attempts) = retry_bound {
        use ess_domain::binding::Failure;
        for (_, raw) in &mut parsed {
            for b in &mut raw.bindings {
                let f = &mut b.on_failure;
                let mut changed = false;
                if let Some(selected) = &mut f.selected {
                    for entry in &mut selected.entries {
                        if entry.policy == Failure::Retry && entry.attempts.is_none() {
                            entry.attempts = Some(attempts);
                            changed = true;
                        }
                    }
                }
                if f.failure == Failure::Retry && f.retry.is_none() || changed && f.retry.is_none()
                {
                    f.retry = Some(ess_domain::binding::retry::RetryBound {
                        attempts,
                        finals: Vec::new(),
                    });
                    changed = true;
                }
                if changed {
                    bounded.push(b.name.clone());
                }
            }
        }
    }
    let assembled = ess_domain::spec::Specification::assemble_traced(parsed).map_err(|traced| {
        traced
            .errors
            .as_slice()
            .iter()
            .take(20)
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let ir = ess_compiler::compile(&assembled, &sources).map_err(|d| {
        let text = format!("{d:?}");
        text.chars().take(4000).collect::<String>()
    })?;
    Ok((ir, bounded))
}
