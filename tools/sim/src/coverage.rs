//! What a run has reached, shared by every walk thread and read by the live page and the report.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use serde_json::{json, Value};

use crate::catalog::Catalog;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Violation {
    pub kind: String,
    pub detail: String,
    pub command: String,
    pub walk: u64,
    pub seed: u64,
    pub step: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FeedLine {
    pub walk: u64,
    pub step: usize,
    pub command: String,
    pub actor: String,
    pub outcome: String,
    pub events: Vec<String>,
    pub note: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TimelineEntry {
    pub index: usize,
    pub phase: String,
    pub label: String,
    pub command: String,
    pub outcome: String,
    pub fault: Option<String>,
    pub checks: Vec<Check>,
}

#[derive(Debug, Clone)]
pub struct BranchEntry {
    pub outcomes: BTreeSet<u32>,
    pub state: String,
    pub seed: u64,
    pub walk: u64,
}

#[derive(Debug, Clone)]
pub struct ErrorEntry {
    pub count: u64,
    pub walk: u64,
    pub seed: u64,
    pub example: String,
}

/// One walk's findings, merged into the run when the walk ends.
#[derive(Default)]
pub struct WalkFindings {
    pub abstract_states: HashSet<u64>,
    pub branch: HashMap<(u32, u64), (BTreeSet<u32>, String)>,
    pub errors: HashMap<(usize, String, String), (u64, String)>,
    pub violations: Vec<Violation>,
    pub first: Vec<(usize, usize)>,
    pub trace: Vec<Value>,
    pub interesting: bool,
    /// (command, answer label) -> [requests, requests whose every identity input named a live row].
    pub attempts: HashMap<(usize, String), [u64; 2]>,
}

#[derive(Default)]
pub struct Inner {
    pub abstract_states: HashSet<u64>,
    pub branch: HashMap<(u32, u64), BranchEntry>,
    pub branch_points: usize,
    pub errors: HashMap<(usize, String, String), ErrorEntry>,
    pub violations: Vec<Violation>,
    pub first: HashMap<usize, (u64, u64, usize)>,
    pub growth: Vec<[u64; 3]>,
    pub timeline: Vec<TimelineEntry>,
    pub matches: Vec<Value>,
    pub traces: BTreeMap<u64, Value>,
    pub search: Vec<Value>,
    pub notes: Vec<String>,
    pub info: BTreeMap<String, Value>,
    pub attempts: HashMap<(usize, String), [u64; 2]>,
    /// Traces kept per violation kind and command.
    pub violation_traces: HashMap<String, usize>,
}

pub struct Coverage {
    pub mode: String,
    pub threads: usize,
    pub outcome_hits: Vec<AtomicU64>,
    pub state_hits: Vec<AtomicU64>,
    pub transition_hits: Vec<AtomicU64>,
    pub binding_invocations: Vec<AtomicU64>,
    /// Per command: requests sent and nanoseconds spent answering them (bindings included).
    pub command_sends: Vec<AtomicU64>,
    pub command_nanos: Vec<AtomicU64>,
    pub outcomes_reached: AtomicU64,
    pub states_reached: AtomicU64,
    pub transitions_reached: AtomicU64,
    pub walks: AtomicU64,
    pub steps: AtomicU64,
    pub done: AtomicBool,
    pub started: Instant,
    pub inner: Mutex<Inner>,
    pub feed: Mutex<VecDeque<FeedLine>>,
    pub version: AtomicU64,
}

fn counters(n: usize) -> Vec<AtomicU64> {
    (0..n).map(|_| AtomicU64::new(0)).collect()
}

/// Keeps at most this many replay traces in a result.
const MAX_TRACES: usize = 64;

impl Coverage {
    pub fn new(cat: &Catalog, mode: &str, threads: usize) -> Self {
        Self {
            mode: mode.to_owned(),
            threads,
            outcome_hits: counters(cat.outcomes_total),
            state_hits: counters(cat.states_total),
            transition_hits: counters(cat.transitions_total),
            binding_invocations: counters(cat.commands.len()),
            command_sends: counters(cat.commands.len()),
            command_nanos: counters(cat.commands.len()),
            outcomes_reached: AtomicU64::new(0),
            states_reached: AtomicU64::new(0),
            transitions_reached: AtomicU64::new(0),
            walks: AtomicU64::new(0),
            steps: AtomicU64::new(0),
            done: AtomicBool::new(false),
            started: Instant::now(),
            inner: Mutex::new(Inner::default()),
            feed: Mutex::new(VecDeque::new()),
            version: AtomicU64::new(0),
        }
    }

    /// Counts a reached outcome; `true` the first time the run reaches it.
    pub fn hit_outcome(&self, gid: usize) -> bool {
        let first = self.outcome_hits[gid].fetch_add(1, Ordering::Relaxed) == 0;
        if first {
            self.outcomes_reached.fetch_add(1, Ordering::Relaxed);
        }
        first
    }

    pub fn hit_state(&self, gid: usize) {
        if self.state_hits[gid].fetch_add(1, Ordering::Relaxed) == 0 {
            self.states_reached.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn hit_transition(&self, gid: usize) {
        if self.transition_hits[gid].fetch_add(1, Ordering::Relaxed) == 0 {
            self.transitions_reached.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn outcome_snapshot(&self) -> Vec<u64> {
        self.outcome_hits
            .iter()
            .map(|a| a.load(Ordering::Relaxed))
            .collect()
    }

    pub fn feed(&self, line: FeedLine) {
        let mut feed = self.feed.lock().expect("feed");
        feed.push_back(line);
        while feed.len() > 200 {
            feed.pop_front();
        }
    }

    pub fn merge(&self, walk: u64, seed: u64, found: WalkFindings) {
        let steps = self.steps.load(Ordering::Relaxed);
        let mut inner = self.inner.lock().expect("coverage");
        inner.abstract_states.extend(found.abstract_states);
        for ((cmd, state), (outcomes, desc)) in found.branch {
            let entry = inner
                .branch
                .entry((cmd, state))
                .or_insert_with(|| BranchEntry {
                    outcomes: BTreeSet::new(),
                    state: desc.clone(),
                    seed,
                    walk,
                });
            let before = entry.outcomes.len();
            entry.outcomes.extend(outcomes);
            let after = entry.outcomes.len();
            if before < 2 && after >= 2 {
                entry.state = desc;
                entry.seed = seed;
                entry.walk = walk;
                inner.branch_points += 1;
            }
        }
        for (key, (count, example)) in found.errors {
            let entry = inner.errors.entry(key).or_insert_with(|| ErrorEntry {
                count: 0,
                walk,
                seed,
                example,
            });
            entry.count += count;
        }
        for (gid, step) in found.first {
            inner.first.entry(gid).or_insert((walk, seed, step));
        }
        let interesting = found.interesting || !found.violations.is_empty();
        // The trace of a walk with a violation is kept while its kind (for a violation of one
        // command, its kind and command) has fewer than three kept, beside the walks that reached
        // something first: every violation found can be replayed.
        let mut keep_violation = false;
        for v in &found.violations {
            let key = match v.kind.as_str() {
                "idempotency" | "entity-invariant" => format!("{} {}", v.kind, v.command),
                _ => v.kind.clone(),
            };
            let n = inner.violation_traces.entry(key).or_insert(0);
            if *n < 3 {
                *n += 1;
                keep_violation = true;
            }
        }
        inner.violations.extend(found.violations);
        for (key, [all, live]) in found.attempts {
            let entry = inner.attempts.entry(key).or_default();
            entry[0] += all;
            entry[1] += live;
        }
        if (keep_violation || interesting && inner.traces.len() < MAX_TRACES)
            && !found.trace.is_empty()
        {
            inner.traces.insert(
                walk,
                json!({"walk": walk, "seed": seed, "steps": found.trace}),
            );
        }
        let reached = self.outcomes_reached.load(Ordering::Relaxed);
        let abstract_count = inner.abstract_states.len() as u64;
        let push = inner.growth.last().is_none_or(|last| {
            steps >= last[0] + 50 || last[1] != abstract_count && steps > last[0]
        });
        if push {
            inner.growth.push([steps, abstract_count, reached]);
            if inner.growth.len() > 4000 {
                let thinned: Vec<[u64; 3]> = inner.growth.iter().step_by(2).copied().collect();
                inner.growth = thinned;
            }
        }
        drop(inner);
        self.walks.fetch_add(1, Ordering::Relaxed);
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    pub fn violations(&self) -> usize {
        self.inner.lock().map(|i| i.violations.len()).unwrap_or(0)
    }

    /// The live and final numbers: what the page shows and what `result.json` carries.
    pub fn snapshot(&self, cat: &Catalog, full: bool) -> Value {
        let walks = self.walks.load(Ordering::Relaxed);
        let steps = self.steps.load(Ordering::Relaxed);
        let elapsed = self.started.elapsed().as_secs_f64().max(1e-9);
        let inner = self.inner.lock().expect("coverage");
        let mut outcome_hits: Vec<u64> = self.outcome_snapshot();
        if outcome_hits.is_empty() {
            outcome_hits = vec![];
        }
        let state_hits: Vec<u64> = self
            .state_hits
            .iter()
            .map(|a| a.load(Ordering::Relaxed))
            .collect();
        let transition_hits: Vec<u64> = self
            .transition_hits
            .iter()
            .map(|a| a.load(Ordering::Relaxed))
            .collect();
        let binding: Vec<u64> = self
            .binding_invocations
            .iter()
            .map(|a| a.load(Ordering::Relaxed))
            .collect();
        let mut branch: Vec<&BranchEntry> = inner
            .branch
            .values()
            .filter(|b| b.outcomes.len() > 1)
            .collect();
        branch.sort_by(|a, b| {
            b.outcomes
                .len()
                .cmp(&a.outcomes.len())
                .then(a.walk.cmp(&b.walk))
        });
        let branch_cmds: Vec<Value> = {
            // per command: how many abstract states branch, the largest fan-out
            let mut per: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
            for ((cmd, _), entry) in &inner.branch {
                if entry.outcomes.len() > 1 {
                    let e = per.entry(*cmd).or_default();
                    e.0 += 1;
                    e.1 = e.1.max(entry.outcomes.len());
                }
            }
            let mut rows: Vec<_> = per.into_iter().collect();
            rows.sort_by_key(|r| std::cmp::Reverse(r.1 .0));
            rows.into_iter()
                .take(if full { 200 } else { 40 })
                .map(|(cmd, (states, fan))| {
                    json!({"command": cat.commands[cmd as usize].name.to_string(), "states": states, "max_fanout": fan})
                })
                .collect()
        };
        let branch_list: Vec<Value> = branch
            .iter()
            .take(if full { 200 } else { 30 })
            .map(|b| {
                let names: Vec<String> = b
                    .outcomes
                    .iter()
                    .map(|g| {
                        let (c, o) = cat.outcome_of[*g as usize];
                        let _ = c;
                        cat.commands[c].outcomes[o].name.clone()
                    })
                    .collect();
                let cmd = cat.outcome_of[*b.outcomes.iter().next().unwrap_or(&0) as usize].0;
                json!({"command": cat.commands[cmd].name.to_string(), "outcomes": names, "state": b.state, "walk": b.walk, "seed": b.seed})
            })
            .collect();
        let mut errors: Vec<(&(usize, String, String), &ErrorEntry)> =
            inner.errors.iter().collect();
        errors.sort_by_key(|e| std::cmp::Reverse(e.1.count));
        let errors: Vec<Value> = errors
            .iter()
            .take(if full { 500 } else { 40 })
            .map(|((cmd, kind, why), e)| {
                json!({"command": cat.commands[*cmd].name.to_string(), "kind": kind, "why": why, "count": e.count, "walk": e.walk, "seed": e.seed, "example": e.example})
            })
            .collect();
        // The commands that took the most time, summed: where a run's seconds go.
        let mut slow: Vec<(usize, u64, u64)> = (0..cat.commands.len())
            .map(|c| {
                (
                    c,
                    self.command_sends[c].load(Ordering::Relaxed),
                    self.command_nanos[c].load(Ordering::Relaxed),
                )
            })
            .filter(|(_, n, _)| *n > 0)
            .collect();
        slow.sort_by_key(|s| std::cmp::Reverse(s.2));
        let slowest: Vec<Value> = slow
            .iter()
            .take(10)
            .map(|(c, n, ns)| json!({"command": cat.commands[*c].name.to_string(), "sends": n, "seconds": *ns as f64 / 1e9, "ms_per_send": *ns as f64 / 1e6 / *n as f64}))
            .collect();
        let feed: Vec<FeedLine> = self
            .feed
            .lock()
            .map(|f| f.iter().cloned().collect())
            .unwrap_or_default();
        let first: BTreeMap<String, Value> = if full {
            inner
                .first
                .iter()
                .map(|(gid, (walk, seed, step))| {
                    let (c, o) = cat.outcome_of[*gid];
                    (
                        format!(
                            "{}/{}",
                            cat.commands[c].name, cat.commands[c].outcomes[o].name
                        ),
                        json!({"walk": walk, "seed": seed, "step": step}),
                    )
                })
                .collect()
        } else {
            BTreeMap::new()
        };
        let mut value = json!({
            "mode": self.mode,
            "running": !self.done.load(Ordering::Relaxed),
            "threads": self.threads,
            "walks": walks,
            "steps": steps,
            "elapsed_s": elapsed,
            "walks_per_s": walks as f64 / elapsed,
            "steps_per_s": steps as f64 / elapsed,
            "outcomes_reached": self.outcomes_reached.load(Ordering::Relaxed),
            "outcomes_declared": cat.outcomes_total,
            "states_reached": self.states_reached.load(Ordering::Relaxed),
            "states_declared": cat.states_total,
            "transitions_reached": self.transitions_reached.load(Ordering::Relaxed),
            "transitions_declared": cat.transitions_total,
            "commands_declared": cat.commands.len(),
            "abstract_states": inner.abstract_states.len(),
            "branch_points": inner.branch_points,
            "violation_count": inner.violations.len(),
            "violations_by_kind": inner.violations.iter().fold(BTreeMap::<String, usize>::new(), |mut m, v| {
                *m.entry(v.kind.clone()).or_default() += 1;
                m
            }),
            "violations": inner.violations.iter().take(if full { 1000 } else { 50 }).collect::<Vec<_>>(),
            "outcome_hits": outcome_hits,
            "state_hits": state_hits,
            "transition_hits": transition_hits,
            "binding_invocations": binding,
            "growth": inner.growth,
            "feed": feed,
            "branch": branch_list,
            "branch_commands": branch_cmds,
            "errors": errors,
            "slowest_commands": slowest,
            "timeline": inner.timeline,
            "matches": inner.matches,
            "search": inner.search,
            "notes": inner.notes,
            "info": inner.info,
        });
        if full {
            value["first_reached"] = json!(first);
            value["traces"] = json!(inner.traces.values().collect::<Vec<_>>());
        }
        value
    }
}
