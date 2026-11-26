//! One session on the interpreted target: send a command, read its answer, keep what the walk can
//! name next (the pool), and keep a shadow of the rows the answers show.
//!
//! The interpreter's store is private to `ess-conformance` (no accessor on `Interpreted`), so the
//! rows here are derived: from each declared outcome's subject and move, its `sets:` from input and
//! literals, and the events of commands a binding ran. A set subject (`instances:`) or `affects:`
//! counts its transition but moves no shadow row.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use ess_conformance::interpret::Interpreted;
use ess_conformance::occurrence_clock::{CommandClock, DecisionInstant};
use ess_conformance::scenario::{
    ActorRef, AuthoredName, BindingRef, CommandRef, DomainRef, EventRef, OutcomeRef, ScenarioId,
};
use ess_conformance::target::{
    ConformanceTarget, Deadline, EventObservationRequest, ExternalOutcomeControl,
    InvocationObservationRequest, ObservedEvent, ScenarioContext, SemanticCommandRequest,
    SemanticViewRequest, TargetError,
};
use ess_domain::name::QualifiedName;
use ess_primitives::ids::CorrelationId;
use ess_primitives::node::Node;
use ess_primitives::time::Timestamp;
use serde_json::{json, Value};

use crate::catalog::{Catalog, Eff, FieldKind, InstanceSrc, SetSrc};
use crate::coverage::{Coverage, WalkFindings};
use crate::gen::{self, Input};

/// The simulated wall clock: seconds since the epoch, moved by the walk.
#[derive(Clone)]
pub struct SimClock(pub Arc<AtomicU64>);

/// 2026-01-01T00:00:00Z.
pub const CLOCK_START: u64 = 1_767_225_600;

impl CommandClock for SimClock {
    fn read(&self) -> Option<DecisionInstant> {
        let seconds = self.0.load(Ordering::Relaxed);
        DecisionInstant::parse(&Timestamp::from_epoch_millis(seconds * 1000).iso_8601()).ok()
    }
}

#[derive(Debug, Clone)]
pub struct Row {
    pub state: usize,
    pub fields: BTreeMap<String, Node>,
    /// Fields whose value here is a stand-in, not the stored value: a literal or a counter the
    /// shadow cannot type, or a `sets:` it cannot compute (`true`). A guard tie never copies one.
    pub guessed: BTreeSet<String>,
}

impl Row {
    /// The value of `field` as the answers showed it, when the shadow holds one.
    pub fn known(&self, field: &str) -> Option<&Node> {
        self.fields.get(field).filter(|_| !self.guessed.contains(field))
    }
}

#[derive(Debug, Clone)]
pub struct Request {
    pub cmd: usize,
    pub actor: Option<usize>,
    pub caller: Option<BTreeMap<String, Node>>,
    pub input: Input,
    pub force: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct Answer {
    /// The outcome gid, when a declared outcome answered.
    pub outcome: Option<usize>,
    pub label: String,
    pub events: Vec<ObservedEvent>,
    pub response: Option<BTreeMap<String, Node>>,
    pub error: Option<(String, String)>,
    /// Outcomes of commands bindings ran, inferred from their events.
    pub bound: Vec<usize>,
    /// The events of commands bindings ran after this command.
    pub bound_events: Vec<ObservedEvent>,
}

impl Answer {
    pub fn refused(&self) -> bool {
        self.outcome.is_none() || self.error.is_some()
    }
}

pub struct Session<'a> {
    pub cat: &'a Catalog,
    pub cov: &'a Coverage,
    pub target: Interpreted,
    pub clock: SimClock,
    pub rows: BTreeMap<(usize, Node), Row>,
    pub pool: HashMap<String, Vec<Node>>,
    /// Values seen together in one answer, event or accepted input.
    pub links: BTreeMap<Node, BTreeSet<Node>>,
    pub correlation: CorrelationId,
    pub walk: u64,
    pub seed: u64,
    /// Seconds the clock moves on each command (1 in explore; 0 in a simulation, whose ticks move it).
    pub step_seconds: u64,
    pub step: usize,
    pub findings: WalkFindings,
    /// Explore's input drawing: live row ids, guard ties, shadow rows filled at creation. Off in a
    /// simulation, whose experiment names its inputs.
    pub guided: bool,
    seen_sequence: u64,
    invocations: HashMap<String, usize>,
    /// event -> (command, outcome) of commands bindings run that emit it.
    bound_emits: Arc<HashMap<QualifiedName, Vec<(usize, usize)>>>,
}

pub fn bound_emits(cat: &Catalog) -> Arc<HashMap<QualifiedName, Vec<(usize, usize)>>> {
    let mut commands = BTreeSet::new();
    for list in cat.bindings_by_event.values() {
        for b in list {
            commands.insert(b.command);
        }
    }
    let mut map: HashMap<QualifiedName, Vec<(usize, usize)>> = HashMap::new();
    for c in commands {
        let command = &cat.ir.commands()[&cat.commands[c].name];
        for (oi, outcome) in command.outcomes.iter().enumerate() {
            for e in &outcome.emits {
                map.entry(e.name().clone()).or_default().push((c, oi));
            }
        }
    }
    Arc::new(map)
}

/// Replaces every UUID-shaped run of a message with `<id>`, so equal causes count as one.
pub fn normalize(text: &str) -> String {
    let bytes: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let shape = [8usize, 4, 4, 4, 12];
    while i < bytes.len() {
        let mut j = i;
        let mut ok = true;
        for (k, len) in shape.iter().enumerate() {
            for _ in 0..*len {
                if j < bytes.len() && bytes[j].is_ascii_hexdigit() {
                    j += 1;
                } else {
                    ok = false;
                    break;
                }
            }
            if !ok {
                break;
            }
            if k < 4 {
                if j < bytes.len() && bytes[j] == '-' {
                    j += 1;
                } else {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            out.push_str("<id>");
            i = j;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    let mut short: String = out.lines().next().unwrap_or("").chars().take(300).collect();
    if out.lines().count() > 1 {
        short.push_str(" …");
    }
    short
}

fn bucket(node: &Node, kind: FieldKind) -> u64 {
    match kind {
        FieldKind::Bool => match node {
            Node::Bool(true) => 1,
            _ => 0,
        },
        FieldKind::Num => {
            let text = match node {
                Node::Number(n) => n.to_string(),
                Node::Text(t) => t.clone(),
                _ => return 3,
            };
            let value: f64 = text.parse().unwrap_or(2.0);
            if value == 0.0 {
                0
            } else if value == 1.0 {
                1
            } else {
                2
            }
        }
        FieldKind::Enum => {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            serde_json::to_string(node).unwrap_or_default().hash(&mut h);
            h.finish()
        }
    }
}

impl<'a> Session<'a> {
    pub fn new(cat: &'a Catalog, cov: &'a Coverage, emits: Arc<HashMap<QualifiedName, Vec<(usize, usize)>>>) -> Self {
        let clock = SimClock(Arc::new(AtomicU64::new(CLOCK_START)));
        let target = Interpreted::for_model(cat.ir.clone()).with_command_clock(clock.clone());
        Self {
            cat,
            cov,
            target,
            clock,
            rows: BTreeMap::new(),
            pool: HashMap::new(),
            links: BTreeMap::new(),
            correlation: CorrelationId::new("walk-0").expect("correlation"),
            walk: 0,
            seed: 0,
            step_seconds: 1,
            step: 0,
            findings: WalkFindings::default(),
            guided: false,
            seen_sequence: 0,
            invocations: HashMap::new(),
            bound_emits: emits,
        }
    }

    pub fn begin(&mut self, walk: u64, seed: u64) -> Result<(), String> {
        self.walk = walk;
        self.seed = seed;
        self.step = 0;
        self.rows.clear();
        self.pool.clear();
        self.links.clear();
        self.findings = WalkFindings::default();
        self.seen_sequence = 0;
        self.invocations.clear();
        self.clock.0.store(CLOCK_START, Ordering::Relaxed);
        self.correlation = CorrelationId::new(format!("walk-{walk}")).map_err(|e| e.to_string())?;
        let scenario = ScenarioId::Authored {
            domain: DomainRef::new(QualifiedName::new("weft.sim").map_err(|e| e.to_string())?),
            name: AuthoredName::new(format!("walk-{walk}")).map_err(|e| e.to_string())?,
        };
        self.target
            .begin_scenario(&ScenarioContext::new(scenario, self.correlation.clone()))
            .map_err(|e| e.to_string())
    }

    pub fn advance_clock(&self, seconds: u64) {
        self.clock.0.fetch_add(seconds, Ordering::Relaxed);
    }

    /// The abstract state: per (entity, state, guard-read field buckets) a row count bucketed 1 or
    /// many, hashed; with a short description.
    pub fn abstract_state(&self, describe: bool) -> (u64, String) {
        let mut counts: BTreeMap<(usize, usize, u64), u8> = BTreeMap::new();
        for ((ent, _), row) in &self.rows {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            for (field, kind) in &self.cat.entities[*ent].tracked {
                match row.fields.get(field) {
                    Some(v) => bucket(v, *kind).hash(&mut h),
                    None => 9u64.hash(&mut h),
                }
            }
            let c = counts.entry((*ent, row.state, h.finish())).or_insert(0);
            *c = (*c + 1).min(2);
        }
        let mut h = std::collections::hash_map::DefaultHasher::new();
        counts.hash(&mut h);
        let desc = if describe {
            let mut parts: BTreeMap<String, u32> = BTreeMap::new();
            for ((ent, state, _), c) in &counts {
                let e = &self.cat.entities[*ent];
                let short = e.name.to_string().rsplit('.').next().unwrap_or("").to_owned();
                *parts.entry(format!("{short}:{}", e.states[*state])).or_default() += u32::from(*c);
            }
            let text: Vec<String> = parts
                .into_iter()
                .map(|(k, c)| if c > 1 { format!("{k}×{c}+") } else { k })
                .collect();
            let joined = text.join(" ");
            if joined.is_empty() {
                "(empty)".to_owned()
            } else {
                joined.chars().take(400).collect()
            }
        } else {
            String::new()
        };
        (h.finish(), desc)
    }

    fn remember(&mut self, items: Vec<(String, Node)>) {
        if items.len() > 1 {
            for (_, a) in items.iter().take(12) {
                for (_, b) in items.iter().take(12) {
                    if a != b {
                        self.links.entry(a.clone()).or_default().insert(b.clone());
                    }
                }
            }
        }
        for (t, v) in items {
            let list = self.pool.entry(t).or_default();
            if let Some(pos) = list.iter().position(|x| *x == v) {
                let moved = list.remove(pos);
                list.push(moved);
            } else {
                list.push(v);
                if list.len() > 24 {
                    list.remove(0);
                }
            }
        }
    }

    /// A value of `t` seen together with one of `chosen`, when there is one.
    pub fn pool_pick_linked(&self, t: &str, chosen: &[Node], rng: &mut crate::rng::Rng) -> Option<Node> {
        let list = self.pool.get(t)?;
        let linked: Vec<&Node> = list
            .iter()
            .filter(|v| chosen.iter().any(|c| self.links.get(c).is_some_and(|l| l.contains(*v))))
            .collect();
        if linked.is_empty() {
            return self.pool_pick(t, rng);
        }
        Some(linked[rng.below(linked.len())].clone())
    }

    pub fn pool_pick(&self, t: &str, rng: &mut crate::rng::Rng) -> Option<Node> {
        let list = self.pool.get(t)?;
        if list.is_empty() {
            return None;
        }
        // Recent values more often: the last four take 60 %.
        if list.len() > 4 && rng.chance(0.6) {
            return Some(list[list.len() - 1 - rng.below(4)].clone());
        }
        Some(list[rng.below(list.len())].clone())
    }

    /// The id of a row of `ent` the walk holds: one seen with `chosen` where there is one, one in
    /// a state of `states` where given and held, and one not in a terminal state more often.
    pub fn live_pick(&self, ent: usize, chosen: &[Node], states: &[usize], rng: &mut crate::rng::Rng) -> Option<&Node> {
        let e = &self.cat.entities[ent];
        // `Node::Null` orders first: the rows of `ent` are one range of the map.
        let mut live: Vec<(&Node, &Row)> = self
            .rows
            .range((ent, Node::Null)..)
            .take_while(|((re, _), _)| *re == ent)
            .map(|((_, id), row)| (id, row))
            .collect();
        if live.is_empty() {
            return None;
        }
        if !states.is_empty() {
            let held: Vec<(&Node, &Row)> = live.iter().copied().filter(|(_, r)| states.contains(&r.state)).collect();
            if !held.is_empty() {
                live = held;
            }
        }
        if !chosen.is_empty() && rng.chance(0.7) {
            let linked: Vec<(&Node, &Row)> = live
                .iter()
                .copied()
                .filter(|(id, _)| chosen.iter().any(|c| self.links.get(c).is_some_and(|l| l.contains(*id))))
                .collect();
            if !linked.is_empty() {
                live = linked;
            }
        }
        if rng.chance(0.75) {
            let open: Vec<(&Node, &Row)> = live.iter().copied().filter(|(_, r)| !e.terminal.contains(&r.state)).collect();
            if !open.is_empty() {
                live = open;
            }
        }
        Some(live[rng.below(live.len())].0)
    }

    /// True when every identity input of `cmd` names a row the walk holds.
    pub fn names_live_rows(&self, cmd: usize, input: &Input) -> bool {
        self.cat.commands[cmd].id_ents.iter().all(|(field, ent)| match input.get(field) {
            None | Some(Node::Null) => self.cat.commands[cmd].create_inputs.contains(field) || !self.cat.commands[cmd].id_inputs.iter().any(|(f, _)| f == field),
            Some(v) => self.cat.commands[cmd].create_inputs.contains(field) || self.rows.contains_key(&(*ent, v.clone())),
        })
    }

    fn hit_state(&self, ent: usize, state: usize) {
        self.cov.hit_state(self.cat.entities[ent].state_base + state);
    }

    fn apply_subject(&mut self, cmd: usize, oi: usize, input: &Input, events: &[ObservedEvent]) {
        let out = &self.cat.commands[cmd].outcomes[oi];
        let Some(subj) = out.subject.clone() else { return };
        let ent = &self.cat.entities[subj.ent];
        let id = match &subj.instance {
            InstanceSrc::Input(field) => input.get(field).cloned(),
            InstanceSrc::Event(event, field) => events
                .iter()
                .find(|e| e.event.name() == event)
                .and_then(|e| e.payload.get(field).cloned())
                .or_else(|| input.get(field).cloned()),
        };
        let Some(id) = id else { return };
        let key = (subj.ent, id);
        match subj.effect {
            Eff::Creates { into } => {
                let mut fields = BTreeMap::new();
                if self.guided {
                    // The stored fields the request and the events name, where the names agree: an
                    // approximation of the row the interpreter holds, for guard ties.
                    for f in &ent.fields {
                        if let Some(v) = input.get(f).filter(|v| !matches!(v, Node::Null)) {
                            fields.insert(f.clone(), v.clone());
                        }
                    }
                    for e in events {
                        if e.payload.get(&ent.identity_field) != Some(&key.1) {
                            continue;
                        }
                        for f in &ent.fields {
                            if let Some(v) = e.payload.get(f).filter(|v| !matches!(v, Node::Null)) {
                                fields.entry(f.clone()).or_insert_with(|| v.clone());
                            }
                        }
                    }
                }
                self.rows.insert(key.clone(), Row { state: into, fields, guessed: BTreeSet::new() });
                self.hit_state(subj.ent, into);
            }
            Eff::Moves { tr, to } => {
                self.cov.hit_transition(ent.tr_base + tr);
                self.rows
                    .entry(key.clone())
                    .or_insert(Row { state: to, fields: BTreeMap::new(), guessed: BTreeSet::new() })
                    .state = to;
                self.hit_state(subj.ent, to);
            }
            Eff::Updates | Eff::Preserves => {
                if let Some(row) = self.rows.get(&key) {
                    let s = row.state;
                    self.hit_state(subj.ent, s);
                }
            }
            Eff::Deletes => {
                self.rows.remove(&key);
                return;
            }
        }
        if let Some(row) = self.rows.get_mut(&key) {
            for (field, src) in &subj.sets {
                if matches!(src, SetSrc::Literal(_) | SetSrc::Increment(_) | SetSrc::Other) {
                    row.guessed.insert(field.clone());
                } else {
                    row.guessed.remove(field);
                }
                match src {
                    SetSrc::Input(name) => {
                        if let Some(v) = input.get(name) {
                            row.fields.insert(field.clone(), v.clone());
                        }
                    }
                    SetSrc::Literal(v) => {
                        row.fields.insert(field.clone(), Node::Text(v.clone()));
                    }
                    SetSrc::Increment(_) => {
                        let next = match row.fields.get(field) {
                            Some(Node::Number(n)) => n.to_string().parse::<i64>().unwrap_or(1) + 1,
                            _ => 1,
                        };
                        row.fields.insert(field.clone(), Node::Number(next.into()));
                    }
                    SetSrc::Cleared => {
                        row.fields.remove(field);
                    }
                    SetSrc::Other => {
                        // A generated or derived value: an event about this row that carries the
                        // field shows it; otherwise a stand-in.
                        let shown = events
                            .iter()
                            .filter(|e| e.payload.get(&ent.identity_field) == Some(&key.1))
                            .find_map(|e| e.payload.get(field).filter(|v| !matches!(v, Node::Null)));
                        match shown {
                            Some(v) if self.guided => {
                                row.fields.insert(field.clone(), v.clone());
                                row.guessed.remove(field);
                            }
                            _ => {
                                row.fields.insert(field.clone(), Node::Bool(true));
                            }
                        }
                    }
                }
            }
        }
        for (e, t) in &self.cat.commands[cmd].outcomes[oi].set_moves {
            self.cov.hit_transition(self.cat.entities[*e].tr_base + t);
        }
    }

    fn harvest_events(&mut self, events: &[ObservedEvent]) {
        let mut items = Vec::new();
        for event in events {
            if let Some(declared) = self.cat.ir.events().get(event.event.name()) {
                for f in &declared.fields {
                    if let Some(v) = event.payload.get(&f.name) {
                        gen::harvest(&self.cat.ir, &f.type_ref, v, &mut items);
                    }
                }
            }
        }
        self.remember(items);
    }

    /// Sends one command and keeps what its answer shows.
    pub fn send(&mut self, req: &Request) -> Answer {
        let started = std::time::Instant::now();
        self.step += 1;
        self.advance_clock(self.step_seconds);
        let cat = self.cat;
        let info = &cat.commands[req.cmd];
        let command = CommandRef::new(info.name.clone());
        if let Some(force) = req.force {
            let name = cat.ir.commands()[&info.name].outcomes[force].name.clone();
            let _ = self.target.configure_external_outcome(ExternalOutcomeControl {
                force: OutcomeRef::new(command.clone(), name),
                correlation: self.correlation.clone(),
            });
        }
        let request = SemanticCommandRequest {
            command: command.clone(),
            actor: req.actor.map(|a| ActorRef::new(cat.actors[a].name.clone())),
            caller: req.caller.clone(),
            input: req.input.clone(),
            correlation: self.correlation.clone(),
        };
        let answer = self.target.execute_command(request);
        let mut result = Answer {
            outcome: None,
            label: String::new(),
            events: Vec::new(),
            response: None,
            error: None,
            bound: Vec::new(),
            bound_events: Vec::new(),
        };
        match answer {
            Ok(done) => {
                let oi = done.outcome.as_ref().and_then(|o| {
                    info.outcomes.iter().position(|x| x.name == o.outcome.to_string())
                });
                if let Some(oi) = oi {
                    let gid = info.outcomes[oi].gid;
                    result.outcome = Some(gid);
                    result.label = info.outcomes[oi].name.clone();
                    if self.cov.hit_outcome(gid) {
                        self.findings.first.push((gid, self.step));
                        self.findings.interesting = true;
                    }
                    self.apply_subject(req.cmd, oi, &req.input, &done.direct_events);
                    if !info.outcomes[oi].error {
                        let mut items = Vec::new();
                        for f in &info.input {
                            if let Some(v) = req.input.get(&f.name) {
                                gen::harvest(&cat.ir, &f.type_ref, v, &mut items);
                            }
                        }
                        if let Some(resp) = &done.response {
                            for f in &info.response {
                                if let Some(v) = resp.get(&f.name) {
                                    gen::harvest(&cat.ir, &f.type_ref, v, &mut items);
                                }
                            }
                        }
                        self.remember(items);
                    }
                } else {
                    result.label = "undeclared".to_owned();
                    result.error = Some((
                        "undeclared".to_owned(),
                        "a refusal the model does not declare".to_owned(),
                    ));
                }
                for e in &done.direct_events {
                    if let Some(s) = e.sequence {
                        self.seen_sequence = self.seen_sequence.max(s);
                    }
                }
                self.harvest_events(&done.direct_events);
                result.events = done.direct_events;
                result.response = done.response;
                (result.bound, result.bound_events) = self.follow_bindings(&result.events);
            }
            Err(error) => {
                let (kind, why) = match &error {
                    TargetError::Unsupported { why, .. } => ("unsupported", why.clone()),
                    TargetError::Unavailable { detail, .. } => {
                        if detail.contains("violating its invariant") {
                            ("invariant", detail.clone())
                        } else {
                            ("unavailable", detail.clone())
                        }
                    }
                    TargetError::NotGranted { actor } => (
                        "not_granted",
                        format!("not granted to {}", actor.clone().unwrap_or_default()),
                    ),
                };
                result.label = kind.to_owned();
                result.error = Some((kind.to_owned(), why));
                // A binding the command's events triggered may have failed after the command
                // committed: the events of commands bindings ran are still read.
                (result.bound, result.bound_events) = self.follow_bindings(&[]);
            }
        }
        if let Some((kind, why)) = &result.error {
            let key = (req.cmd, kind.clone(), normalize(why));
            let entry = self.findings.errors.entry(key).or_insert((0, String::new()));
            entry.0 += 1;
            if entry.1.is_empty() {
                entry.1 = why.chars().take(800).collect();
            }
            if kind == "invariant" {
                self.findings.violations.push(crate::coverage::Violation {
                    kind: "entity-invariant".to_owned(),
                    detail: why.chars().take(800).collect(),
                    command: info.name.to_string(),
                    walk: self.walk,
                    seed: self.seed,
                    step: self.step,
                });
            }
        }
        self.cov.steps.fetch_add(1, Ordering::Relaxed);
        self.cov.command_sends[req.cmd].fetch_add(1, Ordering::Relaxed);
        self.cov.command_nanos[req.cmd].fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
        result
    }

    /// Reads what the bindings did after a command: the invocations they made, and the events
    /// the commands they ran emitted; applies the outcome those events identify.
    fn follow_bindings(&mut self, direct: &[ObservedEvent]) -> (Vec<usize>, Vec<ObservedEvent>) {
        let cat = self.cat;
        let mut triggered = false;
        for e in direct {
            if let Some(list) = cat.bindings_by_event.get(e.event.name()) {
                for b in list {
                    triggered = true;
                    let request = InvocationObservationRequest {
                        binding: BindingRef::new(b.name.clone()),
                        command: CommandRef::new(cat.commands[b.command].name.clone()),
                        correlation: self.correlation.clone(),
                        deadline: Deadline::at(Timestamp::EPOCH),
                    };
                    if let Ok(list) = self.target.observe_invocations(request) {
                        let seen = self.invocations.entry(b.name.to_string()).or_insert(0);
                        if list.len() > *seen {
                            let fresh = (list.len() - *seen) as u64;
                            self.cov.binding_invocations[b.command].fetch_add(fresh, Ordering::Relaxed);
                            *seen = list.len();
                            // A bounded retry that used its whole bound: the spec would retry on.
                            let name = b.name.to_string();
                            if cat.retry_bound > 0 && fresh >= u64::from(cat.retry_bound) && cat.retry_bounded.contains(&name) {
                                let why = format!(
                                    "binding {name} retried {} {fresh} times after one event: the spec retries without bound (weft-sim bounds it at {})",
                                    cat.commands[b.command].name, cat.retry_bound
                                );
                                let entry = self.findings.errors.entry((b.command, "unbounded-retry".to_owned(), why.clone())).or_insert((0, why));
                                entry.0 += 1;
                            }
                        }
                    }
                }
            }
        }
        if !triggered {
            return (Vec::new(), Vec::new());
        }
        let mut fresh: Vec<ObservedEvent> = Vec::new();
        let emits = self.bound_emits.clone();
        for event in emits.keys() {
            let request = EventObservationRequest {
                event: EventRef::new(event.clone()),
                correlation: self.correlation.clone(),
                deadline: Deadline::at(Timestamp::EPOCH),
            };
            if let Ok(list) = self.target.observe_events(request) {
                for e in list {
                    if e.sequence.is_some_and(|s| s > self.seen_sequence) {
                        fresh.push(e);
                    }
                }
            }
        }
        fresh.sort_by_key(|e| e.sequence);
        if let Some(last) = fresh.last().and_then(|e| e.sequence) {
            self.seen_sequence = last;
        }
        self.harvest_events(&fresh);
        let mut bound = Vec::new();
        for e in &fresh {
            let Some(candidates) = emits.get(e.event.name()) else { continue };
            // The outcome is identified when one candidate emits it.
            if candidates.len() != 1 {
                continue;
            }
            let (c, oi) = candidates[0];
            let gid = cat.commands[c].outcomes[oi].gid;
            if self.cov.hit_outcome(gid) {
                self.findings.first.push((gid, self.step));
                self.findings.interesting = true;
            }
            bound.push(gid);
            let payload = e.payload.clone();
            self.apply_subject(c, oi, &payload, std::slice::from_ref(e));
        }
        (bound, fresh)
    }

    /// Reads a view as no actor, every row.
    pub fn view(&self, name: &str, params: BTreeMap<String, Node>) -> Result<Vec<BTreeMap<String, Node>>, String> {
        let view = QualifiedName::new(name).map_err(|e| e.to_string())?;
        let request = SemanticViewRequest {
            view: ess_conformance::scenario::ViewRef::new(view),
            params,
            consistency: Default::default(),
            correlation: self.correlation.clone(),
            deadline: Deadline::at(Timestamp::EPOCH),
        };
        self.target
            .query_view(request)
            .map(|r| r.rows)
            .map_err(|e| e.to_string())
    }

    pub fn trace_entry(&self, req: &Request, answer: &Answer) -> Value {
        json!({
            "command": self.cat.commands[req.cmd].name.to_string(),
            "actor": req.actor.map(|a| self.cat.actors[a].name.to_string()),
            "caller": req.caller,
            "input": req.input,
            "force": req.force.map(|f| self.cat.commands[req.cmd].outcomes[f].name.clone()),
            "answer": answer.label,
        })
    }
}
