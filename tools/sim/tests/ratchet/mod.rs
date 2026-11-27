//! `quality/ratchet.yaml` against itself, the assessor declarations of `quality/assessors/`, the
//! reference records of `ratchet/references/` and the KPI catalogue (`design:quality-kpis`). Every
//! check here reads files and measures nothing: the measurements run in `task kpis`, never in the
//! test step. The helpers are public so that each `kpi-` story calls them with its own list.
//!
//! The module is `tests/ratchet/mod.rs`, not `tests/ratchet.rs`: cargo builds every `tests/*.rs`
//! as a test binary of its own, and weft-sim has one (`tests/main.rs`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

/// The repository root.
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A YAML file as a JSON value.
pub fn yaml(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    ess_yaml::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// `quality/ratchet.yaml`.
pub fn ratchet_file() -> Value {
    yaml(&root().join("quality/ratchet.yaml"))
}

/// `Taskfile.yml`.
pub fn taskfile() -> Value {
    yaml(&root().join("Taskfile.yml"))
}

/// The outputs each assessor of `dir` declares, by `<name>@<version>`.
pub fn assessor_outputs(dir: &Path) -> BTreeMap<String, BTreeSet<String>> {
    let mut out = BTreeMap::new();
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        let Some(file) = path.file_name().and_then(|f| f.to_str()) else { continue };
        let Some(stem) = file.strip_suffix(".assessor.yaml") else { continue };
        let doc = yaml(&path);
        let key = format!("{}@{}", text(&doc["name"]), text(&doc["version"]));
        assert_eq!(key, stem, "{}: name@version differs from the file name", path.display());
        let outputs = doc["outputs"].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
        out.insert(key, outputs);
    }
    out
}

/// The reference records of `dir` (`<requirement>.json`), by the requirement of the file name.
pub fn reference_records(dir: &Path) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "json") {
            let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&path).unwrap();
            let record = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            out.insert(stem, record);
        }
    }
    out
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn keys(v: &Value) -> BTreeSet<String> {
    v.as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default()
}

/// The requirement a gate entry names: a name, or `{requirement: <name>, ...}`.
fn entry_requirement(entry: &Value) -> String {
    match entry {
        Value::String(s) => s.clone(),
        other => text(&other["requirement"]),
    }
}

/// The measurement a requirement reads: the one it names, or the only one of its assessment.
fn requirement_measurement(ratchet: &Value, requirement: &Value) -> Option<String> {
    if let Some(m) = requirement["measurement"].as_str() {
        return Some(m.to_owned());
    }
    let measurements = keys(&ratchet["assessments"][text(&requirement["assessment"])]["measurements"]);
    (measurements.len() == 1).then(|| measurements.into_iter().next().unwrap())
}

/// Every defect of the ratchet file: an assessment of an undeclared subject, a requirement that
/// names a missing assessment or measurement, a gate entry that names a missing requirement, a
/// measurement of a declared assessor that names an output the assessor does not declare, an
/// `accepted_value` requirement with no reference record, and a record that names another
/// requirement or format. A requirement of `pending` (its name and the story that adopts its first
/// record) may lack its record; an entry of `pending` that has a record, or that is no
/// `accepted_value` requirement, is a defect too, so the list only shrinks. Empty when the file
/// is consistent.
pub fn consistency_defects(
    ratchet: &Value,
    assessors: &BTreeMap<String, BTreeSet<String>>,
    records: &BTreeMap<String, Value>,
    pending: &[(&str, &str)],
) -> Vec<String> {
    let mut defects = Vec::new();
    let subjects = keys(&ratchet["subjects"]);
    let assessments = ratchet["assessments"].as_object().cloned().unwrap_or_default();
    let requirements = ratchet["requirements"].as_object().cloned().unwrap_or_default();
    for (name, a) in &assessments {
        let subject = text(&a["subject"]);
        if !subjects.contains(&subject) {
            defects.push(format!("assessment {name}: subject {subject:?} is not declared"));
        }
        if let Some(outputs) = assessors.get(&text(&a["assessor"])) {
            for (m, measurement) in a["measurements"].as_object().cloned().unwrap_or_default() {
                let output = text(&measurement["output"]);
                if !outputs.contains(&output) {
                    defects.push(format!(
                        "assessment {name}, measurement {m}: output {output:?} is not declared by {}",
                        text(&a["assessor"])
                    ));
                }
            }
        }
    }
    for (name, r) in &requirements {
        let assessment = text(&r["assessment"]);
        let Some(a) = assessments.get(&assessment) else {
            defects.push(format!("requirement {name}: assessment {assessment:?} does not exist"));
            continue;
        };
        match requirement_measurement(ratchet, r) {
            Some(m) if keys(&a["measurements"]).contains(&m) => {}
            Some(m) => defects.push(format!("requirement {name}: measurement {m:?} of {assessment} does not exist")),
            None => {
                defects.push(format!("requirement {name}: names no measurement and {assessment} has not exactly one"))
            }
        }
        let awaited = pending.iter().any(|(p, _)| p == name);
        if r["reference"]["kind"] == "accepted_value" && !records.contains_key(name) && !awaited {
            defects.push(format!("requirement {name}: accepted_value with no record ratchet/references/{name}.json"));
        }
    }
    for (name, story) in pending {
        if requirements.get(*name).is_none_or(|r| r["reference"]["kind"] != "accepted_value") {
            defects.push(format!("pending record {name} ({story}): no accepted_value requirement of that name"));
        }
        if records.contains_key(*name) {
            defects
                .push(format!("pending record {name} ({story}): the record exists, remove it from the pending list"));
        }
    }
    for (gate, g) in ratchet["gates"].as_object().cloned().unwrap_or_default() {
        for entry in g["requires"].as_array().cloned().unwrap_or_default() {
            let r = entry_requirement(&entry);
            if !requirements.contains_key(&r) {
                defects.push(format!("gate {gate}: requirement {r:?} does not exist"));
            }
        }
    }
    for (stem, record) in records {
        if record["format"] != "ratchet-reference/1" || text(&record["requirement"]) != *stem {
            defects.push(format!("ratchet/references/{stem}.json: not a ratchet-reference/1 record of {stem}"));
        }
        if !requirements.contains_key(stem) {
            defects.push(format!("ratchet/references/{stem}.json: requirement {stem} does not exist"));
        }
    }
    defects
}

/// The measurement a KPI id names: the part after the subject prefix (`repo.gate-seconds` ->
/// `gate-seconds`).
pub fn measurement_of(id: &str) -> &str {
    id.split_once('.').map_or(id, |(_, m)| m)
}

/// The ids of `ids` with no measurement of that name in an assessment of `subject`.
pub fn kpis_without_measurement(ratchet: &Value, subject: &str, ids: &[&str]) -> Vec<String> {
    let mut held = BTreeSet::new();
    for a in ratchet["assessments"].as_object().into_iter().flat_map(|o| o.values()) {
        if a["subject"] == subject {
            held.extend(keys(&a["measurements"]));
        }
    }
    ids.iter().filter(|id| !held.contains(measurement_of(id))).map(|id| id.to_string()).collect()
}

/// The ids of `ids` that no requirement listed in `gate` holds: a requirement over an assessment
/// of `subject` that reads the id's measurement.
pub fn kpis_not_in_gate(ratchet: &Value, gate: &str, subject: &str, ids: &[&str]) -> Vec<String> {
    let mut held = BTreeSet::new();
    for entry in ratchet["gates"][gate]["requires"].as_array().into_iter().flatten() {
        let r = &ratchet["requirements"][entry_requirement(entry)];
        if ratchet["assessments"][text(&r["assessment"])]["subject"] == subject {
            held.extend(requirement_measurement(ratchet, r));
        }
    }
    ids.iter().filter(|id| !held.contains(measurement_of(id))).map(|id| id.to_string()).collect()
}

/// The text `task check` runs, by where it comes from: each command of the tasks `check` and `test`
/// and of every task they call (a call is written `task <name>`), and every file of the hook
/// directories the gate runs (`tools/check.d`, `tools/test-setup.d`, `tools/test-teardown.d`).
pub fn gate_text(root: &Path, taskfile: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let mut todo: Vec<String> = vec!["check".into(), "test".into()];
    while let Some(task) = todo.pop() {
        if !seen.insert(task.clone()) {
            continue;
        }
        let t = &taskfile["tasks"][&task];
        let deps = t["deps"].as_array().into_iter().flatten().map(|d| (true, d));
        let cmds = t["cmds"].as_array().into_iter().flatten().map(|c| (false, c));
        for (dep, item) in deps.chain(cmds) {
            let line = match item {
                Value::String(s) if dep => {
                    todo.push(s.clone());
                    format!("task {s}")
                }
                Value::String(s) => s.clone(),
                other if other["task"].is_string() => {
                    let called = text(&other["task"]);
                    todo.push(called.clone());
                    format!("task {called}")
                }
                other if other["cmd"].is_string() => text(&other["cmd"]),
                other => other.to_string(),
            };
            out.push((format!("Taskfile.yml task {task}"), line));
        }
    }
    for dir in ["tools/check.d", "tools/test-setup.d", "tools/test-teardown.d"] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else { continue };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_file() && path.extension().is_none_or(|e| e != "md") {
                let body = std::fs::read_to_string(&path).unwrap_or_default();
                out.push((format!("{dir}/{}", path.file_name().unwrap().to_string_lossy()), body));
            }
        }
    }
    out
}

/// Every place of the gate's text (`gate_text`) that names one of `names`.
pub fn gate_naming(root: &Path, taskfile: &Value, names: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    for (place, line) in gate_text(root, taskfile) {
        for name in names {
            if line.contains(name) {
                found.push(format!("{place} names {name}: {line}"));
            }
        }
    }
    found
}

/// The nine measured `repo.*` KPIs of `design:quality-kpis` § 4.
const REPOSITORY_KPIS: &[&str] = &[
    "repo.gate-seconds",
    "repo.test-step-seconds",
    "repo.slowest-test-seconds",
    "repo.gate-budget-cases-failed",
    "repo.test-binaries-per-crate",
    "repo.python-files",
    "repo.toolchain-pins-disagree",
    "repo.cache-bypass",
    "repo.plan-validate-defects",
];

/// The gate KPIs among them (all but `repo.gate-seconds`, a ratchet).
const REPOSITORY_GATE_KPIS: &[&str] = &[
    "repo.test-step-seconds",
    "repo.slowest-test-seconds",
    "repo.gate-budget-cases-failed",
    "repo.test-binaries-per-crate",
    "repo.python-files",
    "repo.toolchain-pins-disagree",
    "repo.cache-bypass",
    "repo.plan-validate-defects",
];

/// What the gate must not run: the KPI task, its script and its hooks (`tools/kpis.d` with or
/// without the slash).
const KPI_COMMANDS: &[&str] = &["tools/kpis.sh", "tools/kpis.d", "task kpis"];

/// `accepted_value` requirements written before their first record, with the story that adopts it.
/// Both read the 1024-walk explore from the full setup, which `story:kpi-specification` measures
/// and adopts; an entry goes when its record lands (`consistency_defects` fails while both exist).
const RECORDS_PENDING: &[(&str, &str)] =
    &[("no-lost-outcomes", "story:kpi-specification"), ("no-new-unsupported-answers", "story:kpi-specification")];

fn committed_defects(ratchet: &Value) -> Vec<String> {
    let root = root();
    consistency_defects(
        ratchet,
        &assessor_outputs(&root.join("quality/assessors")),
        &reference_records(&root.join("ratchet/references")),
        RECORDS_PENDING,
    )
}

#[test]
fn the_ratchet_file_is_consistent() {
    let ratchet = ratchet_file();
    let defects = committed_defects(&ratchet);
    assert!(defects.is_empty(), "{defects:#?}");
    assert!(keys(&ratchet["subjects"]).contains("docs-site"), "the subject docs-site is not declared");

    // A fixture: two requirements, one with a record; each defect class broken once.
    let fixture: Value = ess_yaml::from_str(
        "
subjects: {s: {kind: codebase}}
assessments:
  a:
    subject: s
    assessor: x@1
    measurements: {m: {kind: exact, output: o}}
requirements:
  fixed: {assessment: a, measurement: m, condition: {operator: lte, value: 0}}
  held: {assessment: a, measurement: m, reference: {kind: accepted_value}}
gates:
  merge: {requires: [fixed, {requirement: held}]}
",
    )
    .unwrap();
    let assessors: BTreeMap<String, BTreeSet<String>> = [("x@1".to_owned(), ["o".to_owned()].into())].into();
    let record = serde_json::json!({"format": "ratchet-reference/1", "requirement": "held"});
    let records: BTreeMap<String, Value> = [("held".to_owned(), record)].into();
    assert_eq!(consistency_defects(&fixture, &assessors, &records, &[]), Vec::<String>::new());

    let mut missing_measurement = fixture.clone();
    missing_measurement["requirements"]["fixed"]["measurement"] = "gone".into();
    assert_eq!(
        consistency_defects(&missing_measurement, &assessors, &records, &[]),
        ["requirement fixed: measurement \"gone\" of a does not exist"]
    );

    let mut missing_requirement = fixture.clone();
    missing_requirement["gates"]["merge"]["requires"].as_array_mut().unwrap().push("gone".into());
    assert_eq!(
        consistency_defects(&missing_requirement, &assessors, &records, &[]),
        ["gate merge: requirement \"gone\" does not exist"]
    );

    assert_eq!(
        consistency_defects(&fixture, &assessors, &BTreeMap::new(), &[]),
        ["requirement held: accepted_value with no record ratchet/references/held.json"]
    );

    let mut undeclared_output = fixture.clone();
    undeclared_output["assessments"]["a"]["measurements"]["m"]["output"] = "p".into();
    assert_eq!(
        consistency_defects(&undeclared_output, &assessors, &records, &[]),
        ["assessment a, measurement m: output \"p\" is not declared by x@1"]
    );

    // A pending record excuses its missing record, and nothing else.
    let pending = [("held", "story:x")];
    assert_eq!(consistency_defects(&fixture, &assessors, &BTreeMap::new(), &pending), Vec::<String>::new());
    assert_eq!(
        consistency_defects(&fixture, &assessors, &records, &pending),
        ["pending record held (story:x): the record exists, remove it from the pending list"]
    );
    assert_eq!(
        consistency_defects(&fixture, &assessors, &records, &[("fixed", "story:x")]),
        ["pending record fixed (story:x): no accepted_value requirement of that name"]
    );
}

#[test]
fn repository_kpis_have_measurements() {
    let ratchet = ratchet_file();
    let missing = kpis_without_measurement(&ratchet, "repository", REPOSITORY_KPIS);
    assert!(missing.is_empty(), "no measurement under the subject repository: {missing:?}");

    let mut copy = ratchet.clone();
    for a in copy["assessments"].as_object_mut().unwrap().values_mut() {
        if a["subject"] == "repository" {
            a["measurements"].as_object_mut().unwrap().remove("python-files");
        }
    }
    assert_eq!(kpis_without_measurement(&copy, "repository", REPOSITORY_KPIS), ["repo.python-files"]);
}

#[test]
fn repository_gate_kpis_are_merge_requirements() {
    let ratchet = ratchet_file();
    let missing = kpis_not_in_gate(&ratchet, "merge", "repository", REPOSITORY_GATE_KPIS);
    assert!(missing.is_empty(), "not a requirement of the merge gate: {missing:?}");

    let mut copy = ratchet.clone();
    let requires = copy["gates"]["merge"]["requires"].as_array_mut().unwrap();
    let before = requires.len();
    requires.retain(|e| {
        let r = &ratchet["requirements"][entry_requirement(e)];
        r["measurement"] != "cache-bypass" || ratchet["assessments"][text(&r["assessment"])]["subject"] != "repository"
    });
    assert_eq!(requires.len(), before - 1);
    assert_eq!(kpis_not_in_gate(&copy, "merge", "repository", REPOSITORY_GATE_KPIS), ["repo.cache-bypass"]);
}

#[test]
fn check_runs_no_kpi_hook() {
    let root = root();
    let tasks = taskfile();
    assert!(tasks["tasks"]["kpis"].is_object(), "Taskfile.yml has no task kpis");
    let found = gate_naming(&root, &tasks, KPI_COMMANDS);
    assert!(found.is_empty(), "task check runs a KPI measurement: {found:#?}");

    let mut copy = tasks.clone();
    copy["tasks"]["check"]["cmds"].as_array_mut().unwrap().push(serde_json::json!({"task": "kpis"}));
    let found = gate_naming(&root, &copy, KPI_COMMANDS);
    assert!(found.iter().any(|f| f.starts_with("Taskfile.yml task check names task kpis")), "{found:#?}");
    assert!(found.iter().any(|f| f.starts_with("Taskfile.yml task kpis names tools/kpis.sh")), "{found:#?}");
}
