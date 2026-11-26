//! `weft-sim simulate`: an experiment's setup, traffic, faults and controllers on the interpreted
//! model, with the system invariants checked after every step; and the search over its mutation
//! points (`exhaustive` or `fit`).

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use ess_domain::name::QualifiedName;
use ess_primitives::node::Node;
use serde_json::{json, Value};

use crate::catalog::Catalog;
use crate::coverage::{Check, Coverage, TimelineEntry, Violation};
use crate::experiment::{Controller, Experiment, Step, Traffic};
use crate::explore;
use crate::gen::{Candidates, Input};
use crate::rng::Rng;
use crate::walk::{bound_emits, Answer, Request, Session};

fn qn(name: &str) -> Result<QualifiedName, String> {
    QualifiedName::new(name).map_err(|e| format!("`{name}`: {e}"))
}

pub fn node(value: &Value) -> Result<Node, String> {
    serde_json::from_value(value.clone()).map_err(|e| e.to_string())
}

fn to_value(node: &Node) -> Value {
    serde_json::to_value(node).unwrap_or(Value::Null)
}

/// A request from a recorded trace step.
pub fn request_from_trace(cat: &Catalog, step: &Value) -> Result<Request, String> {
    let name = step["command"].as_str().ok_or("trace step without command")?;
    let cmd = *cat.cmd_index.get(&qn(name)?).ok_or_else(|| format!("no command {name}"))?;
    let actor = match step["actor"].as_str() {
        Some(a) => Some(*cat.actor_index.get(&qn(a)?).ok_or_else(|| format!("no actor {a}"))?),
        None => None,
    };
    let caller = match &step["caller"] {
        Value::Object(map) => {
            let mut out = BTreeMap::new();
            for (k, v) in map {
                out.insert(k.clone(), node(v)?);
            }
            Some(out)
        }
        _ => None,
    };
    let mut input = BTreeMap::new();
    if let Value::Object(map) = &step["input"] {
        for (k, v) in map {
            input.insert(k.clone(), node(v)?);
        }
    }
    let force = step["force"]
        .as_str()
        .and_then(|f| cat.commands[cmd].outcomes.iter().position(|o| o.name == f));
    Ok(Request { cmd, actor, caller, input, force })
}

/// One request of a recorded setup (and settle): the clock it was sent at and its answer.
#[derive(Debug, Clone)]
pub struct Recorded {
    pub req: Request,
    pub answer: String,
    /// The clock (seconds) before the send.
    pub at: u64,
}

/// Sends a recorded setup again from the scenario `sess` just began, with the clock held at each
/// step's recorded value; the number of answers that differ.
pub fn replay_setup(sess: &mut Session, setup: &[Recorded]) -> usize {
    sess.step_seconds = 0;
    let mut differ = 0;
    for r in setup {
        sess.clock.0.store(r.at, Ordering::Relaxed);
        if sess.send(&r.req).label != r.answer {
            differ += 1;
        }
    }
    differ
}

/// Sends a recorded walk again; prints each step, and counts the answers that differ and the
/// system invariant violations (each check that starts to fail, and each duplicate that changed a
/// view).
pub fn replay(cat: &Catalog, cov: &Coverage, walk: u64, trace: &Value, setup: &[Recorded]) -> Result<(usize, usize), String> {
    let mut sess = Session::new(cat, cov, bound_emits(cat));
    let seed = trace["seed"].as_u64().unwrap_or(0);
    sess.begin(walk, seed)?;
    // A walk of `explore --from` starts from the experiment's setup, sent with the clock held.
    let mut differ = replay_setup(&mut sess, setup);
    if !setup.is_empty() {
        println!("setup: {} steps replayed, {differ} answers differ", setup.len());
    }
    sess.step_seconds = 1;
    sess.step = 0;
    sess.guided = true;
    // The system invariants, checked after every step as explore does: a check that starts to
    // fail is printed at the step, and once more when it passes again.
    let invariants: BTreeSet<String> = ALL_INVARIANTS.iter().map(|s| (*s).to_owned()).collect();
    let mut failing: BTreeSet<String> = BTreeSet::new();
    let mut broken = 0;
    for (i, step) in trace["steps"].as_array().into_iter().flatten().enumerate() {
        let req = request_from_trace(cat, step)?;
        // `at`: the clock before the send (a walk's clock jumps and controller ticks move it).
        if let Some(at) = step["at"].as_u64() {
            sess.clock.0.store(at, Ordering::Relaxed);
        }
        let by = step["by"].as_str().unwrap_or("");
        let before = (by == "duplicate").then(|| fingerprint(&sess));
        // A duplicate was sent at the instant of the step it repeats (explore's check_step).
        sess.step_seconds = if by == "duplicate" { 0 } else { 1 };
        let answer = sess.send(&req);
        let recorded = step["answer"].as_str().unwrap_or("");
        let same = recorded == answer.label;
        if !same {
            differ += 1;
        }
        println!(
            "{:>4} {} {}{} -> {}{}",
            i + 1,
            if same { " " } else { "!" },
            cat.commands[req.cmd].name,
            if by.is_empty() { String::new() } else { format!(" ({by})") },
            answer.label,
            if same { String::new() } else { format!(" (recorded {recorded})") }
        );
        if let Some(before) = before {
            let check = idempotency_check(&before, &fingerprint(&sess), &answer);
            if !check.ok {
                broken += 1;
                println!("       violation idempotency: {}", check.detail);
            }
        }
        for (check, _) in system_checks(&sess, &invariants) {
            if !check.ok && failing.insert(check.name.clone()) {
                broken += 1;
                println!("       violation {}: {}", check.name, check.detail);
            } else if check.ok && failing.remove(&check.name) {
                println!("       {} holds again", check.name);
            }
        }
    }
    println!("walk {walk} seed {seed}: {differ} answers differ, {broken} system invariant violations");
    Ok((differ, broken))
}

// ---- substitution -----------------------------------------------------------------------------

/// The rows a controller step reads its input from: `row.<field>`, `cand.<field>`, `vary.<name>`.
pub type Scope<'s> = [(&'s str, &'s BTreeMap<String, Node>)];

fn var_text(v: &Value) -> String {
    match v {
        Value::String(t) => t.clone(),
        other => other.to_string(),
    }
}

/// Resolves the innermost `${name}` first, so `${node_${i}}` reads `node_0`.
fn expand(text: &str, vars: &BTreeMap<String, Value>) -> String {
    let mut out = text.to_owned();
    for _ in 0..16 {
        let Some(start) = out.rfind("${") else { break };
        let Some(len) = out[start..].find('}') else { break };
        let name = &out[start + 2..start + len];
        let value = vars.get(name).map(var_text).unwrap_or_default();
        out.replace_range(start..start + len + 1, &value);
    }
    out
}

/// The inside of a string that is one `${...}` from its first character to its last.
fn whole_var(s: &str) -> Option<&str> {
    let inner = s.strip_prefix("${")?.strip_suffix('}')?;
    let mut depth = 1i32;
    let bytes = s.as_bytes();
    let mut i = 2;
    while i < bytes.len() {
        if bytes[i] == b'$' && bytes.get(i + 1) == Some(&b'{') {
            depth += 1;
            i += 2;
            continue;
        }
        if bytes[i] == b'}' {
            depth -= 1;
            if depth == 0 {
                return (i == bytes.len() - 1).then_some(inner);
            }
        }
        i += 1;
    }
    None
}

/// A value with every `${var}` read; a string that is one `${var}` takes the variable's type.
pub fn substitute_vars(value: &Value, vars: &BTreeMap<String, Value>) -> Value {
    substitute(value, vars, &[])
}

fn substitute(value: &Value, vars: &BTreeMap<String, Value>, scope: &Scope) -> Value {
    match value {
        Value::String(s) => {
            for (prefix, row) in scope {
                if let Some(field) = s.strip_prefix(prefix).and_then(|r| r.strip_prefix('.')) {
                    return row.get(field).map(to_value).unwrap_or(Value::Null);
                }
            }
            if let Some(inner) = whole_var(s) {
                // One variable, its name perhaps built from others: `${node_${i}}`.
                return vars.get(&expand(inner, vars)).cloned().unwrap_or(Value::Null);
            }
            Value::String(expand(s, vars))
        }
        Value::Array(items) => Value::Array(items.iter().map(|v| substitute(v, vars, scope)).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), substitute(v, vars, scope)))
                .collect(),
        ),
        other => other.clone(),
    }
}

// ---- one simulation ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Kpis {
    pub steps: usize,
    pub setup_failures: usize,
    pub violations: usize,
    pub double_placements: usize,
    pub quota_breaches: usize,
    pub cross_tenant_reach: usize,
    /// Pairs of a formation's active placements in groups a closed topology link joins.
    pub closed_link_pairs: usize,
    pub duplicate_effects: usize,
    pub unconverged: usize,
    pub steps_to_converge: usize,
    pub outcomes_reached: usize,
    pub abstract_states: usize,
    pub interpreter_gaps: usize,
    pub pattern_matches: usize,
    /// Messages the traffic sent between running instances.
    pub messages: usize,
    /// Topology links those messages crossed (hops between the nodes' topology groups).
    pub link_crossings: usize,
    /// Messages with no running instance at one end, or an end outside the topology.
    pub messages_unrouted: usize,
    /// Leading commands of the experiment's `chain` the model accepted at least once.
    pub chain_reached: usize,
    /// The first command of the `chain` the model never accepted (empty: the whole chain).
    pub chain_stop: String,
    /// Setup steps not accepted as declared, described; any one fails the run.
    pub setup_refusals: Vec<String>,
}

impl Kpis {
    pub fn get(&self, name: &str) -> f64 {
        let v = serde_json::to_value(self).unwrap_or(Value::Null);
        v.get(name.replace('-', "_")).and_then(Value::as_f64).unwrap_or(0.0)
    }
}

struct Sim<'a, 'c> {
    sess: Session<'a>,
    cands: &'c Candidates,
    exp: &'c Experiment,
    vars: BTreeMap<String, Value>,
    rng: Rng,
    record: bool,
    timeline: Vec<TimelineEntry>,
    kpis: Kpis,
    sent: BTreeMap<String, Request>,
    reached: HashSet<usize>,
    states: HashSet<u64>,
    violations: Vec<Violation>,
    stopped: bool,
    invariants: BTreeSet<String>,
    /// Nodes gone silent (a node_loss fault): nothing names them as `node_id` any more.
    lost: BTreeMap<String, String>,
    /// Commands the model accepted at least once, by name.
    accepted: BTreeSet<String>,
    /// Round-robin offset per spreading controller.
    spread: BTreeMap<String, usize>,
    /// Every request sent while `logging` (the setup and its settle, or a walk's controller tick),
    /// with its answer and the clock before the send.
    log: Vec<(Request, Answer, u64)>,
    logging: bool,
    /// Inside an explore walk's controller tick: no invariant checks, patterns or abstract states
    /// per request (explore checks the system invariants itself, after the tick).
    light: bool,
    /// The non-`steady` controllers that sent requests in the last tick, with the count.
    last_work: Vec<(String, usize)>,
}

pub const ALL_INVARIANTS: [&str; 5] = ["closed-link", "idempotency", "no-double-placement", "quota-conservation", "tenant-isolation"];

/// True when `state` is a terminal state of `entity`.
fn terminal(sess: &Session, entity: &str, state: Option<&Node>) -> bool {
    let Some(Node::Text(s)) = state else { return false };
    let Ok(q) = QualifiedName::new(entity) else { return false };
    sess.cat.ent_index.get(&q).is_some_and(|e| {
        let ent = &sess.cat.entities[*e];
        ent.terminal.iter().any(|t| ent.states[*t] == *s)
    })
}

/// The system invariants read from the store's views (`closed-link`, `no-double-placement`,
/// `quota-conservation`, `tenant-isolation`; `idempotency` needs a second send and is
/// [`idempotency_check`]), each with the number of rows or nodes that break it. A view the model
/// does not declare passes its check. `simulate` runs them after every step, `explore` after every
/// N steps of a walk.
pub fn system_checks(sess: &Session, invariants: &BTreeSet<String>) -> Vec<(Check, usize)> {
    let mut out = Vec::new();
    let view = |name: &str| sess.view(name, BTreeMap::new()).ok();
    let id_of = |r: &BTreeMap<String, Node>, f: &str| r.get(f).map(|n| to_value(n).to_string()).unwrap_or_default();
    let wants = |name: &str| invariants.contains(name);
    let assignments = if wants("no-double-placement") || wants("tenant-isolation") { view("weft.nodes.Assignments") } else { None };
    let decisions = if wants("tenant-isolation") || wants("closed-link") { view("weft.scheduling.PlacementDecisions") } else { None };
    if wants("no-double-placement") {
        let check = match &assignments {
            None => (Check { name: "no-double-placement".into(), ok: true, detail: "view weft.nodes.Assignments not readable".into() }, 0),
            Some(rows) => {
                let mut active: BTreeMap<String, Vec<String>> = BTreeMap::new();
                for r in rows {
                    if terminal(sess, "weft.nodes.Assignment", r.get("state")) {
                        continue;
                    }
                    active.entry(id_of(r, "instance_id")).or_default().push(id_of(r, "node_id"));
                }
                let doubles: Vec<String> = active
                    .iter()
                    .filter(|(_, nodes)| nodes.len() > 1)
                    .map(|(i, nodes)| format!("instance {i} active on {}", nodes.join(", ")))
                    .collect();
                (Check { name: "no-double-placement".into(), ok: doubles.is_empty(), detail: doubles.join("; ") }, doubles.len())
            }
        };
        out.push(check);
    }
    if wants("quota-conservation") {
        let check = match view("weft.accounts.QuotaIndex") {
            None => (Check { name: "quota-conservation".into(), ok: true, detail: "view weft.accounts.QuotaIndex not readable".into() }, 0),
            Some(rows) => {
                let num = |r: &BTreeMap<String, Node>, f: &str| -> Option<i64> {
                    match r.get(f) {
                        Some(Node::Number(n)) => n.to_string().parse::<f64>().ok().map(|v| v as i64),
                        _ => None,
                    }
                };
                let mut bad = Vec::new();
                let mut children: BTreeMap<String, i64> = BTreeMap::new();
                for r in &rows {
                    let id = id_of(r, "quota_id");
                    if let (Some(limit), Some(remaining)) = (num(r, "limit"), num(r, "remaining")) {
                        if remaining < 0 || remaining > limit {
                            bad.push(format!("quota {id}: remaining {remaining} outside 0..={limit}"));
                        }
                    }
                    if let (Some(p), Some(limit)) = (r.get("parent_quota_id"), num(r, "limit")) {
                        if *p != Node::Null && !terminal(sess, "weft.accounts.Quota", r.get("state")) {
                            *children.entry(to_value(p).to_string()).or_default() += limit;
                        }
                    }
                }
                for r in &rows {
                    let id = id_of(r, "quota_id");
                    if let (Some(sum), Some(limit)) = (children.get(&id), num(r, "limit")) {
                        if *sum > limit {
                            bad.push(format!("quota {id}: sub-accounts hold {sum} of a limit {limit}"));
                        }
                    }
                }
                (Check { name: "quota-conservation".into(), ok: bad.is_empty(), detail: bad.join("; ") }, bad.len())
            }
        };
        out.push(check);
    }
    if wants("tenant-isolation") {
        // The tenant of an assignment is its placement decision's (weft.scheduling holds it).
        let check = match (&assignments, &decisions) {
            (Some(assignments), Some(decisions)) => {
                let decision: BTreeMap<String, (String, String)> = decisions
                    .iter()
                    .filter_map(|r| {
                        let id = to_value(r.get("decision_id")?).to_string();
                        let tenant = to_value(r.get("tenant_id")?).to_string();
                        let isolation = match r.get("isolation") { Some(Node::Text(t)) => t.clone(), _ => String::new() };
                        Some((id, (tenant, isolation)))
                    })
                    .collect();
                let mut per_node: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
                for r in assignments {
                    if terminal(sess, "weft.nodes.Assignment", r.get("state")) {
                        continue;
                    }
                    let d = id_of(r, "placement_decision_id");
                    let (tenant, isolation) = decision.get(&d).cloned().unwrap_or_else(|| ("?".into(), String::new()));
                    per_node.entry(id_of(r, "node_id")).or_default().push((tenant, isolation));
                }
                // Two tenants share a kernel when each has an instance below MicroVm on the node;
                // a MicroVm has its own (scheduling.yaml `shared-kernel`, hardening pass 6: a
                // MicroVm beside another tenant's container is no breach).
                let mut bad = Vec::new();
                for (node_id, list) in &per_node {
                    let sharing: BTreeSet<&String> = list.iter().filter(|(t, iso)| t != "?" && iso != "MicroVm").map(|(t, _)| t).collect();
                    if sharing.len() > 1 {
                        bad.push(format!("node {node_id}: {} tenants share its kernel (instances below MicroVm)", sharing.len()));
                    }
                }
                (Check { name: "tenant-isolation".into(), ok: bad.is_empty(), detail: bad.join("; ") }, bad.len())
            }
            _ => (Check { name: "tenant-isolation".into(), ok: true, detail: "views not readable".into() }, 0),
        };
        out.push(check);
    }
    if wants("closed-link") {
        out.push(closed_link_check(sess, decisions.as_deref()));
    }
    out
}

/// `closed-link` (design:tenant-clusters, Step 2b scheduling): no two active placements of one
/// formation sit in topology groups that a Declared link with `crossable == false` joins, either
/// direction. Reads weft.scheduling.PlacementDecisions (a decision that is not Released, with a
/// group) and, for an account with a formation placed in two groups or more,
/// weft.cluster.TopologyLinks.
fn closed_link_check(sess: &Session, decisions: Option<&[BTreeMap<String, Node>]>) -> (Check, usize) {
    let text = |r: &BTreeMap<String, Node>, f: &str| match r.get(f) {
        None | Some(Node::Null) => String::new(),
        Some(n) => key_text(&to_value(n)),
    };
    let Some(decisions) = decisions else {
        return (Check { name: "closed-link".into(), ok: true, detail: "view weft.scheduling.PlacementDecisions not readable".into() }, 0);
    };
    // formation -> group -> the decisions placed there; formation -> its account's raw id.
    let mut groups_of: BTreeMap<String, BTreeMap<String, Vec<String>>> = BTreeMap::new();
    let mut account_of: BTreeMap<String, Node> = BTreeMap::new();
    for r in decisions {
        if terminal(sess, "weft.scheduling.PlacementDecision", r.get("state")) {
            continue;
        }
        let (formation, group) = (text(r, "formation_id"), text(r, "topology_group_id"));
        if formation.is_empty() || group.is_empty() {
            continue;
        }
        if let Some(a) = r.get("account_id") {
            account_of.insert(formation.clone(), a.clone());
        }
        groups_of.entry(formation).or_default().entry(group).or_default().push(text(r, "decision_id"));
    }
    let mut accounts: BTreeMap<String, Node> = BTreeMap::new();
    for (formation, groups) in &groups_of {
        if let (true, Some(a)) = (groups.len() > 1, account_of.get(formation)) {
            accounts.insert(key_text(&to_value(a)), a.clone());
        }
    }
    let mut closed: BTreeSet<(String, String)> = BTreeSet::new();
    for account in accounts.values() {
        let Ok(links) = sess.view("weft.cluster.TopologyLinks", [("account_id".to_owned(), account.clone())].into()) else { continue };
        for l in links {
            if text(&l, "state") == "Declared" && text(&l, "crossable") == "false" {
                let (a, b) = (text(&l, "from_group_id"), text(&l, "to_group_id"));
                closed.insert((b.clone(), a.clone()));
                closed.insert((a, b));
            }
        }
    }
    let mut bad = Vec::new();
    for (formation, groups) in &groups_of {
        for (a, da) in groups {
            for (b, db) in groups.range::<String, _>((std::ops::Bound::Excluded(a), std::ops::Bound::Unbounded)) {
                if closed.contains(&(a.clone(), b.clone())) {
                    bad.push(format!("formation {formation}: placements {} in group {a} and {} in group {b} across a closed link", da.join(","), db.join(",")));
                }
            }
        }
    }
    (Check { name: "closed-link".into(), ok: bad.is_empty(), detail: bad.join("; ") }, bad.len())
}

/// The rows of every parameterless read-your-writes view.
pub type Fingerprint = BTreeMap<String, Vec<BTreeMap<String, Node>>>;

/// What a duplicated step must leave unchanged: the rows of every parameterless read-your-writes
/// view.
pub fn fingerprint(sess: &Session) -> Fingerprint {
    let mut out = BTreeMap::new();
    for (name, view) in sess.cat.ir.views() {
        if view.consistency != ess_domain::view::Consistency::ReadYourWrites || !view.params.is_empty() {
            continue;
        }
        if let Ok(rows) = sess.view(&name.to_string(), BTreeMap::new()) {
            out.insert(name.to_string(), rows);
        }
    }
    out
}

/// The `idempotency` check of a step sent a second time: a declared answer and no view changed
/// (an `unsupported` or `unavailable` answer cannot be checked and passes).
pub fn idempotency_check(before: &Fingerprint, after: &Fingerprint, answer: &Answer) -> Check {
    let changed: Vec<String> = after
        .iter()
        .filter(|(k, v)| before.get(*k) != Some(*v))
        .map(|(k, v)| format!("{k}: {} -> {} rows", before.get(k).map_or(0, Vec::len), v.len()))
        .collect();
    let unchecked = matches!(answer.error.as_ref().map(|e| e.0.as_str()), Some("unsupported" | "unavailable"));
    let ok = unchecked || (changed.is_empty() && answer.outcome.is_some());
    Check {
        name: "idempotency".into(),
        ok,
        detail: if unchecked {
            format!("not checkable: the interpreter answered {}", answer.label)
        } else if ok {
            format!("answered {}", answer.label)
        } else {
            format!("answered {}; changed {}", answer.label, if changed.is_empty() { "nothing".to_owned() } else { changed.join(", ") })
        },
    }
}

/// A declared outcome that is not an error answered.
pub fn accepted(cat: &Catalog, answer: &Answer) -> bool {
    answer.error.is_none()
        && answer.outcome.is_some_and(|g| {
            let (c, o) = cat.outcome_of[g];
            !cat.commands[c].outcomes[o].error
        })
}

fn key_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

impl<'a, 'c> Sim<'a, 'c> {
    fn request(&mut self, step: &Step, scope: &Scope) -> Result<Request, String> {
        let cat = self.sess.cat;
        let cmd = *cat
            .cmd_index
            .get(&qn(&step.command)?)
            .ok_or_else(|| format!("no command `{}`", step.command))?;
        let info = &cat.commands[cmd];
        let actor = match &step.actor {
            Some(a) => Some(*cat.actor_index.get(&qn(a)?).ok_or_else(|| format!("no actor `{a}`"))?),
            None => info.actors.first().copied(),
        };
        // `${now}`: the simulated clock as the step is built, before its send moves it.
        let seconds = self.sess.clock.0.load(Ordering::Relaxed);
        let now = ess_primitives::time::Timestamp::from_epoch_millis(seconds * 1000).iso_8601();
        self.vars.insert("now".into(), json!(now));
        let mut given = Input::new();
        for (k, v) in &step.input {
            let value = substitute(v, &self.vars, scope);
            if !value.is_null() {
                given.insert(k.clone(), node(&value).map_err(|e| format!("{}.{k}: {e}", step.command))?);
            }
        }
        let mut input = explore::input_for(&self.sess, self.cands, cmd, None, &mut self.rng);
        input.retain(|k, _| info.input.iter().any(|f| f.name == *k));
        input.extend(given);
        // A literal `null` in the step leaves the field out (an absent Optional).
        for (k, v) in &step.input {
            if v.is_null() {
                input.remove(k);
            }
        }
        let caller = actor.and_then(|a| {
            let attributes = &cat.actors[a].attributes;
            if attributes.is_empty() {
                return None;
            }
            let mut values = explore::caller_for(&self.sess, a, Some(&input), &mut self.rng).unwrap_or_default();
            for attr in attributes {
                if let Some(v) = step.caller.get(&attr.name).map(|v| substitute(v, &self.vars, scope)).filter(|v| !v.is_null()) {
                    if let Ok(n) = node(&v) {
                        values.insert(attr.name.clone(), n);
                    }
                } else if let Some(v) = self.vars.get(&attr.name).filter(|_| !input.contains_key(&attr.name)) {
                    if let Ok(n) = node(v) {
                        values.insert(attr.name.clone(), n);
                    }
                }
            }
            Some(values)
        });
        Ok(Request { cmd, actor, caller, input, force: None })
    }

    fn bind(&mut self, step: &Step, req: &Request, answer: &Answer) {
        for (var, field) in &step.bind {
            // `Event.field` reads one event (direct or of a command a binding ran); a bare field
            // reads the response, then the direct events, then the bound events, then the input.
            let found = match field.split_once('.') {
                Some((event, f)) => answer
                    .events
                    .iter()
                    .chain(&answer.bound_events)
                    .find(|e| e.event.name().to_string().rsplit('.').next() == Some(event))
                    .and_then(|e| e.payload.get(f)),
                None => answer
                    .response
                    .as_ref()
                    .and_then(|r| r.get(field))
                    .or_else(|| answer.events.iter().find_map(|e| e.payload.get(field)))
                    .or_else(|| answer.bound_events.iter().find_map(|e| e.payload.get(field)))
                    .or_else(|| req.input.get(field)),
            }
            .map(to_value);
            if let Some(v) = found {
                let var = match substitute(&Value::String(var.clone()), &self.vars, &[]) {
                    Value::String(s) => s,
                    _ => var.clone(),
                };
                self.vars.insert(var, v);
            }
        }
    }

    fn view(&self, name: &str, params: BTreeMap<String, Node>) -> Option<Vec<BTreeMap<String, Node>>> {
        self.sess.view(name, params).ok()
    }

    fn view_with(&self, name: &str, key: &str, value: &Value) -> Vec<BTreeMap<String, Node>> {
        let Ok(n) = node(value) else { return Vec::new() };
        self.view(name, [(key.to_owned(), n)].into()).unwrap_or_default()
    }

    fn checks(&mut self) -> Vec<Check> {
        let mut out = Vec::new();
        for (check, bad) in system_checks(&self.sess, &self.invariants) {
            let kpi = match check.name.as_str() {
                "no-double-placement" => &mut self.kpis.double_placements,
                "quota-conservation" => &mut self.kpis.quota_breaches,
                "closed-link" => &mut self.kpis.closed_link_pairs,
                _ => &mut self.kpis.cross_tenant_reach,
            };
            *kpi = (*kpi).max(bad);
            out.push(check);
        }
        for c in &out {
            if !c.ok {
                self.violations.push(Violation {
                    kind: c.name.clone(),
                    detail: c.detail.chars().take(600).collect(),
                    command: String::new(),
                    walk: self.sess.walk,
                    seed: self.sess.seed,
                    step: self.sess.step,
                });
            }
        }
        out
    }

    fn fingerprint(&self) -> Fingerprint {
        fingerprint(&self.sess)
    }

    /// Checks the patterns; `views` also checks those that read views (at the end of a tick).
    fn patterns(&mut self, views: bool) {
        for p in &self.exp.find.patterns {
            if p.reads_views() && !views {
                continue;
            }
            if let Some(rows) = p.matches_with(&self.sess, &self.vars) {
                self.kpis.pattern_matches += 1;
                let mut inner = self.sess.cov.inner.lock().expect("coverage");
                inner.matches.push(json!({"pattern": p.name, "walk": self.sess.walk, "seed": self.sess.seed, "step": self.sess.step, "rows": rows}));
                drop(inner);
                self.stopped = true;
            }
        }
    }

    fn after(&mut self, phase: &str, label: &str, req: &Request, answer: &Answer, fault: Option<String>, extra: Vec<Check>) {
        self.kpis.steps += 1;
        if let Some(g) = answer.outcome {
            self.reached.insert(g);
        }
        for g in &answer.bound {
            self.reached.insert(*g);
        }
        if accepted(self.sess.cat, answer) {
            self.accepted.insert(self.sess.cat.commands[req.cmd].name.to_string());
        }
        for g in &answer.bound {
            let (c, o) = self.sess.cat.outcome_of[*g];
            if !self.sess.cat.commands[c].outcomes[o].error {
                self.accepted.insert(self.sess.cat.commands[c].name.to_string());
            }
        }
        if matches!(answer.error.as_ref().map(|e| e.0.as_str()), Some("unsupported" | "unavailable")) {
            self.kpis.interpreter_gaps += 1;
        }
        if self.light {
            return;
        }
        let (state, _) = self.sess.abstract_state(false);
        self.states.insert(state);
        let mut checks = extra;
        checks.extend(self.checks());
        self.patterns(false);
        if self.record {
            let entry = TimelineEntry {
                index: self.timeline.len() + 1,
                phase: phase.to_owned(),
                label: label.to_owned(),
                command: self.sess.cat.commands[req.cmd].name.to_string(),
                outcome: answer.label.clone(),
                fault,
                checks,
            };
            self.timeline.push(entry.clone());
            let mut inner = self.sess.cov.inner.lock().expect("coverage");
            inner.timeline.push(entry);
            drop(inner);
            self.sess.cov.feed(explore::feed_line(&self.sess, req, answer));
        }
    }

    /// Sends one step; `None` when a silent node would send it (its actor, or the actor whose
    /// frame it `relays`, names the node as `node_id`): a lost node sends nothing.
    fn send_step(&mut self, phase: &str, step: &Step, scope: &Scope, fault: Option<String>) -> Result<Option<Answer>, String> {
        let req = self.request(step, scope)?;
        if phase != "setup" && phase != "fault" {
            if let Some(n) = req.input.get("node_id") {
                let actor = match &step.relays {
                    Some(r) => r.clone(),
                    None => req.actor.map(|a| self.sess.cat.actors[a].name.to_string()).unwrap_or_default(),
                };
                if self.lost.get(&key_text(&to_value(n))).is_some_and(|silent| *silent == actor) {
                    return Ok(None);
                }
            }
        }
        let started = std::time::Instant::now();
        let at = self.sess.clock.0.load(Ordering::Relaxed);
        let answer = self.sess.send(&req);
        let took = started.elapsed().as_secs_f64();
        if took > 1.0 {
            // One command that takes this long is a cost to report: a binding retrying without bound,
            // or a guard whose walk grows with the rows.
            let line = format!("slow step: {} answered {} in {took:.1} s (walk {}, step {})", self.sess.cat.commands[req.cmd].name, answer.label, self.sess.walk, self.sess.step);
            eprintln!("weft-sim: {line}");
            self.sess.cov.inner.lock().expect("coverage").notes.push(line);
        }
        self.bind(step, &req, &answer);
        let label = step.label.clone().unwrap_or_else(|| step.command.rsplit('.').next().unwrap_or("").to_owned());
        let mut extra = Vec::new();
        if let Some(expect) = &step.expect {
            if *expect != answer.label {
                extra.push(Check { name: "expect".into(), ok: false, detail: format!("expected {expect}, answered {}", answer.label) });
            }
        }
        // A setup step is accepted as declared: it answers its `expect`, or with no `expect` an
        // accepting outcome. Anything else (a refusal, `unsupported`, `undeclared`) fails the run.
        let as_declared = match &step.expect {
            Some(expect) => *expect == answer.label,
            None => accepted(self.sess.cat, &answer),
        };
        if phase == "setup" && !as_declared {
            self.kpis.setup_failures += 1;
            let why = answer.error.as_ref().map(|(_, w)| format!(": {}", w.chars().take(200).collect::<String>())).unwrap_or_default();
            self.kpis.setup_refusals.push(format!(
                "{label} ({}) answered {}{}{why}",
                step.command,
                answer.label,
                step.expect.as_ref().map(|e| format!(", declared {e}")).unwrap_or_default()
            ));
        }
        if self.logging {
            self.log.push((req.clone(), answer.clone(), at));
        }
        self.sent.insert(label.clone(), req.clone());
        self.after(phase, &label, &req, &answer, fault, extra);
        Ok(Some(answer))
    }

    /// Sends a step `repeat` times (`${i}` is the repetition).
    fn send_repeated(&mut self, phase: &str, step: &Step) -> Result<(), String> {
        let times = match &step.repeat {
            None => 1,
            Some(v) => substitute(v, &self.vars, &[])
                .as_u64()
                .ok_or_else(|| format!("step {}: repeat is not a number", step.label.clone().unwrap_or_default()))? as usize,
        };
        for i in 0..times {
            self.vars.insert("i".into(), json!(i));
            self.send_step(phase, step, &[], None)?;
            if self.stopped {
                break;
            }
        }
        Ok(())
    }

    fn params(&self, params: &BTreeMap<String, Value>) -> BTreeMap<String, Node> {
        params
            .iter()
            .filter_map(|(k, v)| node(&substitute(v, &self.vars, &[])).ok().map(|n| (k.clone(), n)))
            .collect()
    }

    /// Runs a controller: one pass over its view, repeated up to `rounds` times while a pass sent
    /// something and the view still has rows (a level loop that fills a gap of several).
    fn controller(&mut self, c: &Controller) -> Result<usize, String> {
        let mut total = 0;
        for _ in 0..c.rounds.max(1) {
            let n = self.controller_pass(c)?;
            total += n;
            if n == 0 || self.stopped {
                break;
            }
        }
        Ok(total)
    }

    fn controller_pass(&mut self, c: &Controller) -> Result<usize, String> {
        let rows = self.sess.view(&c.view, self.params(&c.params)).map_err(|e| format!("controller {}: {e}", c.name))?;
        let step = Step {
            command: c.command.clone(),
            actor: Some(c.actor.clone()),
            relays: c.relays.clone(),
            caller: c.caller.clone(),
            input: c.input.clone(),
            expect: None,
            bind: BTreeMap::new(),
            label: Some(format!("{} (controller)", c.name)),
            repeat: None,
        };
        let rows: Vec<BTreeMap<String, Node>> = rows
            .into_iter()
            .filter(|r| c.filter.iter().all(|(k, v)| r.get(k).is_some_and(|have| to_value(have) == substitute(v, &self.vars, &[]))))
            .collect();
        let mut n = 0;
        for row in rows.iter().take(c.limit) {
            let Some(each) = &c.each else {
                if self.send_step("controller", &step, &[("row", row)], None)?.is_some() {
                    n += 1;
                }
                if self.stopped {
                    break;
                }
                continue;
            };
            // A scheduler's choice: every candidate row with every varied value, in order (from
            // a rotating start with `spread`), until the model accepts one.
            let cands = self.sess.view(&each.view, self.params(&each.params)).map_err(|e| format!("controller {}: {e}", c.name))?;
            let mut vary: Vec<BTreeMap<String, Node>> = vec![BTreeMap::new()];
            for (k, values) in &each.vary {
                let mut next = Vec::new();
                for v in values {
                    for base in &vary {
                        let mut m = base.clone();
                        m.insert(k.clone(), node(&substitute(v, &self.vars, &[]))?);
                        next.push(m);
                    }
                }
                vary = next;
            }
            let start = if each.spread { *self.spread.get(&c.name).unwrap_or(&0) } else { 0 };
            let len = cands.len();
            'row: for v in &vary {
                for k in 0..len {
                    let cand = &cands[(start + k) % len];
                    let sent = self.send_step("controller", &step, &[("row", row), ("cand", cand), ("vary", v)], None)?;
                    if let Some(answer) = sent {
                        n += 1;
                        if accepted(self.sess.cat, &answer) {
                            self.spread.insert(c.name.clone(), start + k + 1);
                            break 'row;
                        }
                    }
                    if self.stopped {
                        break 'row;
                    }
                }
            }
            if self.stopped {
                break;
            }
        }
        Ok(n)
    }

    /// The topology group of each node of an account's clusters, and the hop distance between
    /// groups (BFS over the links the topology kind derives, or the declared links of `Custom`).
    fn topology(&self, account: &Value) -> (BTreeMap<String, String>, BTreeMap<(String, String), usize>) {
        let text = |r: &BTreeMap<String, Node>, f: &str| r.get(f).map(|n| key_text(&to_value(n))).unwrap_or_default();
        let mut pool_of_cnp: BTreeMap<String, String> = BTreeMap::new();
        for r in self.view_with("weft.cluster.ClusterNodePools", "account_id", account) {
            if let Some(p) = r.get("node_pool_id").filter(|n| **n != Node::Null) {
                pool_of_cnp.insert(text(&r, "cluster_node_pool_id"), key_text(&to_value(p)));
            }
        }
        let topologies: BTreeMap<String, String> = self
            .view_with("weft.cluster.Topologies", "account_id", account)
            .iter()
            .filter(|r| text(r, "state") != "Removed")
            .map(|r| (text(r, "topology_id"), text(r, "kind")))
            .collect();
        let groups: Vec<BTreeMap<String, Node>> = self
            .view_with("weft.cluster.TopologyGroups", "account_id", account)
            .into_iter()
            .filter(|r| text(r, "state") != "Removed" && topologies.contains_key(&text(r, "topology_id")))
            .collect();
        let mut group_of_pool: BTreeMap<String, String> = BTreeMap::new();
        for g in &groups {
            if let Some(Node::Seq(pools)) = g.get("node_pools") {
                for cnp in pools {
                    if let Some(pool) = pool_of_cnp.get(&key_text(&to_value(cnp))) {
                        group_of_pool.insert(pool.clone(), text(g, "topology_group_id"));
                    }
                }
            }
        }
        let mut group_of_node = BTreeMap::new();
        for n in self.view("weft.infrastructure.Nodes", BTreeMap::new()).unwrap_or_default() {
            if let Some(g) = group_of_pool.get(&text(&n, "node_pool_id")) {
                group_of_node.insert(text(&n, "node_id"), g.clone());
            }
        }
        // Edges per topology.
        let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut link = |a: &str, b: &str| {
            edges.entry(a.to_owned()).or_default().insert(b.to_owned());
            edges.entry(b.to_owned()).or_default().insert(a.to_owned());
        };
        for (tid, kind) in &topologies {
            let mine: Vec<&BTreeMap<String, Node>> = groups.iter().filter(|g| text(g, "topology_id") == *tid).collect();
            let id = |g: &BTreeMap<String, Node>| text(g, "topology_group_id");
            let mut ordered = mine.clone();
            ordered.sort_by_key(|g| text(g, "position").parse::<i64>().unwrap_or(i64::MAX));
            match kind.as_str() {
                "Star" => {
                    for hub in mine.iter().filter(|g| text(g, "role") == "Hub") {
                        for spoke in mine.iter().filter(|g| text(g, "role") != "Hub") {
                            link(&id(hub), &id(spoke));
                        }
                    }
                }
                "Ring" | "Line" => {
                    for w in ordered.windows(2) {
                        link(&id(w[0]), &id(w[1]));
                    }
                    if kind == "Ring" && ordered.len() > 2 {
                        link(&id(ordered[0]), &id(ordered[ordered.len() - 1]));
                    }
                }
                "Tree" => {
                    for g in &mine {
                        let parent = text(g, "parent_group_id");
                        if !parent.is_empty() && parent != "null" {
                            link(&id(g), &parent);
                        }
                    }
                }
                "Custom" => {
                    for l in self.view_with("weft.cluster.TopologyLinks", "account_id", account) {
                        if text(&l, "topology_id") == *tid && text(&l, "crossable") == "true" && text(&l, "state") != "Removed" {
                            link(&text(&l, "from_group_id"), &text(&l, "to_group_id"));
                        }
                    }
                }
                _ => {
                    for a in &mine {
                        for b in &mine {
                            if id(a) != id(b) {
                                link(&id(a), &id(b));
                            }
                        }
                    }
                }
            }
        }
        let mut hops = BTreeMap::new();
        for start in edges.keys() {
            let mut dist: BTreeMap<String, usize> = [(start.clone(), 0)].into();
            let mut queue = std::collections::VecDeque::from([start.clone()]);
            while let Some(g) = queue.pop_front() {
                let d = dist[&g];
                for next in edges.get(&g).into_iter().flatten() {
                    if !dist.contains_key(next) {
                        dist.insert(next.clone(), d + 1);
                        queue.push_back(next.clone());
                    }
                }
            }
            for (g, d) in dist {
                hops.insert((start.clone(), g), d);
            }
        }
        (group_of_node, hops)
    }

    /// One tick of a formation's traffic: `rate` messages on each formation link, between the
    /// running instances of its two components as the pattern pairs them; each message counts
    /// the topology links between the two nodes' groups.
    fn traffic_tick(&mut self, t: &Traffic) {
        let (Some(formation), Some(pattern)) = (t.formation.as_ref().map(|f| substitute(f, &self.vars, &[])), t.pattern.clone()) else {
            return;
        };
        let rate = t.rate.as_ref().and_then(|r| substitute(r, &self.vars, &[]).as_u64()).unwrap_or(1) as usize;
        let account = t.account.as_ref().map(|a| substitute(a, &self.vars, &[])).unwrap_or(Value::Null);
        let text = |r: &BTreeMap<String, Node>, f: &str| r.get(f).map(|n| key_text(&to_value(n))).unwrap_or_default();
        let component: BTreeMap<String, String> = self
            .view_with("weft.formations.FormationComponents", "formation_id", &formation)
            .iter()
            .map(|r| (text(r, "component_id"), text(r, "name")))
            .collect();
        let links: Vec<(String, String)> = self
            .view_with("weft.formations.FormationLinks", "formation_id", &formation)
            .iter()
            .map(|r| (component.get(&text(r, "caller_component_id")).cloned().unwrap_or_default(), component.get(&text(r, "target_component_id")).cloned().unwrap_or_default()))
            .collect();
        // Running instances per component, in replica order, by node.
        let mut nodes_of: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for w in self.view_with("weft.workloads.WorkloadList", "formation_id", &formation) {
            let name = text(&w, "component");
            let Some(id) = w.get("workload_id").map(to_value) else { continue };
            let mut running: Vec<(i64, String)> = self
                .view_with("weft.workloads.InstanceList", "workload_id", &id)
                .iter()
                .filter(|i| text(i, "state") == "Running")
                .map(|i| (text(i, "replica_index").parse().unwrap_or(0), text(i, "node_id")))
                .collect();
            running.sort();
            nodes_of.entry(name).or_default().extend(running.into_iter().map(|(_, n)| n));
        }
        let (group_of_node, hops) = self.topology(&account);
        for (caller, target) in &links {
            let cs = nodes_of.get(caller).cloned().unwrap_or_default();
            let ts = nodes_of.get(target).cloned().unwrap_or_default();
            if cs.is_empty() || ts.is_empty() {
                self.kpis.messages_unrouted += rate;
                continue;
            }
            let pairs: Vec<(String, String)> = match pattern.as_str() {
                "FanIn" => cs.iter().map(|c| (c.clone(), ts[0].clone())).collect(),
                "FanOut" => ts.iter().map(|t| (cs[0].clone(), t.clone())).collect(),
                "Pipeline" => cs.iter().enumerate().map(|(i, c)| (c.clone(), ts[i % ts.len()].clone())).collect(),
                "RequestResponse" => cs
                    .iter()
                    .enumerate()
                    .flat_map(|(i, c)| [(c.clone(), ts[i % ts.len()].clone()), (ts[i % ts.len()].clone(), c.clone())])
                    .collect(),
                "RingAllReduce" => {
                    let ring: Vec<String> = cs.iter().chain(&ts).cloned().collect();
                    (0..ring.len()).map(|i| (ring[i].clone(), ring[(i + 1) % ring.len()].clone())).collect()
                }
                _ => cs.iter().flat_map(|c| ts.iter().map(move |t| (c.clone(), t.clone()))).collect(),
            };
            for m in 0..rate {
                let (a, b) = &pairs[m % pairs.len()];
                if a == b {
                    self.kpis.messages += 1;
                    continue;
                }
                match (group_of_node.get(a), group_of_node.get(b)) {
                    (Some(ga), Some(gb)) => match hops.get(&(ga.clone(), gb.clone())).or(if ga == gb { Some(&0) } else { None }) {
                        Some(h) => {
                            self.kpis.messages += 1;
                            self.kpis.link_crossings += h;
                        }
                        None => self.kpis.messages_unrouted += 1,
                    },
                    _ => self.kpis.messages_unrouted += 1,
                }
            }
        }
    }

    fn run(&mut self) -> Result<(), String> {
        for step in self.exp.setup.clone() {
            self.send_repeated("setup", &step)?;
            if self.stopped {
                return Ok(());
            }
        }
        self.patterns(true);
        if self.stopped {
            return Ok(());
        }
        let last_fault = self.exp.faults.iter().map(|f| f.at).max().unwrap_or(0);
        let horizon = self.exp.ticks.max(last_fault + 1) + self.exp.converge_within;
        let mut dropped: BTreeMap<String, usize> = BTreeMap::new();
        let mut converged_at: Option<usize> = None;
        for tick in 0..horizon {
            self.sess.advance_clock(30);
            self.vars.insert("tick".into(), json!(tick));
            for fault in self.exp.faults.clone().iter().filter(|f| f.at == tick) {
                match fault.kind.as_str() {
                    "duplicate" => {
                        let target = fault.target.clone().unwrap_or_default();
                        let Some(req) = self.sent.get(&target).cloned() else {
                            let mut inner = self.sess.cov.inner.lock().expect("coverage");
                            inner.notes.push(format!("duplicate at tick {tick}: no step labelled `{target}` was sent"));
                            continue;
                        };
                        let before = self.fingerprint();
                        let answer = self.sess.send(&req);
                        let check = idempotency_check(&before, &self.fingerprint(), &answer);
                        let ok = check.ok;
                        if !ok {
                            self.kpis.duplicate_effects += 1;
                        }
                        if !ok && self.invariants.contains("idempotency") {
                            self.violations.push(Violation {
                                kind: "idempotency".into(),
                                detail: check.detail.clone(),
                                command: self.sess.cat.commands[req.cmd].name.to_string(),
                                walk: self.sess.walk,
                                seed: self.sess.seed,
                                step: self.sess.step,
                            });
                        }
                        self.after("fault", &format!("{target} again"), &req, &answer, Some("duplicate".into()), vec![check]);
                    }
                    "clock" => self.sess.advance_clock(fault.seconds),
                    "drop" => {
                        let target = fault.target.clone().unwrap_or_default();
                        dropped.insert(target, fault.ticks);
                    }
                    "node_loss" | "steps" => {
                        for n in &fault.silence {
                            let v = substitute(n, &self.vars, &[]);
                            if !v.is_null() {
                                self.lost.insert(key_text(&v), fault.silent_actor.clone());
                            }
                        }
                        for step in &fault.steps {
                            self.send_step("fault", step, &[], Some(fault.kind.clone()))?;
                        }
                    }
                    other => return Err(format!("fault kind `{other}`: expected duplicate, drop, clock or node_loss")),
                }
            }
            for t in self.exp.traffic.clone() {
                let number = |v: &Value| substitute(v, &self.vars, &[]).as_u64();
                let every = number(&t.every).unwrap_or(1) as usize;
                let from = t.from.as_ref().and_then(number).unwrap_or(0) as usize;
                let until = t.until.as_ref().and_then(number).map_or(usize::MAX, |u| u as usize);
                if every > 0 && tick >= from && tick <= until && (tick - from).is_multiple_of(every) {
                    if let Some(step) = &t.step {
                        self.send_repeated("traffic", step)?;
                    }
                }
            }
            let mut work = 0;
            for c in self.exp.controllers.clone() {
                if let Some(left) = dropped.get_mut(&c.name) {
                    if *left > 0 {
                        *left -= 1;
                        let mut inner = self.sess.cov.inner.lock().expect("coverage");
                        if self.record {
                            inner.timeline.push(TimelineEntry {
                                index: self.timeline.len() + 1,
                                phase: "fault".into(),
                                label: format!("{} skipped", c.name),
                                command: c.command.clone(),
                                outcome: "skipped".into(),
                                fault: Some("drop".into()),
                                checks: Vec::new(),
                            });
                        }
                        drop(inner);
                        work += 1;
                        continue;
                    }
                }
                let n = self.controller(&c)?;
                if !c.steady {
                    work += n;
                }
            }
            // The traffic of this tick flows between the instances running at its end.
            for t in self.exp.traffic.clone() {
                let number = |v: &Value| substitute(v, &self.vars, &[]).as_u64();
                let every = number(&t.every).unwrap_or(1) as usize;
                let from = t.from.as_ref().and_then(number).unwrap_or(0) as usize;
                let until = t.until.as_ref().and_then(number).map_or(usize::MAX, |u| u as usize);
                if every > 0 && tick >= from && tick <= until && (tick - from).is_multiple_of(every) {
                    self.traffic_tick(&t);
                }
            }
            if !self.stopped {
                self.patterns(true);
            }
            if self.stopped {
                return Ok(());
            }
            if tick >= last_fault {
                if work == 0 {
                    if converged_at.is_none() {
                        converged_at = Some(tick);
                    }
                } else {
                    converged_at = None;
                }
            }
        }
        match converged_at {
            Some(t) => self.kpis.steps_to_converge = t.saturating_sub(last_fault),
            None if self.exp.controllers.iter().any(|c| !c.steady) => {
                self.kpis.unconverged = 1;
                self.kpis.steps_to_converge = horizon - last_fault;
                self.violations.push(Violation {
                    kind: "convergence".into(),
                    detail: format!("the controllers still had work {} ticks after the last fault", self.exp.converge_within),
                    command: String::new(),
                    walk: self.sess.walk,
                    seed: self.sess.seed,
                    step: self.sess.step,
                });
            }
            None => {}
        }
        Ok(())
    }
}

/// Runs one point of an experiment; `record` writes its timeline to the page.
pub fn simulate_one(
    cat: &Catalog,
    cov: &Coverage,
    cands: &Candidates,
    exp: &Experiment,
    walk: u64,
    record: bool,
) -> Result<Kpis, String> {
    let mut sim = new_sim(cat, cov, cands, exp, walk, record)?;
    let result = sim.run();
    finish_one(sim, cov, exp, walk, record, result)
}

/// The setup of an experiment, run once: every request it sent with the answer, so a walk can
/// replay it from an empty store (the interpreter's store has no copy: `Interpreted` holds it
/// privately, and its scenario is reset by `begin_scenario`).
pub struct SetupRun {
    pub steps: Vec<Recorded>,
    pub refusals: Vec<String>,
    pub seconds: f64,
    /// Requests of the setup alone; the rest of `steps` are the settle's.
    pub setup_steps: usize,
    /// Settle ticks run, and the tick at which the controllers had no work left (`None`: they
    /// still had work after the last tick).
    pub settle_ticks: usize,
    pub settled_at: Option<usize>,
    /// The experiment's variables after the setup bound its answers.
    pub vars: BTreeMap<String, Value>,
    /// The experiment's invariants broken after a setup or settle step (checked after each), and
    /// `no-convergence` when the controllers still had work after `settle` ticks.
    pub violations: Vec<Violation>,
}

/// Runs `exp`'s `setup:` (no traffic, faults or patterns) on walk 0, then up to `settle` ticks of
/// its controllers (30 s of clock each) until a tick in which no controller but a `steady` one
/// sent anything. The outcomes it reaches count in `cov` once. A setup step not accepted as
/// declared is in `refusals`.
pub fn record_setup(cat: &Catalog, cov: &Coverage, cands: &Candidates, exp: &Experiment, settle: usize) -> Result<SetupRun, String> {
    let started = std::time::Instant::now();
    let mut only = exp.clone();
    only.find.patterns.clear();
    let mut sim = new_sim(cat, cov, cands, &only, 0, false)?;
    sim.logging = true;
    for step in only.setup.clone() {
        sim.send_repeated("setup", &step)?;
    }
    let setup_steps = sim.log.len();
    let mut settle_ticks = 0;
    let mut settled_at = None;
    if sim.kpis.setup_refusals.is_empty() {
        for tick in 0..settle {
            settle_ticks += 1;
            let work = sim.controller_tick(tick)?;
            if work == 0 {
                settled_at = Some(tick);
                break;
            }
        }
        if settle > 0 && settled_at.is_none() {
            let detail = format!("the setup's controllers still had work after {settle} ticks: {}", sim.busy());
            sim.violations.push(Violation { kind: "no-convergence".into(), detail, command: String::new(), walk: 0, seed: exp.seed, step: sim.sess.step });
        }
    }
    Ok(SetupRun {
        steps: sim.log.drain(..).map(|(req, answer, at)| Recorded { req, answer: answer.label, at }).collect(),
        refusals: std::mem::take(&mut sim.kpis.setup_refusals),
        seconds: started.elapsed().as_secs_f64(),
        setup_steps,
        settle_ticks,
        settled_at,
        vars: sim.vars.clone(),
        violations: std::mem::take(&mut sim.violations),
    })
}

impl Sim<'_, '_> {
    /// One tick of the experiment's controllers alone: the clock moves 30 s, each controller runs
    /// its rounds. The requests sent by controllers that are not `steady`.
    fn controller_tick(&mut self, tick: usize) -> Result<usize, String> {
        self.sess.advance_clock(30);
        self.vars.insert("tick".into(), json!(tick));
        let mut work = 0;
        self.last_work.clear();
        for c in self.exp.controllers.clone() {
            let n = self.controller(&c)?;
            if !c.steady {
                work += n;
                if n > 0 {
                    self.last_work.push((c.name.clone(), n));
                }
            }
        }
        Ok(work)
    }

    /// The controllers that sent requests in the last tick, with the count: `placer 3, renew 1`.
    fn busy(&self) -> String {
        self.last_work.iter().map(|(c, n)| format!("{c} {n}")).collect::<Vec<_>>().join(", ")
    }
}

/// A walk's convergence check: the controllers ticked with no new input until a tick with no
/// work, at most `max` ticks.
pub struct Settle {
    /// (request, answer, clock before the send) of every request the ticks sent.
    pub sends: Vec<(Request, Answer, u64)>,
    pub ticks: usize,
    /// The tick with no work; `None`: the controllers still had work after `max` ticks.
    pub settled_at: Option<usize>,
    /// The controllers that had work in the last tick, with their requests.
    pub busy: String,
    pub error: Option<String>,
}

/// The experiment's controllers, run inside explore walks: a tick swaps the walk's session in,
/// runs every controller once (as a settle tick), and hands back what they sent.
pub struct Ticker<'a, 'c> {
    sim: Sim<'a, 'c>,
    ticks: usize,
    /// The share of walk steps that are a controller tick instead of a random command.
    pub share: f64,
}

impl<'a, 'c> Ticker<'a, 'c> {
    pub fn new(
        cat: &'a Catalog,
        cov: &'a Coverage,
        cands: &'c Candidates,
        exp: &'c Experiment,
        vars: &BTreeMap<String, Value>,
    ) -> Result<Self, String> {
        let mut sim = new_sim(cat, cov, cands, exp, 0, false)?;
        sim.vars = vars.clone();
        sim.logging = true;
        sim.light = true;
        Ok(Self { sim, ticks: 0, share: 0.0 })
    }

    /// Starts a walk: the controllers' choices depend on its seed only.
    pub fn begin(&mut self, seed: u64) {
        self.sim.rng = Rng::new(seed ^ 0x5e77_1e5e_77e5);
        self.sim.spread.clear();
        self.ticks = 0;
    }

    /// One controller tick on the walk's session: (request, answer, clock before the send) of
    /// every request sent, and the first controller error.
    pub fn tick(&mut self, sess: &mut Session<'a>) -> (Vec<(Request, Answer, u64)>, Option<String>) {
        std::mem::swap(&mut self.sim.sess, sess);
        let step_seconds = self.sim.sess.step_seconds;
        let result = self.sim.controller_tick(self.ticks);
        self.ticks += 1;
        self.sim.sess.step_seconds = step_seconds;
        std::mem::swap(&mut self.sim.sess, sess);
        (std::mem::take(&mut self.sim.log), result.err())
    }

    /// Convergence (sketch § 14: "Convergence is a checked property"): after a walk, its
    /// controllers tick with no new input until a tick in which no non-`steady` controller sent
    /// anything, at most `max` ticks.
    pub fn settle(&mut self, sess: &mut Session<'a>, max: usize) -> Settle {
        std::mem::swap(&mut self.sim.sess, sess);
        let step_seconds = self.sim.sess.step_seconds;
        let mut out = Settle { sends: Vec::new(), ticks: 0, settled_at: None, busy: String::new(), error: None };
        for tick in 0..max {
            out.ticks += 1;
            let result = self.sim.controller_tick(self.ticks);
            self.ticks += 1;
            match result {
                Ok(0) => {
                    out.settled_at = Some(tick);
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    out.error = Some(e);
                    break;
                }
            }
        }
        out.busy = self.sim.busy();
        self.sim.sess.step_seconds = step_seconds;
        std::mem::swap(&mut self.sim.sess, sess);
        out.sends = std::mem::take(&mut self.sim.log);
        out
    }
}

fn new_sim<'a, 'c>(
    cat: &'a Catalog,
    cov: &'a Coverage,
    cands: &'c Candidates,
    exp: &'c Experiment,
    walk: u64,
    record: bool,
) -> Result<Sim<'a, 'c>, String> {
    let mut sess = Session::new(cat, cov, bound_emits(cat));
    sess.step_seconds = 0;
    sess.begin(walk, exp.seed)?;
    let invariants: BTreeSet<String> = if exp.invariants.is_empty() {
        ALL_INVARIANTS.iter().map(|s| (*s).to_owned()).collect()
    } else {
        exp.invariants.iter().cloned().collect()
    };
    Ok(Sim {
        sess,
        cands,
        exp,
        vars: exp.vars.clone(),
        rng: Rng::new(exp.seed),
        record,
        timeline: Vec::new(),
        kpis: Kpis::default(),
        sent: BTreeMap::new(),
        reached: HashSet::new(),
        states: HashSet::new(),
        violations: Vec::new(),
        stopped: false,
        invariants,
        lost: BTreeMap::new(),
        accepted: BTreeSet::new(),
        spread: BTreeMap::new(),
        log: Vec::new(),
        logging: false,
        light: false,
        last_work: Vec::new(),
    })
}

fn finish_one(mut sim: Sim, cov: &Coverage, exp: &Experiment, walk: u64, record: bool, result: Result<(), String>) -> Result<Kpis, String> {
    sim.kpis.chain_reached = exp.chain.iter().take_while(|c| sim.accepted.contains(*c)).count();
    sim.kpis.chain_stop = exp.chain.get(sim.kpis.chain_reached).cloned().unwrap_or_default();
    sim.kpis.outcomes_reached = sim.reached.len();
    sim.kpis.abstract_states = sim.states.len();
    let interpreter_violations = sim.sess.findings.violations.len();
    sim.kpis.violations = sim.violations.len() + interpreter_violations;
    let mut findings = std::mem::take(&mut sim.sess.findings);
    if record {
        findings.violations.extend(sim.violations.clone());
    } else {
        findings.violations.clear();
    }
    findings.abstract_states.extend(sim.states.iter().copied());
    cov.merge(walk, exp.seed, findings);
    result.map(|()| sim.kpis)
}

fn apply_point(exp: &Experiment, point: &BTreeMap<String, Value>) -> Result<Experiment, String> {
    let mut out = exp.clone();
    for m in &exp.mutations {
        let Some(value) = point.get(&m.name) else { continue };
        match m.kind.as_str() {
            "var" | "setup" | "traffic" => {
                out.vars.insert(m.point.clone(), value.clone());
            }
            "vars" => {
                let map = value.as_object().ok_or_else(|| format!("mutation {}: a `vars` value is a map of variables", m.name))?;
                for (k, v) in map {
                    out.vars.insert(k.clone(), v.clone());
                }
            }
            "fault" => {
                let parts: Vec<&str> = m.point.split('.').collect();
                let (Some(index), Some(field)) = (parts.get(1).and_then(|i| i.parse::<usize>().ok()), parts.get(2)) else {
                    return Err(format!("mutation {}: point `{}` is not faults.<i>.<at|target>", m.name, m.point));
                };
                let fault = out.faults.get_mut(index).ok_or_else(|| format!("mutation {}: no fault {index}", m.name))?;
                match *field {
                    "at" => fault.at = value.as_u64().ok_or("fault tick must be a number")? as usize,
                    "target" => fault.target = value.as_str().map(ToOwned::to_owned),
                    "ticks" => fault.ticks = value.as_u64().unwrap_or(1) as usize,
                    other => return Err(format!("mutation {}: fault field `{other}`", m.name)),
                }
            }
            other => return Err(format!("mutation {}: kind `{other}` (var, vars, fault; spec variants come later)", m.name)),
        }
    }
    Ok(out)
}

fn fitness(search: &crate::experiment::Search, k: &Kpis) -> (f64, bool) {
    let feasible = search.gates.iter().all(|g| k.get(g) == 0.0);
    let f = search.weights.iter().map(|(name, w)| w * k.get(name)).sum();
    (f, feasible)
}

fn evaluate(
    cat: &Catalog,
    cov: &Coverage,
    cands: &Candidates,
    exp: &Experiment,
    points: &[BTreeMap<String, Value>],
    threads: usize,
    base_walk: u64,
) -> Vec<Result<Kpis, String>> {
    let results: Mutex<Vec<Option<Result<Kpis, String>>>> = Mutex::new(vec![None; points.len()]);
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..threads.clamp(1, 8).min(points.len().max(1)) {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= points.len() {
                    break;
                }
                let r = apply_point(exp, &points[i]).and_then(|e| simulate_one(cat, cov, cands, &e, base_walk + i as u64, false));
                results.lock().expect("results")[i] = Some(r);
            });
        }
    });
    results.into_inner().expect("results").into_iter().map(|r| r.unwrap_or_else(|| Err("not run".into()))).collect()
}

fn record_point(cov: &Coverage, point: &BTreeMap<String, Value>, result: &Result<Kpis, String>, search: &crate::experiment::Search) -> Option<(f64, bool)> {
    let mut inner = cov.inner.lock().expect("coverage");
    match result {
        Ok(k) => {
            let (f, feasible) = fitness(search, k);
            inner.search.push(json!({"point": point, "kpis": k, "fitness": f, "feasible": feasible}));
            Some((f, feasible))
        }
        Err(e) => {
            inner.search.push(json!({"point": point, "kpis": {}, "fitness": 0, "feasible": false, "error": e}));
            None
        }
    }
}

pub fn run(cat: &Catalog, cov: &Coverage, exp: &Experiment, threads: usize) -> Result<Value, String> {
    let cands = Candidates::default();
    let Some(search) = exp.search.clone() else {
        let k = simulate_one(cat, cov, &cands, exp, 0, true)?;
        return Ok(json!({"kpis": k, "checks": cov.inner.lock().map(|i| i.timeline.iter().map(|t| t.checks.len()).sum::<usize>()).unwrap_or(0)}));
    };
    if exp.mutations.is_empty() {
        return Err("search: the experiment names no mutations".into());
    }
    let mut best: Option<(f64, BTreeMap<String, Value>)> = None;
    let consider = |point: &BTreeMap<String, Value>, scored: Option<(f64, bool)>, best: &mut Option<(f64, BTreeMap<String, Value>)>| {
        if let Some((f, true)) = scored {
            if best.as_ref().is_none_or(|(b, _)| f > *b) {
                *best = Some((f, point.clone()));
            }
        }
    };
    match search.mode.as_str() {
        "exhaustive" => {
            let size: usize = exp.mutations.iter().map(|m| m.values.len().max(1)).product();
            eprintln!("weft-sim: exhaustive search over {} mutation points: {size} combinations (limit {})", exp.mutations.len(), search.limit);
            if size > search.limit {
                return Err(format!("exhaustive search refused: {size} combinations exceed the limit {}", search.limit));
            }
            let mut points = Vec::new();
            for mut index in 0..size {
                let mut point = BTreeMap::new();
                for m in &exp.mutations {
                    let n = m.values.len().max(1);
                    point.insert(m.name.clone(), m.values.get(index % n).cloned().unwrap_or(Value::Null));
                    index /= n;
                }
                points.push(point);
            }
            let results = evaluate(cat, cov, &cands, exp, &points, threads, 1);
            for (p, r) in points.iter().zip(&results) {
                let scored = record_point(cov, p, r, &search);
                consider(p, scored, &mut best);
            }
        }
        "fit" => {
            let mut rng = Rng::new(exp.seed);
            let mut current: Vec<usize> = exp.mutations.iter().map(|_| 0).collect();
            let to_point = |idx: &[usize]| -> BTreeMap<String, Value> {
                exp.mutations.iter().zip(idx).map(|(m, i)| (m.name.clone(), m.values.get(*i).cloned().unwrap_or(Value::Null))).collect()
            };
            let mut seen: BTreeSet<Vec<usize>> = BTreeSet::new();
            let mut walk = 1u64;
            let first = evaluate(cat, cov, &cands, exp, &[to_point(&current)], threads, walk);
            walk += 1;
            seen.insert(current.clone());
            let mut current_score = record_point(cov, &to_point(&current), &first[0], &search).map(|(f, ok)| if ok { f } else { f64::MIN }).unwrap_or(f64::MIN);
            consider(&to_point(&current), record_point_noop(&first[0], &search), &mut best);
            let mut evaluated = 1;
            while evaluated < search.budget {
                let mut neighbours = Vec::new();
                for (mi, m) in exp.mutations.iter().enumerate() {
                    for delta in [-1i64, 1] {
                        let v = current[mi] as i64 + delta;
                        if v >= 0 && (v as usize) < m.values.len() {
                            let mut n = current.clone();
                            n[mi] = v as usize;
                            if !seen.contains(&n) {
                                neighbours.push(n);
                            }
                        }
                    }
                }
                if neighbours.is_empty() {
                    // A local optimum: restart from a random point not visited.
                    let n: Vec<usize> = exp.mutations.iter().map(|m| rng.below(m.values.len().max(1))).collect();
                    if seen.contains(&n) {
                        if seen.len() >= exp.mutations.iter().map(|m| m.values.len().max(1)).product::<usize>() {
                            break;
                        }
                        continue;
                    }
                    neighbours.push(n);
                }
                neighbours.truncate(search.budget - evaluated);
                let points: Vec<_> = neighbours.iter().map(|n| to_point(n)).collect();
                let results = evaluate(cat, cov, &cands, exp, &points, threads, walk);
                walk += points.len() as u64;
                evaluated += points.len();
                let mut improved: Option<(f64, Vec<usize>)> = None;
                for ((n, p), r) in neighbours.iter().zip(&points).zip(&results) {
                    seen.insert(n.clone());
                    let scored = record_point(cov, p, r, &search);
                    consider(p, scored, &mut best);
                    if let Some((f, true)) = scored {
                        if f > current_score && improved.as_ref().is_none_or(|(b, _)| f > *b) {
                            improved = Some((f, n.clone()));
                        }
                    }
                }
                match improved {
                    Some((f, n)) => {
                        current = n;
                        current_score = f;
                    }
                    None => {
                        current = exp.mutations.iter().map(|m| rng.below(m.values.len().max(1))).collect();
                        current_score = f64::MIN;
                    }
                }
            }
        }
        other => return Err(format!("search mode `{other}`: expected fit or exhaustive")),
    }
    // The best point runs once more with its timeline recorded.
    let (best_fitness, best_point) = best.clone().unwrap_or((0.0, BTreeMap::new()));
    let best_exp = apply_point(exp, &best_point)?;
    let k = simulate_one(cat, cov, &cands, &best_exp, 0, true)?;
    let points = cov.inner.lock().map(|i| i.search.len()).unwrap_or(0);
    Ok(json!({"mode": search.mode, "points": points, "best": {"point": best_point, "fitness": best_fitness, "kpis": k}, "checks": cov.inner.lock().map(|i| i.timeline.iter().map(|t| t.checks.len()).sum::<usize>()).unwrap_or(0)}))
}

fn record_point_noop(result: &Result<Kpis, String>, search: &crate::experiment::Search) -> Option<(f64, bool)> {
    result.as_ref().ok().map(|k| fitness(search, k))
}
