//! `weft-sim explore`: random walks over the whole model, weighted toward outcomes not reached
//! yet, on T threads.
//!
//! Walks run in epochs of [`EPOCH`] walks. A walk reads the run's coverage as it stood when its
//! epoch began, so what one walk does depends on its seed and the epochs before it, never on how
//! threads interleave: the same `--seed --walks --steps` gives the same walks on any `--threads`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Barrier, RwLock};
use std::time::{Duration, Instant};

use ess_primitives::node::Node;

use crate::catalog::Catalog;
use crate::coverage::{Coverage, FeedLine, Violation};
use crate::experiment::Experiment;
use crate::gen::{self, Candidates, Input, FRESH_BASE};
use crate::rng::Rng;
use crate::simulate::{fingerprint, idempotency_check, replay_setup, system_checks, Recorded, Ticker, ALL_INVARIANTS};
use crate::walk::{bound_emits, Answer, Request, Session};

pub const EPOCH: u64 = 32;

pub struct Config {
    pub seed: u64,
    pub walks: u64,
    pub steps: usize,
    pub threads: usize,
    pub max_seconds: u64,
    /// `--from`: the setup every walk replays before its first step (requests with the answers
    /// recorded when it ran once, and its settle ticks with `--settle`).
    pub setup: Option<Vec<Recorded>>,
    /// `--controller-share`: the experiment's controllers, ticked inside walks; with
    /// `checks.converge` also after each walk.
    pub controllers: Option<WalkControllers>,
    pub checks: Checks,
}

impl Default for Config {
    fn default() -> Self {
        Self { seed: 1, walks: 64, steps: 60, threads: 4, max_seconds: 280, setup: None, controllers: None, checks: Checks::default() }
    }
}

/// The system checks of a walk.
#[derive(Clone)]
pub struct Checks {
    /// `--invariants`: the system invariants checked (of `simulate::ALL_INVARIANTS`).
    pub invariants: BTreeSet<String>,
    /// `--check-every N`: the view invariants after every N-th step of a walk (a controller tick
    /// counts as one), and an accepted step of a redelivered command sent again on every N-th step
    /// (`idempotency`); 0 checks nothing.
    pub every: usize,
    /// `--settle N`: after each walk, up to N ticks of the experiment's controllers with no new
    /// input; a walk whose controllers still have work is a `no-convergence` violation. 0: none.
    pub converge: usize,
}

impl Default for Checks {
    fn default() -> Self {
        Self { invariants: ALL_INVARIANTS.iter().map(|s| (*s).to_owned()).collect(), every: 1, converge: 0 }
    }
}

/// What a walk's checks found so far: each invariant (and for `idempotency` each command) is
/// reported once per walk, at the first step it fails.
#[derive(Default)]
struct WalkChecks {
    reported: BTreeSet<String>,
    /// Steps of the walk (a controller tick counts as one).
    steps: usize,
    nanos: u64,
}

/// The controllers a walk ticks: those of `exp`, with the variables its setup bound; a step is a
/// tick instead of a random command with probability `share`.
pub struct WalkControllers {
    pub exp: Experiment,
    pub vars: BTreeMap<String, serde_json::Value>,
    pub share: f64,
}

/// What replaying the setup cost a run, and whether it answered as recorded; and the cost of the
/// controller ticks inside walks.
#[derive(Default)]
pub struct Replays {
    pub walks: AtomicU64,
    pub nanos: AtomicU64,
    /// Walks whose replay answered one step differently from the recorded setup.
    pub differ: AtomicU64,
    pub ticks: AtomicU64,
    pub tick_sends: AtomicU64,
    pub tick_nanos: AtomicU64,
    /// Walk steps of their own (random commands), to compare with the ticks' sends.
    pub walk_steps: AtomicU64,
    pub walk_nanos: AtomicU64,
}

/// A fresh value of a declared type: names no row a walk holds.
pub fn fresh(cat: &Catalog, t: &ess_compiler::ir::ResolvedTypeRef, rng: &mut Rng) -> Option<Node> {
    gen::mint(&cat.ir, t, FRESH_BASE + rng.below(400_000_000_000) as u64)
}

/// Caller attributes for `actor`, drawn from what the walk holds.
pub fn caller_for(sess: &Session, actor: usize, input: Option<&Input>, rng: &mut Rng) -> Option<BTreeMap<String, Node>> {
    let cat = sess.cat;
    let attributes = &cat.actors[actor].attributes;
    if attributes.is_empty() {
        return None;
    }
    let mut values = BTreeMap::new();
    for a in attributes {
        let chosen: Vec<Node> = input.map(|i| i.values().cloned().collect()).unwrap_or_default();
        let value = input
            .and_then(|i| i.get(&a.name).cloned())
            .or_else(|| a.type_ref.declared().and_then(|t| sess.pool_pick_linked(&t.name().to_string(), &chosen, rng)))
            .or_else(|| fresh(cat, &a.type_ref, rng))
            .or_else(|| gen::mint(&cat.ir, &a.type_ref, 1));
        if let Some(v) = value {
            values.insert(a.name.clone(), v);
        }
    }
    Some(values)
}

/// An input for `cmd`, directed at outcome `target` where given.
pub fn input_for(
    sess: &Session,
    cands: &Candidates,
    cmd: usize,
    target: Option<usize>,
    rng: &mut Rng,
) -> Input {
    let cat = sess.cat;
    let info = &cat.commands[cmd];
    let directed = target.filter(|t| info.outcomes[*t].guard.is_some());
    let mut list = cands.get(cat, cmd, directed);
    if list.is_empty() && directed.is_some() {
        list = cands.get(cat, cmd, None);
    }
    let mut input: Input = if list.is_empty() {
        let mut minted = Input::new();
        for f in &info.input {
            if f.type_ref.is_optional() && rng.chance(0.5) {
                continue;
            }
            if let Some(v) = gen::mint(&cat.ir, &f.type_ref, 1 + rng.below(4) as u64) {
                minted.insert(f.name.clone(), v);
            }
        }
        minted
    } else {
        let pick = if directed.is_some() { rng.below(list.len().min(3)) } else { rng.below(list.len()) };
        list[pick].clone()
    };
    let kind = target.map(|t| info.outcomes[t].kind).unwrap_or("");
    let mut chosen: Vec<Node> = Vec::new();
    for f in &info.input {
        let list = matches!(f.type_ref.required(), ess_compiler::ir::ResolvedTypeRef::List { .. });
        let t = match f.type_ref.required() {
            ess_compiler::ir::ResolvedTypeRef::Declared { name } => name,
            ess_compiler::ir::ResolvedTypeRef::List { of } => match of.required() {
                ess_compiler::ir::ResolvedTypeRef::Declared { name } => name,
                _ => continue,
            },
            _ => continue,
        };
        let tname = t.name().to_string();
        if f.type_ref.is_optional() && !input.contains_key(&f.name) && rng.chance(0.6) {
            continue;
        }
        let is_identity = cat.identity_types.contains_key(t.name());
        let ent = cat.identity_types.get(t.name()).copied();
        let live = |rng: &mut Rng| -> Option<Node> {
            if sess.guided && rng.chance(0.8) {
                sess.live_pick(ent?, &chosen, &[], rng).cloned()
            } else {
                None
            }
        };
        let value = if info.create_inputs.contains(&f.name) {
            if kind == "existing_instance" {
                live(rng).or_else(|| sess.pool_pick(&tname, rng))
            } else {
                fresh(cat, &f.type_ref, rng)
            }
        } else if kind == "unknown_instance" && is_identity {
            fresh(cat, &f.type_ref, rng)
        } else if let Some(v) = live(rng).filter(|_| !list) {
            Some(v)
        } else if rng.chance(0.92) {
            sess.pool_pick_linked(&tname, &chosen, rng)
        } else {
            None
        };
        if let Some(v) = value {
            let v = match (list, v) {
                (true, Node::Seq(items)) => Node::Seq(items),
                (true, scalar) => Node::Seq(vec![scalar]),
                (false, v) => v,
            };
            if is_identity {
                chosen.push(v.clone());
            }
            input.insert(f.name.clone(), v);
        }
    }
    if sess.guided {
        guide(sess, cmd, &mut input, rng);
    }
    input
}

/// True when `t` is a Timestamp, through newtypes and Optional.
pub fn is_timestamp(cat: &Catalog, t: &ess_compiler::ir::ResolvedTypeRef) -> bool {
    use ess_compiler::ir::{ResolvedBody, ResolvedTypeRef};
    let mut current = t.required().clone();
    for _ in 0..40 {
        match current {
            ResolvedTypeRef::Primitive { name } => return name == ess_domain::types::Primitive::Timestamp,
            ResolvedTypeRef::Declared { ref name } => match &cat.ir.named_type(name).body {
                ResolvedBody::Newtype { of, .. } => current = of.required().clone(),
                _ => return false,
            },
            _ => return false,
        }
    }
    false
}

/// An instant near the walk's clock: before `now` or after it, by 1 s to 1 h.
pub fn near_now(sess: &Session, rng: &mut Rng) -> Node {
    let now = sess.clock.0.load(std::sync::atomic::Ordering::Relaxed) as i64;
    let spans = [1i64, 30, 300, 3600];
    let span = spans[rng.below(spans.len())];
    let offset = if rng.chance(0.5) { span } else { -span };
    let seconds = (now + offset).max(0) as u64;
    Node::Text(ess_primitives::time::Timestamp::from_epoch_millis(seconds * 1000).iso_8601())
}

/// Explore's guidance on top of a drawn input: Timestamps around the clock, rows a guard requires
/// to exist, and fields a guard ties to the row an input names, copied from that row. Each with a
/// random share left out, so refusals are still hit.
fn guide(sess: &Session, cmd: usize, input: &mut Input, rng: &mut Rng) {
    let cat = sess.cat;
    let info = &cat.commands[cmd];
    for f in &info.input {
        if is_timestamp(cat, &f.type_ref) && rng.chance(0.6) && (input.contains_key(&f.name) || !f.type_ref.is_optional()) {
            input.insert(f.name.clone(), near_now(sess, rng));
        }
    }
    let scalar = |name: &str| {
        info.input.iter().any(|f| f.name == name && !matches!(f.type_ref.required(), ess_compiler::ir::ResolvedTypeRef::List { .. }))
    };
    for st in &info.set_ties {
        if !rng.chance(0.5) {
            continue;
        }
        let chosen: Vec<Node> = st.pairs.iter().filter_map(|(_, i)| input.get(i).cloned()).collect();
        let Some(id) = sess.live_pick(st.ent, &chosen, &st.states, rng) else { continue };
        let Some(row) = sess.rows.get(&(st.ent, id.clone())) else { continue };
        let ident = &cat.entities[st.ent].identity_field;
        for (row_field, input_field) in &st.pairs {
            let value = if row_field == ident { Some(id.clone()) } else { row.known(row_field).cloned() };
            if let Some(v) = value.filter(|_| scalar(input_field)) {
                input.insert(input_field.clone(), v);
            }
        }
    }
    for tie in &info.ties {
        if !rng.chance(0.85) {
            continue;
        }
        let Some(id) = input.get(&tie.via) else { continue };
        let Some(row) = sess.rows.get(&(tie.ent, id.clone())) else { continue };
        let value = if tie.row_field == cat.entities[tie.ent].identity_field {
            Some(id.clone())
        } else {
            row.known(&tie.row_field).cloned()
        };
        if let Some(v) = value.filter(|_| scalar(&tie.input_field)) {
            input.insert(tie.input_field.clone(), v);
        }
    }
}

/// The next request of a walk: a command the actors may send, weighted toward outcomes `prior`
/// has not reached and toward commands whose row inputs the walk can fill.
pub fn choose(
    sess: &Session,
    cands: &Candidates,
    prior: &[u64],
    sent: &[u32],
    rng: &mut Rng,
) -> Option<Request> {
    let cat = sess.cat;
    let weights: Vec<f64> = cat
        .commands
        .iter()
        .enumerate()
        .map(|(c, info)| {
            if info.outcomes.is_empty() {
                return 0.0;
            }
            let unreached = info.outcomes.iter().filter(|o| prior[o.gid] == 0).count();
            let mut w = 1.0 + 6.0 * unreached as f64 / info.outcomes.len() as f64;
            let missing = info.id_inputs.iter().filter(|(_, t)| !sess.pool.contains_key(t)).count();
            if missing > 0 {
                w *= 0.03;
            } else if info.creates {
                let need: usize = info
                    .creates_types
                    .iter()
                    .filter(|t| !sess.pool.contains_key(*t))
                    .map(|t| cat.demand.get(t).copied().unwrap_or(0).min(20))
                    .sum();
                w *= 1.5 + need as f64 / 2.0;
            }
            if sent[c] == 0 {
                w *= 1.5;
            } else {
                w /= 1.0 + f64::from(sent[c]) * 0.5;
            }
            w
        })
        .collect();
    let cmd = rng.weighted(&weights)?;
    let info = &cat.commands[cmd];
    let unreached: Vec<usize> = info
        .outcomes
        .iter()
        .enumerate()
        .filter(|(_, o)| prior[o.gid] == 0)
        .map(|(i, _)| i)
        .collect();
    let target = if !unreached.is_empty() && rng.chance(0.7) {
        Some(unreached[rng.below(unreached.len())])
    } else if rng.chance(0.15) {
        Some(rng.below(info.outcomes.len()))
    } else {
        None
    };
    let actor = if info.actors.is_empty() {
        None
    } else {
        Some(info.actors[rng.below(info.actors.len())])
    };
    let input = input_for(sess, cands, cmd, target, rng);
    let caller = actor.and_then(|a| caller_for(sess, a, Some(&input), rng));
    let force = target.filter(|t| info.outcomes[*t].external);
    Some(Request {
        cmd,
        actor,
        caller,
        input,
        force,
    })
}

pub fn feed_line(sess: &Session, req: &Request, answer: &Answer) -> FeedLine {
    let cat = sess.cat;
    FeedLine {
        walk: sess.walk,
        step: sess.step,
        command: cat.commands[req.cmd].name.to_string(),
        actor: req
            .actor
            .map(|a| cat.actors[a].name.to_string().rsplit('.').next().unwrap_or("").to_owned())
            .unwrap_or_default(),
        outcome: answer.label.clone(),
        events: answer
            .events
            .iter()
            .map(|e| e.event.name().to_string().rsplit('.').next().unwrap_or("").to_owned())
            .collect(),
        note: if answer.bound.is_empty() {
            String::new()
        } else {
            let names: Vec<String> = answer
                .bound
                .iter()
                .map(|g| {
                    let (c, o) = cat.outcome_of[*g];
                    let command = cat.commands[c].name.to_string();
                    format!("{}/{}", command.rsplit('.').next().unwrap_or(""), cat.commands[c].outcomes[o].name)
                })
                .collect();
            format!("+{} by bindings: {}", answer.bound.len(), names.join(" "))
        },
    }
}

/// The setup a walk replays first: its steps, the coverage its answers count in (not the run's:
/// the setup's own run counted them once), and the cost.
pub struct SetupStart<'a> {
    pub steps: &'a [Recorded],
    pub scratch: &'a Coverage,
    pub replays: &'a Replays,
}

/// Counts one request of a walk in its findings: attempts, the branch point it left, its trace
/// entry (with `at`, the clock before the send, and `by` for a controller's), the feed. `pre` is
/// the abstract state before it; the state after is returned.
#[allow(clippy::too_many_arguments)]
fn record_step(sess: &mut Session, req: &Request, answer: &Answer, live: bool, at: u64, by: Option<&str>, pre: u64, feed: bool) -> u64 {
    let tally = sess.findings.attempts.entry((req.cmd, answer.label.clone())).or_default();
    tally[0] += 1;
    if live {
        tally[1] += 1;
    }
    if let Some(gid) = answer.outcome {
        let key = (req.cmd as u32, pre);
        if !sess.findings.branch.contains_key(&key) {
            let (_, desc) = sess.abstract_state(true);
            sess.findings.branch.insert(key, (Default::default(), desc));
        }
        if let Some(entry) = sess.findings.branch.get_mut(&key) {
            entry.0.insert(gid as u32);
        }
    }
    let (post, _) = sess.abstract_state(false);
    sess.findings.abstract_states.insert(post);
    let mut entry = sess.trace_entry(req, answer);
    entry["at"] = serde_json::json!(at);
    if let Some(by) = by {
        entry["by"] = serde_json::json!(by);
    }
    sess.findings.trace.push(entry);
    if feed {
        sess.cov.feed(feed_line(sess, req, answer));
    }
    post
}

/// Runs the view invariants on the walk's store; one that fails for the first time in this walk
/// is a violation at the walk's current step, named after `command` (the last one sent).
fn check_views(sess: &mut Session, checks: &Checks, seen: &mut WalkChecks, command: &str) {
    let started = Instant::now();
    for (check, _) in system_checks(sess, &checks.invariants) {
        if !check.ok && seen.reported.insert(check.name.clone()) {
            let v = Violation { kind: check.name, detail: check.detail.chars().take(600).collect(), command: command.to_owned(), walk: sess.walk, seed: sess.seed, step: sess.step };
            sess.findings.violations.push(v);
        }
    }
    seen.nanos += started.elapsed().as_nanos() as u64;
}

/// The checks after a step of the walk's own: on every `checks.every`-th step, an accepted step of
/// a command an `at_least_once` binding invokes is sent again (it must change no view), then the
/// view invariants. The duplicate goes in the trace with `by: duplicate`.
#[allow(clippy::too_many_arguments)]
fn check_step(sess: &mut Session, checks: &Checks, seen: &mut WalkChecks, req: &Request, answer: &Answer, live: bool, pre: &mut u64, feed: bool, sent: &mut [u32]) -> bool {
    seen.steps += 1;
    if checks.every == 0 || !seen.steps.is_multiple_of(checks.every) {
        return false;
    }
    let cat = sess.cat;
    let name = cat.commands[req.cmd].name.to_string();
    let duplicate = checks.invariants.contains("idempotency") && cat.redelivered.contains(&req.cmd) && crate::simulate::accepted(cat, answer);
    if duplicate {
        let started = Instant::now();
        let before = fingerprint(sess);
        let at = sess.clock.0.load(Ordering::Relaxed);
        let again = sess.send(req);
        sent[req.cmd] += 1;
        let check = idempotency_check(&before, &fingerprint(sess), &again);
        *pre = record_step(sess, req, &again, live, at, Some("duplicate"), *pre, feed);
        if !check.ok && seen.reported.insert(format!("idempotency {name}")) {
            let v = Violation { kind: "idempotency".into(), detail: check.detail.chars().take(600).collect(), command: name.clone(), walk: sess.walk, seed: sess.seed, step: sess.step };
            sess.findings.violations.push(v);
        }
        seen.nanos += started.elapsed().as_nanos() as u64;
    }
    check_views(sess, checks, seen, &name);
    duplicate
}

/// Adds a walk's check counts and seconds to the run's `info.checks`.
fn count_checks(sess: &Session, seen: &WalkChecks, duplicates: u64, converge: Option<(u64, u64, u64)>) {
    let mut inner = sess.cov.inner.lock().expect("coverage");
    let entry = inner.info.entry("checks".into()).or_insert_with(|| serde_json::json!({}));
    let add = |v: &mut serde_json::Value, k: &str, n: f64| v[k] = serde_json::json!(v[k].as_f64().unwrap_or(0.0) + n);
    add(entry, "seconds", seen.nanos as f64 / 1e9);
    add(entry, "duplicates", duplicates as f64);
    if let Some((ticks, sends, nanos)) = converge {
        add(entry, "converge_walks", 1.0);
        add(entry, "converge_ticks", ticks as f64);
        add(entry, "converge_sends", sends as f64);
        add(entry, "converge_seconds", nanos as f64 / 1e9);
    }
}

/// One walk: `steps` requests from a fresh scenario, or from the replayed setup; with a ticker,
/// a share of the steps are a tick of the experiment's controllers. The system invariants are
/// checked as `checks` says; with `checks.converge` the walk ends with the convergence check.
#[allow(clippy::too_many_arguments)]
pub fn run_walk<'a>(
    sess: &mut Session<'a>,
    cands: &Candidates,
    prior: &[u64],
    walk: u64,
    seed: u64,
    steps: usize,
    feed: bool,
    pattern: Option<&crate::experiment::Pattern>,
    from: Option<&SetupStart<'a>>,
    mut ticker: Option<&mut Ticker<'a, '_>>,
    checks: &Checks,
) {
    let mut seen = WalkChecks::default();
    let mut duplicates = 0u64;
    let cat = sess.cat;
    if let Err(e) = sess.begin(walk, seed) {
        let mut inner = sess.cov.inner.lock().expect("coverage");
        if inner.notes.len() < 20 {
            inner.notes.push(format!("walk {walk}: begin_scenario refused: {e}"));
        }
        return;
    }
    if let Some(from) = from {
        let started = Instant::now();
        let run_cov = sess.cov;
        sess.cov = from.scratch;
        let differ = replay_setup(sess, from.steps) > 0;
        sess.cov = run_cov;
        sess.step_seconds = 1;
        sess.step = 0;
        sess.findings = Default::default();
        from.replays.walks.fetch_add(1, Ordering::Relaxed);
        from.replays.nanos.fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
        if differ {
            from.replays.differ.fetch_add(1, Ordering::Relaxed);
        }
    }
    let mut rng = Rng::new(seed);
    let mut sent = vec![0u32; cat.commands.len()];
    let (mut pre, _) = sess.abstract_state(false);
    sess.findings.abstract_states.insert(pre);
    if let Some(t) = ticker.as_deref_mut() {
        t.begin(seed);
    }
    let walk_started = Instant::now();
    let mut tick_nanos = 0u64;
    let mut own_steps = 0u64;
    for _ in 0..steps {
        if rng.chance(0.05) {
            sess.advance_clock(3600);
        }
        // A controller tick in place of a random command (the draw only with a ticker, so a run
        // without one walks as before).
        if let Some(t) = ticker.as_deref_mut().filter(|t| t.share > 0.0) {
            if rng.chance(t.share) {
                let started = Instant::now();
                let (sends, error) = t.tick(sess);
                if let Some(e) = error {
                    let mut inner = sess.cov.inner.lock().expect("coverage");
                    if inner.notes.len() < 20 {
                        inner.notes.push(format!("walk {walk}: controller tick: {e}"));
                    }
                }
                if let Some(r) = from.map(|f| f.replays) {
                    r.ticks.fetch_add(1, Ordering::Relaxed);
                    r.tick_sends.fetch_add(sends.len() as u64, Ordering::Relaxed);
                }
                for (req, answer, at) in &sends {
                    sent[req.cmd] += 1;
                    let live = sess.names_live_rows(req.cmd, &req.input);
                    pre = record_step(sess, req, answer, live, *at, Some("controller"), pre, feed);
                }
                tick_nanos += started.elapsed().as_nanos() as u64;
                // A tick counts as one step for the checks; a duplicate is of a walk's own step.
                seen.steps += 1;
                if checks.every > 0 && seen.steps.is_multiple_of(checks.every) {
                    check_views(sess, checks, &mut seen, "(controller tick)");
                }
                continue;
            }
        }
        let Some(req) = choose(sess, cands, prior, &sent, &mut rng) else { break };
        let live = sess.names_live_rows(req.cmd, &req.input);
        let at = sess.clock.0.load(Ordering::Relaxed);
        let answer = sess.send(&req);
        sent[req.cmd] += 1;
        own_steps += 1;
        pre = record_step(sess, &req, &answer, live, at, None, pre, feed);
        if check_step(sess, checks, &mut seen, &req, &answer, live, &mut pre, feed, &mut sent) {
            duplicates += 1;
        }
        if let Some(p) = pattern {
            if let Some(found) = p.matches(sess) {
                let mut inner = sess.cov.inner.lock().expect("coverage");
                inner.matches.push(serde_json::json!({
                    "pattern": p.name, "walk": walk, "seed": seed, "step": sess.step, "rows": found,
                }));
                drop(inner);
                sess.findings.interesting = true;
                break;
            }
        }
    }
    if let Some(r) = from.map(|f| f.replays) {
        r.tick_nanos.fetch_add(tick_nanos, Ordering::Relaxed);
        r.walk_steps.fetch_add(own_steps, Ordering::Relaxed);
        let total = walk_started.elapsed().as_nanos() as u64;
        r.walk_nanos.fetch_add(total.saturating_sub(tick_nanos), Ordering::Relaxed);
    }
    // Convergence: the controllers with no new input, until a tick without work.
    let mut converge = None;
    if let Some(t) = ticker.filter(|_| checks.converge > 0) {
        let started = Instant::now();
        let settle = t.settle(sess, checks.converge);
        for (req, answer, at) in &settle.sends {
            let live = sess.names_live_rows(req.cmd, &req.input);
            pre = record_step(sess, req, answer, live, *at, Some("converge"), pre, feed);
        }
        match (&settle.error, settle.settled_at) {
            (Some(e), _) => {
                let mut inner = sess.cov.inner.lock().expect("coverage");
                if inner.notes.len() < 20 {
                    inner.notes.push(format!("walk {walk}: convergence tick: {e}"));
                }
            }
            (None, None) => {
                let detail = format!("the controllers still had work after {} ticks with no new input: {}", settle.ticks, settle.busy);
                let v = Violation { kind: "no-convergence".into(), detail, command: String::new(), walk, seed, step: sess.step };
                sess.findings.violations.push(v);
            }
            (None, Some(_)) => {}
        }
        converge = Some((settle.ticks as u64, settle.sends.len() as u64, started.elapsed().as_nanos() as u64));
    }
    // The store the walk (and its convergence ticks) ends with.
    if checks.every > 0 {
        check_views(sess, checks, &mut seen, "(end of walk)");
    }
    count_checks(sess, &seen, duplicates, converge);
    let findings = std::mem::take(&mut sess.findings);
    sess.cov.merge(walk, seed, findings);
}

pub fn run(
    cat: &Catalog,
    cov: &Coverage,
    cands: &Candidates,
    config: &Config,
    pattern: Option<&crate::experiment::Pattern>,
) {
    let threads = config.threads.clamp(1, 8);
    let barrier = Barrier::new(threads);
    let prior: RwLock<Arc<Vec<u64>>> = RwLock::new(Arc::new(vec![0; cat.outcomes_total]));
    let next = AtomicU64::new(0);
    let stop = AtomicBool::new(false);
    let started = Instant::now();
    let emits = bound_emits(cat);
    let scratch = Coverage::new(cat, "setup-replay", 1);
    let replays = Replays::default();
    let from = config.setup.as_ref().map(|steps| SetupStart { steps, scratch: &scratch, replays: &replays });
    std::thread::scope(|scope| {
        for t in 0..threads {
            let barrier = &barrier;
            let prior = &prior;
            let next = &next;
            let stop = &stop;
            let from = from.as_ref();
            let emits = emits.clone();
            scope.spawn(move || {
                let mut sess = Session::new(cat, cov, emits);
                sess.guided = true;
                let mut ticker = config.controllers.as_ref().filter(|c| c.share > 0.0 || config.checks.converge > 0).and_then(|c| {
                    match Ticker::new(cat, cov, cands, &c.exp, &c.vars) {
                        Ok(mut t) => {
                            t.share = c.share;
                            Some(t)
                        }
                        Err(e) => {
                            cov.inner.lock().expect("coverage").notes.push(format!("controllers in walks: {e}"));
                            None
                        }
                    }
                });
                let mut epoch_start = 0;
                while epoch_start < config.walks {
                    let end = (epoch_start + EPOCH).min(config.walks);
                    barrier.wait();
                    if t == 0 {
                        *prior.write().expect("prior") = Arc::new(cov.outcome_snapshot());
                        next.store(epoch_start, Ordering::SeqCst);
                        if started.elapsed() > Duration::from_secs(config.max_seconds) {
                            stop.store(true, Ordering::SeqCst);
                        }
                    }
                    barrier.wait();
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let p = prior.read().expect("prior").clone();
                    loop {
                        let w = next.fetch_add(1, Ordering::SeqCst);
                        if w >= end {
                            break;
                        }
                        let seed = Rng::walk_seed(config.seed, w);
                        run_walk(&mut sess, cands, &p, w, seed, config.steps, t == 0, pattern, from, ticker.as_mut(), &config.checks);
                    }
                    epoch_start = end;
                }
            });
        }
    });
    if from.is_some() {
        let walks = replays.walks.load(Ordering::Relaxed);
        let seconds = replays.nanos.load(Ordering::Relaxed) as f64 / 1e9;
        let mut inner = cov.inner.lock().expect("coverage");
        let entry = inner.info.entry("setup_replay".into()).or_insert_with(|| serde_json::json!({"walks": 0, "seconds": 0.0, "walks_differ": 0}));
        let add = |v: &serde_json::Value, n: f64| serde_json::json!(v.as_f64().unwrap_or(0.0) + n);
        let load = |a: &AtomicU64| a.load(Ordering::Relaxed) as f64;
        entry["walks"] = add(&entry["walks"], walks as f64);
        entry["seconds"] = add(&entry["seconds"], seconds);
        entry["walks_differ"] = add(&entry["walks_differ"], load(&replays.differ));
        entry["steps"] = serde_json::json!(config.setup.as_ref().map_or(0, Vec::len));
        entry["walk_seconds"] = add(&entry["walk_seconds"], load(&replays.walk_nanos) / 1e9);
        entry["walk_steps"] = add(&entry["walk_steps"], load(&replays.walk_steps));
        entry["controller_ticks"] = add(&entry["controller_ticks"], load(&replays.ticks));
        entry["controller_sends"] = add(&entry["controller_sends"], load(&replays.tick_sends));
        entry["controller_seconds"] = add(&entry["controller_seconds"], load(&replays.tick_nanos) / 1e9);
    }
    if stop.load(Ordering::SeqCst) {
        let mut inner = cov.inner.lock().expect("coverage");
        inner.notes.push(format!(
            "stopped after {} s (--max-seconds) at {} walks",
            config.max_seconds,
            cov.walks.load(Ordering::Relaxed)
        ));
    }
}
