//! The experiment file (`weft-sim-experiment/1`, YAML): a setup sequence, traffic, a fault
//! schedule, controllers driven as the reconciliation loops would, invariants, mutation points,
//! a search and patterns over rows.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::walk::Session;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Experiment {
    pub format: String,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default = "default_seed")]
    pub seed: u64,
    /// Variables: `${name}` in any string of a step reads one.
    #[serde(default)]
    pub vars: BTreeMap<String, Value>,
    pub setup: Vec<Step>,
    #[serde(default)]
    pub traffic: Vec<Traffic>,
    #[serde(default)]
    pub faults: Vec<Fault>,
    #[serde(default)]
    pub controllers: Vec<Controller>,
    /// Ticks after the last fault in which the controllers must converge.
    #[serde(default = "default_converge")]
    pub converge_within: usize,
    #[serde(default)]
    pub ticks: usize,
    #[serde(default)]
    pub invariants: Vec<String>,
    #[serde(default)]
    pub mutations: Vec<Mutation>,
    #[serde(default)]
    pub search: Option<Search>,
    #[serde(default)]
    pub find: Find,
    /// Commands in the order a full setup reaches them; KPI `chain-reached` counts the leading
    /// ones the model accepted at least once, and the result names the first it never accepted.
    #[serde(default)]
    pub chain: Vec<String>,
    /// Bindings (by id) whose every delivery is lost for the whole run: the model is compiled
    /// without them, so only a level step the experiment runs as a controller repairs their gap.
    #[serde(default)]
    pub lost_bindings: Vec<String>,
}

fn default_seed() -> u64 {
    1
}
fn default_converge() -> usize {
    6
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub command: String,
    #[serde(default)]
    pub actor: Option<String>,
    /// The actor whose frame this step records (weft-control's channel endpoint, the ControlPlane,
    /// records a node agent's frame with the time it stamped): a fault's `silence` compares this
    /// actor with `silent_actor` instead of `actor`.
    #[serde(default)]
    pub relays: Option<String>,
    #[serde(default)]
    pub caller: BTreeMap<String, Value>,
    #[serde(default)]
    pub input: BTreeMap<String, Value>,
    /// The outcome this step must answer; the setup stops otherwise.
    #[serde(default)]
    pub expect: Option<String>,
    /// Variables bound from the answer: `name: field` of an emitted event or the response.
    #[serde(default)]
    pub bind: BTreeMap<String, String>,
    #[serde(default)]
    pub label: Option<String>,
    /// Repeat the step this many times (a number or `${var}`; `${i}` reads the repetition, from 0).
    #[serde(default)]
    pub repeat: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Traffic {
    pub name: String,
    /// One request every `every` ticks (a number or `${var}`).
    #[serde(default = "one_value")]
    pub every: Value,
    /// The first tick it runs at.
    #[serde(default)]
    pub from: Option<Value>,
    /// The last tick it runs at.
    #[serde(default)]
    pub until: Option<Value>,
    /// A `weft.cluster.TrafficPattern` (`FanIn`, `Pipeline`, `AllToAll`, `FanOut`,
    /// `RequestResponse`, `RingAllReduce`) over the links of `formation`.
    #[serde(default)]
    pub pattern: Option<String>,
    /// The formation whose links carry the traffic (`${var}`); with `pattern` and `rate` the
    /// simulator counts the topology links each message crosses (KPI `link-crossings`).
    #[serde(default)]
    pub formation: Option<Value>,
    /// The account that owns the formation's cluster (reads its topology views).
    #[serde(default)]
    pub account: Option<Value>,
    /// Messages per tick on each formation link (a number or `${var}`).
    #[serde(default)]
    pub rate: Option<Value>,
    /// The model input the traffic turns into, sent once per run of this traffic.
    #[serde(default)]
    pub step: Option<Step>,
}

fn one() -> usize {
    1
}

fn one_value() -> Value {
    Value::from(1)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fault {
    /// The tick the fault fires at.
    pub at: usize,
    /// `duplicate`, `drop` or `node_loss`.
    pub kind: String,
    /// duplicate: the setup or traffic step label sent again; drop: the controller skipped.
    #[serde(default)]
    pub target: Option<String>,
    /// node_loss: the commands that mark the node gone.
    #[serde(default)]
    pub steps: Vec<Step>,
    /// drop: how many ticks the controller is skipped.
    #[serde(default = "one")]
    pub ticks: usize,
    /// clock: seconds the simulated clock moves.
    #[serde(default)]
    pub seconds: u64,
    /// node_loss: the nodes (`${var}`) that go silent from this tick on: no traffic step and no
    /// controller command of `silent_actor` whose input names one of them as `node_id` is sent
    /// any more. The control plane still sends to them: it does not know.
    #[serde(default)]
    pub silence: Vec<Value>,
    #[serde(default = "node_agent")]
    pub silent_actor: String,
}

fn node_agent() -> String {
    "weft.actors.NodeAgent".to_owned()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Controller {
    pub name: String,
    /// The gap view the controller reads.
    pub view: String,
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
    pub command: String,
    pub actor: String,
    /// As `Step::relays`.
    #[serde(default)]
    pub relays: Option<String>,
    #[serde(default)]
    pub caller: BTreeMap<String, Value>,
    /// Input field -> row field (`row.<field>`), candidate field (`cand.<field>`), varied value
    /// (`vary.<name>`), or a literal / `${var}`.
    pub input: BTreeMap<String, Value>,
    /// For each row, try candidates until one is accepted (a scheduler's choice of node).
    #[serde(default)]
    pub each: Option<Each>,
    /// At most this many rows a tick.
    #[serde(default = "default_rows")]
    pub limit: usize,
    /// Only rows whose fields equal these values (`${var}`).
    #[serde(default)]
    pub filter: BTreeMap<String, Value>,
    /// Steady work (a node agent's heartbeats and lease renewals): not counted when the
    /// simulator decides whether the controllers converged.
    #[serde(default)]
    pub steady: bool,
    /// Passes over the view a tick, while a pass sent something (default 1).
    #[serde(default)]
    pub rounds: usize,
}

fn default_rows() -> usize {
    16
}

/// The candidates a controller tries for one row, in order, until the model accepts one.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Each {
    pub view: String,
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
    /// Every combination of these values is tried with every candidate row (`vary.<name>`).
    #[serde(default)]
    pub vary: BTreeMap<String, Vec<Value>>,
    /// Start each row at the next candidate (round robin), so rows spread over the candidates.
    #[serde(default)]
    pub spread: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mutation {
    pub name: String,
    /// `var` (a setup or traffic value), `vars` (each value is a map of variables set together)
    /// or `fault` (the tick or target of a fault).
    pub kind: String,
    /// The variable it sets, or `faults.<index>.at` / `faults.<index>.target`; for `vars` a
    /// label of the point (the values name their variables).
    pub point: String,
    pub values: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Search {
    /// `fit` or `exhaustive`.
    pub mode: String,
    #[serde(default)]
    pub weights: BTreeMap<String, f64>,
    /// KPIs that must be 0 at a point, or the point is infeasible.
    #[serde(default)]
    pub gates: Vec<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_budget")]
    pub budget: usize,
}

fn default_limit() -> usize {
    256
}
fn default_budget() -> usize {
    24
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Find {
    #[serde(default)]
    pub gaps: bool,
    #[serde(default)]
    pub patterns: Vec<Pattern>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RowPattern {
    /// The simulator's shadow rows of an entity (fields the answers showed).
    #[serde(default)]
    pub entity: Option<String>,
    /// Or the rows of a view the model answers (every field the view declares).
    #[serde(default)]
    pub view: Option<String>,
    /// View parameters (`${var}`); a list reads the view once per value and joins the rows.
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
    /// The name `where` uses for this row (default `r<index>`).
    #[serde(default, rename = "as")]
    pub alias: Option<String>,
    /// The pattern matches only when no row of this kind satisfies `where` with the others.
    #[serde(default)]
    pub absent: bool,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub state_not: Option<String>,
    #[serde(default)]
    pub fields: BTreeMap<String, Value>,
}

/// A state pattern over rows: every row pattern matches a distinct row, the rows agree on every
/// field named in `same`, and every `where` condition holds (`a.field op b.field` or
/// `a.field op literal`, op one of `== != < <= > >=`; a missing or absent field fails the
/// condition).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pattern {
    pub name: String,
    pub rows: Vec<RowPattern>,
    #[serde(default)]
    pub same: Vec<String>,
    #[serde(default, rename = "where")]
    pub conditions: Vec<String>,
}

type Fields = BTreeMap<String, Value>;

#[derive(Debug, Clone)]
enum Operand {
    Field(usize, String),
    Literal(Value),
}

#[derive(Debug, Clone)]
struct Condition {
    left: Operand,
    op: &'static str,
    right: Operand,
}

impl Condition {
    fn rows(&self) -> Vec<usize> {
        [&self.left, &self.right]
            .iter()
            .filter_map(|o| match o {
                Operand::Field(r, _) => Some(*r),
                Operand::Literal(_) => None,
            })
            .collect()
    }

    fn holds(&self, chosen: &[Option<&Fields>]) -> bool {
        let read = |o: &Operand| -> Option<Value> {
            match o {
                Operand::Field(r, f) => chosen[*r].and_then(|row| row.get(f)).filter(|v| !v.is_null()).cloned(),
                Operand::Literal(v) => Some(v.clone()),
            }
        };
        let (Some(a), Some(b)) = (read(&self.left), read(&self.right)) else { return false };
        match self.op {
            "==" => same_value(&a, &b),
            "!=" => !same_value(&a, &b),
            op => {
                let (Some(x), Some(y)) = (a.as_f64(), b.as_f64()) else { return false };
                match op {
                    "<" => x < y,
                    "<=" => x <= y,
                    ">" => x > y,
                    _ => x >= y,
                }
            }
        }
    }
}

/// Equal as values; a text and a number of the same digits are equal (views answer numbers,
/// literals in a pattern are parsed).
fn same_value(a: &Value, b: &Value) -> bool {
    if a == b {
        return true;
    }
    match (a, b) {
        (Value::String(s), other) | (other, Value::String(s)) if !other.is_string() => {
            other.as_f64().is_some_and(|n| s.parse::<f64>().ok() == Some(n)) || s == &other.to_string()
        }
        _ => false,
    }
}

fn literal(text: &str) -> Value {
    let t = text.trim();
    if let Some(s) = t.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        return Value::String(s.to_owned());
    }
    if let Ok(n) = t.parse::<i64>() {
        return Value::from(n);
    }
    if let Ok(n) = t.parse::<f64>() {
        return Value::from(n);
    }
    match t {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => Value::String(t.to_owned()),
    }
}

impl Pattern {
    fn aliases(&self) -> Vec<String> {
        self.rows
            .iter()
            .enumerate()
            .map(|(i, r)| r.alias.clone().unwrap_or_else(|| format!("r{i}")))
            .collect()
    }

    fn conditions(&self) -> Result<Vec<Condition>, String> {
        let aliases = self.aliases();
        let operand = |t: &str| -> Operand {
            if let Some((a, f)) = t.trim().split_once('.') {
                if let Some(i) = aliases.iter().position(|x| x == a) {
                    return Operand::Field(i, f.to_owned());
                }
            }
            Operand::Literal(literal(t))
        };
        let mut out = Vec::new();
        for c in &self.conditions {
            let op = ["==", "!=", "<=", ">=", "<", ">"]
                .into_iter()
                .find(|op| c.contains(op))
                .ok_or_else(|| format!("pattern {}: `{c}` has no comparison", self.name))?;
            let (l, r) = c.split_once(op).expect("found above");
            out.push(Condition { left: operand(l), op, right: operand(r) });
        }
        Ok(out)
    }

    /// True when a row pattern reads a view: the pattern is then checked at the end of a tick,
    /// not after every step.
    pub fn reads_views(&self) -> bool {
        self.rows.iter().any(|r| r.view.is_some())
    }

    /// The matching rows, described, when the walk's rows match.
    pub fn matches(&self, sess: &Session) -> Option<Vec<String>> {
        self.matches_with(sess, &BTreeMap::new())
    }

    /// As `matches`, with `${var}` in view parameters read from `vars`.
    pub fn matches_with(&self, sess: &Session, vars: &BTreeMap<String, Value>) -> Option<Vec<String>> {
        let conditions = self.conditions().ok()?;
        let mut candidates: Vec<Vec<(String, Fields)>> = Vec::new();
        for rp in &self.rows {
            let list: Vec<(String, Fields)> = rows_of(sess, rp, vars)?
                .into_iter()
                .filter(|(_, row)| {
                    let state = row.get("state").and_then(Value::as_str);
                    rp.state.as_ref().is_none_or(|s| state == Some(s.as_str()))
                        && rp.state_not.as_ref().is_none_or(|s| state != Some(s.as_str()))
                        && rp.fields.iter().all(|(k, v)| row.get(k).is_some_and(|have| same_value(have, v)))
                })
                .collect();
            if list.is_empty() && !rp.absent {
                return None;
            }
            candidates.push(list);
        }
        let mut chosen: Vec<Option<usize>> = vec![None; self.rows.len()];
        let mut budget = 200_000usize;
        if self.search(&candidates, &conditions, 0, &mut chosen, &mut budget) {
            return Some(
                chosen
                    .iter()
                    .enumerate()
                    .filter_map(|(i, c)| c.map(|k| candidates[i][k].0.clone()))
                    .collect(),
            );
        }
        None
    }

    fn view_of<'r>(candidates: &'r [Vec<(String, Fields)>], chosen: &[Option<usize>]) -> Vec<Option<&'r Fields>> {
        chosen
            .iter()
            .enumerate()
            .map(|(i, c)| c.map(|k| &candidates[i][k].1))
            .collect()
    }

    fn search(
        &self,
        candidates: &[Vec<(String, Fields)>],
        conditions: &[Condition],
        depth: usize,
        chosen: &mut Vec<Option<usize>>,
        budget: &mut usize,
    ) -> bool {
        if *budget == 0 {
            return false;
        }
        *budget -= 1;
        if depth == self.rows.len() {
            let rows = Self::view_of(candidates, chosen);
            let same = self.same.iter().all(|field| {
                let values: Vec<Option<&Value>> = rows.iter().flatten().map(|r| r.get(field)).collect();
                values.iter().all(|v| v.is_some() && *v == values[0])
            });
            if !same {
                return false;
            }
            // Each absent row: no candidate satisfies the conditions that name it.
            for (i, rp) in self.rows.iter().enumerate() {
                if !rp.absent {
                    continue;
                }
                let named: Vec<&Condition> = conditions.iter().filter(|c| c.rows().contains(&i)).collect();
                for k in 0..candidates[i].len() {
                    chosen[i] = Some(k);
                    let rows = Self::view_of(candidates, chosen);
                    let all = named.iter().all(|c| c.holds(&rows));
                    chosen[i] = None;
                    if all {
                        return false;
                    }
                }
            }
            return true;
        }
        if self.rows[depth].absent {
            return self.search(candidates, conditions, depth + 1, chosen, budget);
        }
        for k in 0..candidates[depth].len() {
            let key = &candidates[depth][k].0;
            if chosen.iter().enumerate().any(|(i, c)| c.is_some_and(|c| candidates[i][c].0 == *key)) {
                continue;
            }
            chosen[depth] = Some(k);
            let rows = Self::view_of(candidates, chosen);
            // Conditions over chosen positive rows only.
            let ok = conditions.iter().all(|c| {
                let named = c.rows();
                if named.iter().any(|r| self.rows[*r].absent || rows[*r].is_none()) {
                    return true;
                }
                c.holds(&rows)
            });
            if ok && self.search(candidates, conditions, depth + 1, chosen, budget) {
                return true;
            }
            chosen[depth] = None;
        }
        false
    }
}

/// The rows a row pattern reads, each with a key that tells rows apart.
fn rows_of(sess: &Session, rp: &RowPattern, vars: &BTreeMap<String, Value>) -> Option<Vec<(String, Fields)>> {
    let cat = sess.cat;
    if let Some(view) = &rp.view {
        // Every combination of the parameter values.
        let mut combos: Vec<BTreeMap<String, ess_primitives::node::Node>> = vec![BTreeMap::new()];
        for (k, v) in &rp.params {
            let v = crate::simulate::substitute_vars(v, vars);
            let values: Vec<Value> = match v {
                Value::Array(items) => items,
                other => vec![other],
            };
            let mut next = Vec::new();
            for combo in &combos {
                for value in &values {
                    let Ok(node) = serde_json::from_value::<ess_primitives::node::Node>(value.clone()) else { continue };
                    let mut c = combo.clone();
                    c.insert(k.clone(), node);
                    next.push(c);
                }
            }
            combos = next;
        }
        let mut out = Vec::new();
        for params in combos {
            let rows = sess.view(view, params).ok()?;
            for row in rows {
                let fields: Fields = row
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::to_value(v).unwrap_or(Value::Null)))
                    .collect();
                let key = format!("{view} {}", serde_json::to_string(&fields).unwrap_or_default());
                out.push((key, fields));
            }
        }
        return Some(out);
    }
    let entity = rp.entity.as_ref()?;
    let &ent = cat.ent_index.get(&ess_domain::name::QualifiedName::new(entity).ok()?)?;
    let e = &cat.entities[ent];
    Some(
        sess.rows
            .iter()
            .filter(|((re, _), _)| *re == ent)
            .map(|((_, id), row)| {
                let mut fields: Fields = row
                    .fields
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::to_value(v).unwrap_or(Value::Null)))
                    .collect();
                let id_value = serde_json::to_value(id).unwrap_or(Value::Null);
                fields.insert(e.identity_field.clone(), id_value.clone());
                fields.insert("state".into(), Value::String(e.states[row.state].clone()));
                (format!("{} {} {}", e.name, id_value, e.states[row.state]), fields)
            })
            .collect(),
    )
}

pub fn load(path: &Path) -> Result<Experiment, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let experiment: Experiment =
        ess_yaml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if experiment.format != "weft-sim-experiment/1" {
        return Err(format!(
            "{}: format `{}`, expected `weft-sim-experiment/1`",
            path.display(),
            experiment.format
        ));
    }
    Ok(experiment)
}
