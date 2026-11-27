//! `result.json` and `index.html`: the numbers, the coverage, the gaps with the line of the
//! specification each names, and one static page with the data inlined.

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Value};

use crate::catalog::Catalog;
use crate::coverage::Coverage;

pub const PAGE: &str = include_str!("page.html");

/// Where each name is declared: `file:line`, from the specification's own text.
pub struct Locator {
    files: Vec<(String, Vec<String>)>,
    index: HashMap<String, (usize, usize)>,
}

impl Locator {
    pub fn new(spec: &Path) -> Self {
        let mut files: Vec<(String, Vec<String>)> = Vec::new();
        if let Ok(text) = std::fs::read_to_string(spec.join("ess-inputs.yaml")) {
            for line in text.lines() {
                let t = line.trim();
                if let Some(rest) = t.strip_prefix("- ") {
                    if rest.ends_with(".yaml") {
                        if let Ok(body) = std::fs::read_to_string(spec.join(rest)) {
                            files.push((
                                format!("{}/{}", spec.display(), rest),
                                body.lines().map(ToOwned::to_owned).collect::<Vec<String>>(),
                            ));
                        }
                    }
                }
            }
        }
        let mut index = HashMap::new();
        for (fi, (_, lines)) in files.iter().enumerate() {
            for (li, line) in lines.iter().enumerate() {
                let t = line.trim_start().trim_start_matches("- ");
                if let Some(rest) = t.strip_prefix("name: ") {
                    let key = rest.split([' ', '#']).next().unwrap_or("").to_owned();
                    index.entry(key).or_insert((fi, li));
                }
            }
        }
        Self { files, index }
    }

    fn find(&self, needle: &str) -> Option<(usize, usize)> {
        self.index.get(needle).copied()
    }

    pub fn name(&self, qualified: &str) -> String {
        self.find(qualified)
            .map(|(f, l)| format!("{}:{}", self.files[f].0, l + 1))
            .unwrap_or_else(|| qualified.to_owned())
    }

    /// The line of `child` (an outcome or a state) inside the block of `parent`.
    pub fn within(&self, parent: &str, child: &str) -> String {
        let Some((f, l)) = self.find(parent) else { return parent.to_owned() };
        let lines = &self.files[f].1;
        let indent = lines[l].len() - lines[l].trim_start().len();
        for (i, line) in lines.iter().enumerate().skip(l + 1) {
            let trimmed = line.trim_start();
            let this = line.len() - trimmed.len();
            if !trimmed.is_empty() && this <= indent && trimmed.starts_with("- name:") {
                break;
            }
            if trimmed.contains(&format!("name: {child}")) || trimmed == format!("- {child}") || trimmed.starts_with(&format!("{child}:")) || trimmed.contains(&format!(" {child},")) || trimmed.contains(&format!("[{child}")) {
                return format!("{}:{}", self.files[f].0, i + 1);
            }
        }
        format!("{}:{}", self.files[f].0, l + 1)
    }
}

/// Requests that named a live row for every identity input, below which a command is not called
/// blocked.
pub const BLOCKED_MIN_REQUESTS: u64 = 8;

/// Per (command, answer label): [requests, requests whose identity inputs all named live rows].
pub type Attempts = HashMap<(usize, String), [u64; 2]>;

/// Why a command was never accepted: its refusals by answer, and a one-line class.
///
/// `blocked-by:<guard file:line>`: every request whose identity inputs named rows the walk held
/// (at least [`BLOCKED_MIN_REQUESTS`]) got the same refusal, whatever the other input values the
/// walks drew, and that refusal's guard reads a stored row (a state, a field predicate, a row set):
/// the guard holds on every reachable row, an inferred specification gap. `rare`: the refusals
/// vary, too few requests named live rows, the command was never sent, or the one refusal is an
/// input-only guard or an existence check (the drawing or an upstream command is the gap).
pub fn classify(cat: &Catalog, ci: usize, attempts: &Attempts, loc: &Locator, errors: &[Value]) -> Vec<(&'static str, Value)> {
    let c = &cat.commands[ci];
    let mut all: std::collections::BTreeMap<String, u64> = Default::default();
    let mut live: std::collections::BTreeMap<String, u64> = Default::default();
    for ((cmd, label), [n, l]) in attempts {
        if *cmd == ci {
            *all.entry(label.clone()).or_default() += n;
            if *l > 0 {
                *live.entry(label.clone()).or_default() += l;
            }
        }
    }
    let sent: u64 = all.values().sum();
    let live_total: u64 = live.values().sum();
    let (class, why, blocked) = if sent == 0 {
        ("rare".to_owned(), "never sent in a walk".to_owned(), Value::Null)
    } else if live_total < BLOCKED_MIN_REQUESTS {
        ("rare".to_owned(), format!("{live_total} of {sent} requests named a live row for every identity input"), Value::Null)
    } else if live.len() == 1 && c.outcomes.iter().any(|o| o.name == *live.keys().next().expect("one") && !o.reads_rows) {
        // An input-only guard (other inputs pass it) or an existence check (the row comes from an
        // upstream command): the gap is the drawing or the upstream command, not this guard.
        let (label, n) = live.iter().next().expect("one");
        let o = c.outcomes.iter().find(|o| o.name == *label).expect("declared");
        ("rare".to_owned(), format!("all {n} requests naming live rows answered `{label}` ({}): a guard no stored row decides", o.kind), Value::Null)
    } else if live.len() == 1 {
        let (label, n) = live.iter().next().expect("one");
        let name = c.name.to_string();
        let declared = c.outcomes.iter().any(|o| o.name == *label);
        // An interpreter answer names the guard it could not decide: "the guard of `<cmd>/<outcome>`".
        let guard = (!declared)
            .then(|| {
                errors.iter().filter(|e| e["command"] == name.as_str() && e["kind"] == label.as_str()).find_map(|e| {
                    let text = e["example"].as_str()?;
                    let rest = text.split(&format!("`{name}/")).nth(1)?;
                    rest.split('`').next().map(ToOwned::to_owned)
                })
            })
            .flatten();
        let at = match (&guard, declared) {
            (_, true) => loc.within(&name, label),
            (Some(g), false) => loc.within(&name, g),
            (None, false) => loc.name(&name),
        };
        let what = guard.as_ref().map_or_else(|| label.clone(), |g| format!("{label} at {g}"));
        (
            format!("blocked-by:{at}"),
            format!("all {n} requests naming live rows answered `{what}`"),
            json!({"outcome": what, "at": at, "requests": n, "declared": declared}),
        )
    } else {
        ("rare".to_owned(), format!("{} different refusals over {live_total} requests naming live rows", live.len()), Value::Null)
    };
    vec![
        ("refusals", json!(all)),
        ("refusals_live", json!(live)),
        ("class", json!(class)),
        ("why", json!(why)),
        ("blocked_by", blocked),
    ]
}

/// The gaps of a run, ordered: what the model or the interpreter cannot run first, then what no
/// walk reached.
pub fn gaps(cat: &Catalog, snap: &Value, loc: &Locator, attempts: &Attempts) -> Value {
    let hits = snap["outcome_hits"].as_array().cloned().unwrap_or_default();
    let hit = |gid: usize| hits.get(gid).and_then(Value::as_u64).unwrap_or(0);
    let state_hits = snap["state_hits"].as_array().cloned().unwrap_or_default();
    let tr_hits = snap["transition_hits"].as_array().cloned().unwrap_or_default();
    let mut top: Vec<Value> = Vec::new();
    for v in snap["violations"].as_array().into_iter().flatten().take(10) {
        top.push(json!({"kind": "violation", "what": v["detail"], "at": loc.name(v["command"].as_str().unwrap_or("")), "walk": v["walk"], "seed": v["seed"]}));
    }
    // Unsupported answers: constructs the interpreter cannot run, by command.
    let mut unsupported: Vec<&Value> = snap["errors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| e["kind"] == "unsupported")
        .collect();
    unsupported.sort_by(|a, b| b["count"].as_u64().cmp(&a["count"].as_u64()));
    for e in unsupported.iter().take(15) {
        top.push(json!({"kind": "unsupported", "what": e["why"], "command": e["command"], "at": loc.name(e["command"].as_str().unwrap_or("")), "count": e["count"], "walk": e["walk"], "seed": e["seed"]}));
    }
    // Bindings that used the whole retry bound explore set: the spec would retry them without end.
    for e in snap["errors"].as_array().into_iter().flatten().filter(|e| e["kind"] == "unbounded-retry").take(10) {
        top.push(json!({"kind": "unbounded-retry", "what": e["why"], "command": e["command"], "at": loc.name(e["command"].as_str().unwrap_or("")), "count": e["count"], "walk": e["walk"], "seed": e["seed"]}));
    }
    // Requests no declared outcome covers: the command has no branch for them.
    let mut undeclared: Vec<&Value> = snap["errors"].as_array().into_iter().flatten().filter(|e| e["kind"] == "undeclared").collect();
    undeclared.sort_by(|a, b| b["count"].as_u64().cmp(&a["count"].as_u64()));
    for e in undeclared.iter().take(8) {
        top.push(json!({"kind": "undeclared-refusal", "what": format!("{}: a request no declared outcome covers", e["command"].as_str().unwrap_or("")), "command": e["command"], "at": loc.name(e["command"].as_str().unwrap_or("")), "count": e["count"], "walk": e["walk"], "seed": e["seed"]}));
    }
    // Commands that never answered an accepting outcome, ranked by how many outcomes wait on them.
    let mut never: Vec<(usize, usize)> = Vec::new();
    let mut unreached_outcomes = Vec::new();
    for (ci, c) in cat.commands.iter().enumerate() {
        let accepted = c.outcomes.iter().any(|o| !o.error && hit(o.gid) > 0);
        let waiting = c.outcomes.iter().filter(|o| hit(o.gid) == 0).count();
        if !accepted && c.outcomes.iter().any(|o| !o.error) {
            never.push((ci, waiting));
        }
        for o in &c.outcomes {
            if hit(o.gid) == 0 {
                unreached_outcomes.push(json!({
                    "command": c.name.to_string(), "outcome": o.name, "kind": o.kind,
                    "at": loc.within(&c.name.to_string(), &o.name),
                }));
            }
        }
    }
    never.sort_by(|a, b| b.1.cmp(&a.1));
    let never_list: Vec<Value> = never
        .iter()
        .map(|(ci, waiting)| {
            let c = &cat.commands[*ci];
            let mut row = json!({"command": c.name.to_string(), "unreached_outcomes": waiting, "at": loc.name(&c.name.to_string())});
            // Only explore counts attempts; a simulation's commands are the experiment's choice.
            if !attempts.is_empty() {
                for (k, v) in classify(cat, *ci, attempts, loc, snap["errors"].as_array().map_or(&[][..], Vec::as_slice)) {
                    row[k] = v;
                }
            }
            row
        })
        .collect();
    let blocked: Vec<&Value> = never_list.iter().filter(|n| n["class"].as_str().is_some_and(|c| c.starts_with("blocked-by:"))).collect();
    for n in blocked.iter().take(10) {
        top.push(json!({"kind": "command-blocked-by", "what": format!("{} is always refused `{}` ({} requests naming live rows)", n["command"].as_str().unwrap_or(""), n["blocked_by"]["outcome"].as_str().unwrap_or(""), n["blocked_by"]["requests"]), "at": n["blocked_by"]["at"]}));
    }
    let blocked_count = blocked.len();
    let mut unreached_states = Vec::new();
    let mut unreached_transitions = Vec::new();
    let mut never_created = Vec::new();
    for e in &cat.entities {
        let name = e.name.to_string();
        let mut any = false;
        for (si, s) in e.states.iter().enumerate() {
            let n = state_hits.get(e.state_base + si).and_then(Value::as_u64).unwrap_or(0);
            if n == 0 {
                unreached_states.push(json!({"entity": name, "state": s, "at": loc.within(&name, s)}));
            } else {
                any = true;
            }
        }
        if !any {
            never_created.push(json!({"entity": name, "at": loc.name(&name)}));
        }
        for (ti, t) in e.transitions.iter().enumerate() {
            let n = tr_hits.get(e.tr_base + ti).and_then(Value::as_u64).unwrap_or(0);
            if n == 0 {
                unreached_transitions.push(json!({"entity": name, "transition": t.name, "at": loc.within(&name, &t.name)}));
            }
        }
    }
    for n in never_created.iter().take(5) {
        top.push(json!({"kind": "entity-never-held", "what": format!("no walk held a row of {}", n["entity"].as_str().unwrap_or("")), "at": n["at"]}));
    }
    for n in never_list.iter().take(10) {
        top.push(json!({"kind": "command-never-accepted", "what": format!("{} never answered an accepting outcome ({} outcomes unreached)", n["command"].as_str().unwrap_or(""), n["unreached_outcomes"]), "at": n["at"]}));
    }
    json!({
        "top": top,
        "commands_never_accepted": never_list,
        "blocked_by_count": blocked_count,
        "entities_never_held": never_created,
        "unreached_outcomes": unreached_outcomes,
        "unreached_states": unreached_states,
        "unreached_transitions": unreached_transitions,
    })
}

pub fn page(model: &Value, snap: &Value, live: bool) -> String {
    // Inlined into a <script>: no `<` at all, so no `</script`, `<!--` or `<script` in a recorded
    // value can end the script or move the HTML parser into an escaped state. `<` only occurs
    // inside JSON strings, where its escape (backslash, u003c) reads back as the same character.
    let safe = |v: &Value| v.to_string().replace('<', "\\u003c");
    PAGE.replace("/*__MODEL__*/null", &safe(model))
        .replace("/*__SNAP__*/null", &safe(snap))
        .replace("/*__LIVE__*/false", if live { "true" } else { "false" })
}

#[allow(clippy::too_many_arguments)]
pub fn write(
    out: &Path,
    cat: &Catalog,
    cov: &Coverage,
    loc: &Locator,
    run: Value,
) -> Result<Value, String> {
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut snap = cov.snapshot(cat, true);
    let attempts = cov.inner.lock().map(|i| i.attempts.clone()).unwrap_or_default();
    let gaps = gaps(cat, &snap, loc, &attempts);
    snap["gaps"] = gaps;
    snap["run"] = run;
    let mut per_entity: HashMap<String, Value> = HashMap::new();
    for e in &cat.entities {
        per_entity.insert(e.name.to_string(), json!({"at": loc.name(&e.name.to_string())}));
    }
    snap["locations"] = json!(per_entity);
    // Per command its accepted (non-error) answers and the outcomes reached, by name.
    let hits = snap["outcome_hits"].as_array().cloned().unwrap_or_default();
    let mut accepted = serde_json::Map::new();
    for c in &cat.commands {
        let mut n = 0;
        let mut reached = serde_json::Map::new();
        for o in &c.outcomes {
            let h = hits.get(o.gid).and_then(Value::as_u64).unwrap_or(0);
            if !o.error {
                n += h;
            }
            if h > 0 {
                reached.insert(o.name.clone(), json!(h));
            }
        }
        accepted.insert(c.name.to_string(), json!({"accepted": n, "outcomes": reached}));
    }
    snap["commands"] = Value::Object(accepted);
    // `outcome_hits[gid]` is the outcome `outcome_names[gid]` (command/outcome).
    let mut names = vec![String::new(); cat.outcomes_total];
    for c in &cat.commands {
        for o in &c.outcomes {
            names[o.gid] = format!("{}/{}", c.name, o.name);
        }
    }
    snap["outcome_names"] = json!(names);
    let text = serde_json::to_string_pretty(&snap).map_err(|e| e.to_string())?;
    std::fs::write(out.join("result.json"), text).map_err(|e| e.to_string())?;
    let model = cat.model_json();
    std::fs::write(out.join("index.html"), page(&model, &snap, false)).map_err(|e| e.to_string())?;
    Ok(snap)
}
