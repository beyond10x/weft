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
        let Some(file) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        let Some(stem) = file.strip_suffix(".assessor.yaml") else {
            continue;
        };
        let doc = yaml(&path);
        let key = format!("{}@{}", text(&doc["name"]), text(&doc["version"]));
        assert_eq!(
            key,
            stem,
            "{}: name@version differs from the file name",
            path.display()
        );
        let outputs = doc["outputs"]
            .as_object()
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default();
        out.insert(key, outputs);
    }
    out
}

/// The reference records of `dir` (`<requirement>.json`), by the requirement of the file name.
pub fn reference_records(dir: &Path) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "json") {
            let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&path).unwrap();
            let record =
                serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
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
    v.as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default()
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
    let measurements =
        keys(&ratchet["assessments"][text(&requirement["assessment"])]["measurements"]);
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
    let assessments = ratchet["assessments"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let requirements = ratchet["requirements"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    for (name, a) in &assessments {
        let subject = text(&a["subject"]);
        if !subjects.contains(&subject) {
            defects.push(format!(
                "assessment {name}: subject {subject:?} is not declared"
            ));
        }
        let assessor = text(&a["assessor"]);
        let Some(outputs) = assessors.get(&assessor) else {
            defects.push(format!(
                "assessment {name}: assessor {assessor:?} has no declaration quality/assessors/{assessor}.assessor.yaml"
            ));
            continue;
        };
        for (m, measurement) in a["measurements"].as_object().cloned().unwrap_or_default() {
            let output = text(&measurement["output"]);
            if !outputs.contains(&output) {
                defects.push(format!(
                    "assessment {name}, measurement {m}: output {output:?} is not declared by {assessor}"
                ));
            }
        }
    }
    for (name, r) in &requirements {
        let assessment = text(&r["assessment"]);
        let Some(a) = assessments.get(&assessment) else {
            defects.push(format!(
                "requirement {name}: assessment {assessment:?} does not exist"
            ));
            continue;
        };
        match requirement_measurement(ratchet, r) {
            Some(m) if keys(&a["measurements"]).contains(&m) => {}
            Some(m) => defects.push(format!(
                "requirement {name}: measurement {m:?} of {assessment} does not exist"
            )),
            None => defects.push(format!(
                "requirement {name}: names no measurement and {assessment} has not exactly one"
            )),
        }
        let awaited = pending.iter().any(|(p, _)| p == name);
        if r["reference"]["kind"] == "accepted_value" && !records.contains_key(name) && !awaited {
            defects.push(format!(
                "requirement {name}: accepted_value with no record ratchet/references/{name}.json"
            ));
        }
    }
    for (name, story) in pending {
        if requirements
            .get(*name)
            .is_none_or(|r| r["reference"]["kind"] != "accepted_value")
        {
            defects.push(format!(
                "pending record {name} ({story}): no accepted_value requirement of that name"
            ));
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
            defects.push(format!(
                "ratchet/references/{stem}.json: not a ratchet-reference/1 record of {stem}"
            ));
        }
        if !requirements.contains_key(stem) {
            defects.push(format!(
                "ratchet/references/{stem}.json: requirement {stem} does not exist"
            ));
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
    for a in ratchet["assessments"]
        .as_object()
        .into_iter()
        .flat_map(|o| o.values())
    {
        if a["subject"] == subject {
            held.extend(keys(&a["measurements"]));
        }
    }
    ids.iter()
        .filter(|id| !held.contains(measurement_of(id)))
        .map(|id| id.to_string())
        .collect()
}

/// The ids of `kpis` (id, bound) that no requirement listed in `gate` holds: a requirement over an
/// assessment of `subject` that reads the id's measurement with the condition `lte <bound>`, the
/// bound of the catalogue exactly.
pub fn kpis_not_in_gate(
    ratchet: &Value,
    gate: &str,
    subject: &str,
    kpis: &[(&str, f64)],
) -> Vec<String> {
    let mut held = Vec::new();
    for entry in ratchet["gates"][gate]["requires"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let r = &ratchet["requirements"][entry_requirement(entry)];
        if ratchet["assessments"][text(&r["assessment"])]["subject"] == subject
            && r["condition"]["operator"] == "lte"
        {
            if let (Some(m), Some(v)) = (
                requirement_measurement(ratchet, r),
                r["condition"]["value"].as_f64(),
            ) {
                held.push((m, v));
            }
        }
    }
    kpis.iter()
        .filter(|(id, bound)| {
            !held
                .iter()
                .any(|(m, v)| m == measurement_of(id) && v == bound)
        })
        .map(|(id, _)| id.to_string())
        .collect()
}

/// The tasks a shell command calls: every word after a `task` command word up to the end of that
/// command (`;`, `&`, `|`, a newline, `--`) that names a task of `tasks` and is no option.
fn task_calls(command: &str, tasks: &Value) -> Vec<String> {
    let mut calls = Vec::new();
    for segment in command.split([';', '&', '|', '\n', '(', ')', '`']) {
        let words: Vec<&str> = segment.split_whitespace().collect();
        let Some(at) = words
            .iter()
            .position(|w| *w == "task" || w.ends_with("/task"))
        else {
            continue;
        };
        for word in &words[at + 1..] {
            if *word == "--" {
                break;
            }
            let name = word.trim_matches(|c| c == '"' || c == '\'');
            if !name.starts_with('-') && tasks.get(name).is_some() {
                calls.push(name.to_owned());
            }
        }
    }
    calls
}

/// The text `task check` runs, by where it comes from: each command of the tasks `check` and `test`
/// and of every task they call (a call is `task: <name>`, a dependency, or `task <name>` in a shell
/// command or a hook, written `task <name>`), and every file of the hook directories the gate runs
/// (`tools/check.d`, `tools/test-setup.d`, `tools/test-teardown.d`).
pub fn gate_text(root: &Path, taskfile: &Value) -> Vec<(String, String)> {
    let tasks = &taskfile["tasks"];
    let mut out = Vec::new();
    let mut todo: Vec<String> = vec!["check".into(), "test".into()];
    for dir in [
        "tools/check.d",
        "tools/test-setup.d",
        "tools/test-teardown.d",
    ] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_file() && path.extension().is_none_or(|e| e != "md") {
                let body = std::fs::read_to_string(&path).unwrap_or_default();
                todo.extend(task_calls(&body, tasks));
                out.push((
                    format!("{dir}/{}", path.file_name().unwrap().to_string_lossy()),
                    body,
                ));
            }
        }
    }
    let mut seen = BTreeSet::new();
    while let Some(task) = todo.pop() {
        if !seen.insert(task.clone()) {
            continue;
        }
        let t = &tasks[&task];
        let deps = t["deps"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|d| (true, d));
        let cmds = t["cmds"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|c| (false, c));
        for (dep, item) in deps.chain(cmds) {
            let line = match item {
                Value::String(s) if dep => {
                    todo.push(s.clone());
                    format!("task {s}")
                }
                Value::String(s) => {
                    todo.extend(task_calls(s, tasks));
                    s.clone()
                }
                other if other["task"].is_string() => {
                    let called = text(&other["task"]);
                    todo.push(called.clone());
                    format!("task {called}")
                }
                other if other["cmd"].is_string() => {
                    todo.extend(task_calls(other["cmd"].as_str().unwrap(), tasks));
                    text(&other["cmd"])
                }
                other => other.to_string(),
            };
            out.push((format!("Taskfile.yml task {task}"), line));
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

/// The gate KPIs among them (all but `repo.gate-seconds`, a ratchet), each with the bound of the
/// catalogue: the value must be at most that.
const REPOSITORY_GATE_KPIS: &[(&str, f64)] = &[
    ("repo.test-step-seconds", 120.0),
    ("repo.slowest-test-seconds", 60.0),
    ("repo.gate-budget-cases-failed", 0.0),
    ("repo.test-binaries-per-crate", 1.0),
    ("repo.python-files", 0.0),
    ("repo.toolchain-pins-disagree", 0.0),
    ("repo.cache-bypass", 0.0),
    ("repo.plan-validate-defects", 0.0),
];

/// What the gate must not run: the KPI task, its script and its hooks (`tools/kpis.d` with or
/// without the slash).
const KPI_COMMANDS: &[&str] = &["tools/kpis.sh", "tools/kpis.d", "task kpis"];

/// `accepted_value` requirements written before their first record, with the story that adopts it.
/// Both read the 1024-walk explore from the full setup, which `story:kpi-specification` measures
/// and adopts; an entry goes when its record lands (`consistency_defects` fails while both exist).
const RECORDS_PENDING: &[(&str, &str)] = &[
    ("no-lost-outcomes", "story:kpi-specification"),
    ("no-new-unsupported-answers", "story:kpi-specification"),
];

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
    assert!(
        keys(&ratchet["subjects"]).contains("docs-site"),
        "the subject docs-site is not declared"
    );

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
    let assessors: BTreeMap<String, BTreeSet<String>> =
        [("x@1".to_owned(), ["o".to_owned()].into())].into();
    let record = serde_json::json!({"format": "ratchet-reference/1", "requirement": "held"});
    let records: BTreeMap<String, Value> = [("held".to_owned(), record)].into();
    assert_eq!(
        consistency_defects(&fixture, &assessors, &records, &[]),
        Vec::<String>::new()
    );

    let mut missing_measurement = fixture.clone();
    missing_measurement["requirements"]["fixed"]["measurement"] = "gone".into();
    assert_eq!(
        consistency_defects(&missing_measurement, &assessors, &records, &[]),
        ["requirement fixed: measurement \"gone\" of a does not exist"]
    );

    let mut missing_requirement = fixture.clone();
    missing_requirement["gates"]["merge"]["requires"]
        .as_array_mut()
        .unwrap()
        .push("gone".into());
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

    let mut undeclared_assessor = fixture.clone();
    undeclared_assessor["assessments"]["a"]["assessor"] = "y@1".into();
    assert_eq!(
        consistency_defects(&undeclared_assessor, &assessors, &records, &[]),
        ["assessment a: assessor \"y@1\" has no declaration quality/assessors/y@1.assessor.yaml"]
    );

    // A pending record excuses its missing record, and nothing else.
    let pending = [("held", "story:x")];
    assert_eq!(
        consistency_defects(&fixture, &assessors, &BTreeMap::new(), &pending),
        Vec::<String>::new()
    );
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
    assert!(
        missing.is_empty(),
        "no measurement under the subject repository: {missing:?}"
    );

    let mut copy = ratchet.clone();
    for a in copy["assessments"].as_object_mut().unwrap().values_mut() {
        if a["subject"] == "repository" {
            a["measurements"]
                .as_object_mut()
                .unwrap()
                .remove("python-files");
        }
    }
    assert_eq!(
        kpis_without_measurement(&copy, "repository", REPOSITORY_KPIS),
        ["repo.python-files"]
    );
}

#[test]
fn repository_gate_kpis_are_merge_requirements() {
    let ratchet = ratchet_file();
    let missing = kpis_not_in_gate(&ratchet, "merge", "repository", REPOSITORY_GATE_KPIS);
    assert!(
        missing.is_empty(),
        "not a requirement of the merge gate: {missing:?}"
    );

    let mut copy = ratchet.clone();
    let requires = copy["gates"]["merge"]["requires"].as_array_mut().unwrap();
    let before = requires.len();
    requires.retain(|e| {
        let r = &ratchet["requirements"][entry_requirement(e)];
        r["measurement"] != "cache-bypass"
            || ratchet["assessments"][text(&r["assessment"])]["subject"] != "repository"
    });
    assert_eq!(requires.len(), before - 1);
    assert_eq!(
        kpis_not_in_gate(&copy, "merge", "repository", REPOSITORY_GATE_KPIS),
        ["repo.cache-bypass"]
    );

    // A requirement that loosens the catalogue's bound, or compares the other way, holds nothing.
    let mut loose = ratchet.clone();
    let mut other_way = ratchet.clone();
    for (name, r) in ratchet["requirements"].as_object().unwrap() {
        if r["measurement"] == "test-step-seconds" && r["assessment"] == "repository-facts" {
            loose["requirements"][name]["condition"]["value"] = 121.into();
            other_way["requirements"][name]["condition"]["operator"] = "gte".into();
        }
    }
    assert_eq!(
        kpis_not_in_gate(&loose, "merge", "repository", REPOSITORY_GATE_KPIS),
        ["repo.test-step-seconds"]
    );
    assert_eq!(
        kpis_not_in_gate(&other_way, "merge", "repository", REPOSITORY_GATE_KPIS),
        ["repo.test-step-seconds"]
    );
}

/// `RECORDS_PENDING` only shrinks: it names at most the two requirements that were written before
/// their records (at 9229cea); `story:kpi-specification` empties it.
const RECORDS_PENDING_AT_START: &[&str] = &["no-lost-outcomes", "no-new-unsupported-answers"];

/// The entries of `pending` that `allowed` does not name.
pub fn pending_beyond(pending: &[(&str, &str)], allowed: &[&str]) -> Vec<String> {
    pending
        .iter()
        .filter(|(name, _)| !allowed.contains(name))
        .map(|(name, _)| name.to_string())
        .collect()
}

#[test]
fn the_pending_records_list_does_not_grow() {
    let grown = pending_beyond(RECORDS_PENDING, RECORDS_PENDING_AT_START);
    assert!(
        grown.is_empty(),
        "RECORDS_PENDING may only shrink; new entries: {grown:?}"
    );
    let added = [
        ("no-lost-outcomes", "story:kpi-specification"),
        ("repo-gate-time-not-worse", "story:x"),
    ];
    assert_eq!(
        pending_beyond(&added, RECORDS_PENDING_AT_START),
        ["repo-gate-time-not-worse"]
    );
}

/// The hook prints one line per output its declaration names, with the id `repo.<output>`, and
/// those are the nine repository KPIs; the test step is timed as the catalogue defines it, the
/// root `tools/test-step.sh`. Run on a fixture tree with no tools, so every line is `unavailable`
/// and nothing is measured.
#[test]
fn the_repository_hook_prints_each_declared_output() {
    let declared: BTreeSet<String> = assessor_outputs(&root().join("quality/assessors"))
        ["repo-facts@1"]
        .iter()
        .map(|o| format!("repo.{o}"))
        .collect();
    let kpis: BTreeSet<String> = REPOSITORY_KPIS.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        declared, kpis,
        "repo-facts@1 outputs against the catalogue's repo.* ids"
    );

    let dir = hook_fixture("declared-outputs", &[PIN], true);
    let printed: BTreeSet<String> = run_repository_hook(&dir, &[]).into_keys().collect();
    assert_eq!(
        printed, declared,
        "ids the hook prints against repo-facts@1 outputs"
    );

    let tsv = std::fs::read_to_string(dir.join("tree/target/kpis/repository/values.tsv")).unwrap();
    let step = tsv
        .lines()
        .find(|l| l.starts_with("repo.test-step-seconds\t"))
        .unwrap();
    assert!(
        step.contains("tools/test-step.sh") && !step.contains("task test"),
        "{step}"
    );
}

#[test]
fn check_runs_no_kpi_hook() {
    let root = root();
    let tasks = taskfile();
    assert!(
        tasks["tasks"]["kpis"].is_object(),
        "Taskfile.yml has no task kpis"
    );
    let found = gate_naming(&root, &tasks, KPI_COMMANDS);
    assert!(
        found.is_empty(),
        "task check runs a KPI measurement: {found:#?}"
    );

    let mut copy = tasks.clone();
    copy["tasks"]["check"]["cmds"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"task": "kpis"}));
    let found = gate_naming(&root, &copy, KPI_COMMANDS);
    assert!(
        found
            .iter()
            .any(|f| f.starts_with("Taskfile.yml task check names task kpis")),
        "{found:#?}"
    );
    assert!(
        found
            .iter()
            .any(|f| f.starts_with("Taskfile.yml task kpis names tools/kpis.sh")),
        "{found:#?}"
    );
}

// Adversary cases (story:kpi-repository). The hook cases run tools/kpis.d/10-repository.sh on a
// fixture tree of a few files under CARGO_TARGET_TMPDIR, with a PATH that holds only the system
// tools the hook reads with and the stubs a case names: no task, no slot, no real cargo or aep,
// so nothing is built, tested or measured, and no network is reached.

/// A fixture tree `<tmp>/ratchet-hook/<name>/tree` of `files`, a git repository with every file
/// added when `git` is true. Returns the case directory.
fn hook_fixture(name: &str, files: &[(&str, &str)], git: bool) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("ratchet-hook")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let tree = dir.join("tree");
    std::fs::create_dir_all(&tree).unwrap();
    for (path, body) in files {
        let p = tree.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, body).unwrap();
    }
    if git {
        for args in [
            &["-c", "init.defaultBranch=main", "init", "-q"][..],
            &["add", "-A"][..],
        ] {
            let s = std::process::Command::new("git")
                .args(args)
                .current_dir(&tree)
                .env("GIT_CEILING_DIRECTORIES", &dir)
                .status()
                .unwrap();
            assert!(s.success(), "git {args:?} in {}", tree.display());
        }
    }
    dir
}

/// Runs the repository hook in the fixture `dir` with `stubs` (`name`, `sh body`) on PATH beside
/// the system tools it reads with; returns `id -> value` from `target/kpis/repository/values.tsv`.
fn run_repository_hook(dir: &Path, stubs: &[(&str, &str)]) -> BTreeMap<String, String> {
    use std::os::unix::fs::PermissionsExt;
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let path = std::env::var_os("PATH").unwrap_or_default();
    for tool in [
        "sh", "git", "jq", "awk", "sed", "grep", "wc", "tr", "date", "mkdir", "find", "head", "rm",
        "touch", "cat", "dirname", "printf",
    ] {
        if let Some(real) = std::env::split_paths(&path)
            .map(|d| d.join(tool))
            .find(|f| f.is_file())
        {
            std::os::unix::fs::symlink(real, bin.join(tool)).unwrap();
        }
    }
    for (name, body) in stubs {
        let p = bin.join(name);
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let tree = dir.join("tree");
    let out = std::process::Command::new(bin.join("sh"))
        .arg(root().join("tools/kpis.d/10-repository.sh"))
        .current_dir(&tree)
        .env_clear()
        .env("PATH", &bin)
        .env("HOME", dir)
        .env("LC_ALL", "C")
        .env("GIT_CEILING_DIRECTORIES", dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "the hook exited {:?}: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let tsv = std::fs::read_to_string(tree.join("target/kpis/repository/values.tsv")).unwrap();
    tsv.lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            Some((f.next()?.to_owned(), f.next()?.to_owned()))
        })
        .collect()
}

const PIN: (&str, &str) = ("rust-toolchain.toml", "[toolchain]\nchannel = \"1.98.1\"\n");

/// `repo.cache-bypass` guards `~/beyond10x/AGENTS.md` § One compile cache: no `SCCACHE_*` server
/// setting and no build that empties `RUSTC_WRAPPER` or `CARGO_BUILD_RUSTC_WRAPPER`. Each line
/// of the fixture is one such bypass; the literal `RUSTC_WRAPPER=""` is the control.
#[test]
fn adv_repository_hook_counts_every_cache_bypass_spelling() {
    let dir = hook_fixture(
        "cache-bypass",
        &[
            PIN,
            (
                ".github/workflows/gate.yml",
                "jobs:\n  gate:\n    env:\n      RUSTC_WRAPPER: \"\"\n",
            ),
            (".cargo/config.toml", "[build]\nrustc-wrapper = \"\"\n"),
            (
                "tools/build.sh",
                "#!/bin/sh\nSCCACHE_CACHE_SIZE=2G cargo build\nRUSTC_WRAPPER= cargo test\n",
            ),
            (
                "tools/control.sh",
                "#!/bin/sh\nRUSTC_WRAPPER=\"\" cargo build\n",
            ),
        ],
        true,
    );
    let v = run_repository_hook(&dir, &[]);
    assert_eq!(
        v.get("repo.cache-bypass").map(String::as_str),
        Some("5"),
        "{v:#?}"
    );
}

/// `repo.python-files` guards § No Python for permanent tools. A hook of `tools/check.d` has no
/// suffix (`tools/check.d/50-spec-diff`), so a Python hook is a file with a python shebang and
/// no `.py`; `tools/x.py` is the control.
#[test]
fn adv_repository_hook_counts_python_without_the_py_suffix() {
    let dir = hook_fixture(
        "python-shebang",
        &[
            PIN,
            (
                "tools/check.d/60-probe",
                "#!/usr/bin/env python3\nprint(1)\n",
            ),
            ("tools/x.py", "print(1)\n"),
        ],
        true,
    );
    let v = run_repository_hook(&dir, &[]);
    assert_eq!(
        v.get("repo.python-files").map(String::as_str),
        Some("2"),
        "{v:#?}"
    );
}

/// tools/kpis.d/README.md: "A KPI that cannot be measured on this machine prints `unavailable`
/// and the reason, never a guessed value." A tree git cannot read (not a checkout, or one git
/// refuses as of dubious ownership) holds one Python file, one disagreeing pin and one cache
/// bypass; each KPI must print `unavailable` or that 1, never 0.
#[test]
fn adv_repository_hook_says_unavailable_when_git_cannot_read_the_tree() {
    let dir = hook_fixture(
        "no-git",
        &[
            PIN,
            (
                "tools/sim/rust-toolchain.toml",
                "[toolchain]\nchannel = \"1.0.0\"\n",
            ),
            ("tools/x.py", "print(1)\n"),
            (
                "tools/build.sh",
                "#!/bin/sh\nRUSTC_WRAPPER=\"\" cargo build\n",
            ),
        ],
        false,
    );
    let v = run_repository_hook(&dir, &[]);
    let guessed: Vec<_> = [
        "repo.python-files",
        "repo.toolchain-pins-disagree",
        "repo.cache-bypass",
    ]
    .into_iter()
    .filter(|id| !matches!(v.get(*id).map(String::as_str), Some("unavailable" | "1")))
    .map(|id| format!("{id} = {:?}", v.get(id)))
    .collect();
    assert!(
        guessed.is_empty(),
        "a value from a failed git command: {guessed:#?}"
    );
}

/// The hook's branch "cargo metadata failed": cargo metadata of tools/sim fails while the root's
/// succeeds (one crate, one test target). The KPI is the most over both workspaces, so it is
/// `unavailable`, not the root's 1; with both failing, not `null`.
#[test]
fn adv_repository_hook_says_unavailable_when_cargo_metadata_fails() {
    let root_meta = r#"{"packages":[{"name":"a","targets":[{"kind":["test"],"name":"main"}]}]}"#;
    let dir = hook_fixture("cargo-sim-fails", &[PIN], true);
    let stub = format!(
        "case \"$*\" in *--manifest-path*) echo 'error: failed to parse manifest' >&2; exit 101;; esac\necho '{root_meta}'"
    );
    let v = run_repository_hook(&dir, &[("cargo", &stub)]);
    let half = v.get("repo.test-binaries-per-crate").cloned();

    let dir = hook_fixture("cargo-fails", &[PIN], true);
    let v = run_repository_hook(
        &dir,
        &[(
            "cargo",
            "echo 'error: failed to parse manifest' >&2; exit 101",
        )],
    );
    let none = v.get("repo.test-binaries-per-crate").cloned();
    assert_eq!(
        (half.as_deref(), none.as_deref()),
        (Some("unavailable"), Some("unavailable")),
        "(tools/sim fails, both fail)"
    );
}

/// `aep plan artifact validate` that fails and prints a JSON document with no `problems` list is
/// no measurement of 0 defects.
#[test]
fn adv_repository_hook_says_unavailable_when_validate_prints_no_problems() {
    let dir = hook_fixture("aep-fails", &[PIN], true);
    let v = run_repository_hook(&dir, &[("aep", "echo '{\"error\": \"no store\"}'; exit 2")]);
    assert_eq!(
        v.get("repo.plan-validate-defects").map(String::as_str),
        Some("unavailable"),
        "{v:#?}"
    );
}

/// A pending record is excused because a story adopts it, so that story's planning file names the
/// requirement; otherwise nothing ever takes the entry off the list.
#[test]
fn adv_each_pending_record_is_named_by_its_story() {
    let mut untaken = Vec::new();
    for (requirement, story) in RECORDS_PENDING {
        let file = root()
            .join(".engineering/planning/story")
            .join(format!("{}.md", story.trim_start_matches("story:")));
        let body = std::fs::read_to_string(&file).unwrap_or_default();
        if !body.contains(requirement) {
            untaken.push(format!(
                "{story} ({}) does not name {requirement}",
                file.display()
            ));
        }
    }
    assert!(untaken.is_empty(), "{untaken:#?}");
}

/// gate_text: "every task they call (a call is written `task <name>`)". A task called from a
/// shell command of `check` is a task the gate runs.
#[test]
fn adv_gate_text_follows_a_task_called_from_a_shell_command() {
    let root = root();
    let mut copy = taskfile();
    copy["tasks"]["check"]["cmds"]
        .as_array_mut()
        .unwrap()
        .push("task measure".into());
    copy["tasks"]["measure"] = serde_json::json!({"cmds": ["tools/kpis.sh"]});
    let found = gate_naming(&root, &copy, KPI_COMMANDS);
    assert!(
        found
            .iter()
            .any(|f| f.starts_with("Taskfile.yml task measure names tools/kpis.sh")),
        "{found:#?}"
    );
}

/// `aep plan artifact validate` exits 1 when it lists problems (measured on a store with a dangling
/// edge); those problems are the count, not `unavailable`. The other side of
/// `adv_repository_hook_says_unavailable_when_validate_prints_no_problems`.
#[test]
fn the_repository_hook_counts_the_problems_of_a_failing_validate() {
    let dir = hook_fixture("aep-problems", &[PIN], true);
    let v = run_repository_hook(
        &dir,
        &[("aep", "echo '{\"problems\": [\"a\", \"b\"]}'; exit 1")],
    );
    assert_eq!(
        v.get("repo.plan-validate-defects").map(String::as_str),
        Some("2"),
        "{v:#?}"
    );
}
