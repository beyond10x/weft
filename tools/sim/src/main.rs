//! weft-sim: explore and simulate the weft ESS model in memory.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde_json::json;

use weft_sim::catalog::Catalog;
use weft_sim::coverage::Coverage;
use weft_sim::gen::Candidates;
use weft_sim::{experiment, explore, report, serve, simulate, spec};

const USAGE: &str = "usage:
  weft-sim explore  --spec DIR --seed N --walks W --steps S --threads T --out DIR
                    [--serve [ADDR]] [--exit-when-done] [--max-seconds N] [--patterns FILE]
                    [--from EXPERIMENT]   every walk starts from the experiment's setup
                    [--settle N]          after the setup, up to N ticks of its controllers (until no work);
                                          after each walk the same: still working is `no-convergence`
                    [--controller-share P] a share P (0-1) of walk steps is a tick of its controllers
                    [--retry-bound N]     unbounded binding retries stop after N attempts (default 16; 0: as the spec)
                    [--invariants LIST]   system invariants checked in walks, comma-separated, or none
                                          (default: idempotency,no-double-placement,quota-conservation,tenant-isolation)
                    [--check-every N]     check them after every N-th step of a walk (default 1; 0: never)
  weft-sim simulate --spec DIR --experiment FILE --out DIR
                    [--threads T] [--serve [ADDR]] [--exit-when-done] [--no-search]
  weft-sim replay   --spec DIR --trace DIR/result.json --walk K
  weft-sim describe --spec DIR [--prefix weft.nodes.]   commands, inputs, outcomes, bindings

--serve serves the live page (default 127.0.0.1:7480, loopback only) and keeps it served after the
run until Ctrl-C, or exits with --exit-when-done. Exit status 1 on an invariant violation; 3 when a
setup step is not accepted as declared (its `expect`, or an accepting outcome without one); with
--from such a step stops the run before any walk.";

pub const DEFAULT_ADDR: &str = "127.0.0.1:7480";
const ESS_REV: &str = "c0b5f560cb7cae0b04d845f27b06341d3013218e (main, 2026-11-26; tag 0.99.2 cannot parse the current spec)";

struct Args {
    command: String,
    flags: BTreeMap<String, String>,
}

fn parse() -> Result<Args, String> {
    let mut args = std::env::args().skip(1);
    let command = args.next().ok_or_else(|| USAGE.to_owned())?;
    let rest: Vec<String> = args.collect();
    let mut flags = BTreeMap::new();
    let mut i = 0;
    while i < rest.len() {
        let key = rest[i]
            .strip_prefix("--")
            .ok_or_else(|| format!("unexpected argument `{}`\n{USAGE}", rest[i]))?
            .to_owned();
        let takes_value = !matches!(key.as_str(), "exit-when-done" | "no-search");
        if takes_value && i + 1 < rest.len() && !rest[i + 1].starts_with("--") {
            flags.insert(key, rest[i + 1].clone());
            i += 2;
        } else {
            flags.insert(key, String::new());
            i += 1;
        }
    }
    Ok(Args { command, flags })
}

impl Args {
    fn get(&self, key: &str, default: &str) -> String {
        self.flags.get(key).cloned().filter(|v| !v.is_empty()).unwrap_or_else(|| default.to_owned())
    }
    fn num(&self, key: &str, default: u64) -> Result<u64, String> {
        match self.flags.get(key) {
            Some(v) if !v.is_empty() => v.parse().map_err(|_| format!("--{key} {v}: not a number")),
            _ => Ok(default),
        }
    }
    fn has(&self, key: &str) -> bool {
        self.flags.contains_key(key)
    }
}

fn load(spec_dir: &PathBuf) -> Result<(Catalog, f64), String> {
    load_without(spec_dir, &[])
}

fn load_without(spec_dir: &std::path::Path, lost: &[String]) -> Result<(Catalog, f64), String> {
    load_bounded(spec_dir, lost, 0)
}

/// As `load_without`, with every unbounded binding retry bounded at `retry_bound` attempts (0:
/// the spec as written).
fn load_bounded(spec_dir: &std::path::Path, lost: &[String], retry_bound: u32) -> Result<(Catalog, f64), String> {
    let started = Instant::now();
    let (ir, bounded) = spec::load_with(spec_dir, lost, (retry_bound > 0).then_some(retry_bound))?;
    let mut cat = Catalog::new(ir);
    cat.retry_bounded = bounded.into_iter().collect();
    cat.retry_bound = retry_bound;
    Ok((cat, started.elapsed().as_secs_f64()))
}

/// Explore's bound on a binding's retries (`--retry-bound`): every walk keeps going past a binding
/// the spec retries without bound.
const DEFAULT_RETRY_BOUND: u64 = 16;

fn start_live(args: &Args, cat: &Arc<Catalog>, cov: &Arc<Coverage>) -> Result<Option<Arc<serve::Live>>, String> {
    if !args.has("serve") {
        return Ok(None);
    }
    let addr = serve::check_addr(&args.get("serve", DEFAULT_ADDR))?;
    let live = Arc::new(serve::Live {
        cat: cat.clone(),
        cov: cov.clone(),
        model: cat.model_json(),
        last: Mutex::new(None),
    });
    // The default port may be taken by another service on the host: the next nine are tried.
    let explicit = args.flags.get("serve").is_some_and(|v| !v.is_empty());
    let mut bound = None;
    let mut last_error = String::new();
    for offset in 0..if explicit { 1 } else { 10 } {
        let mut candidate = addr;
        candidate.set_port(addr.port() + offset);
        match serve::start(candidate, live.clone()) {
            Ok(()) => {
                bound = Some(candidate);
                break;
            }
            Err(e) => last_error = e,
        }
    }
    let addr = bound.ok_or(last_error)?;
    eprintln!("weft-sim: live page at http://{addr}/");
    Ok(Some(live))
}

fn finish(args: &Args, live: Option<Arc<serve::Live>>, snap: serde_json::Value, violations: usize) -> ExitCode {
    if let Some(live) = live {
        *live.last.lock().expect("live") = Some(snap);
        if !args.has("exit-when-done") {
            eprintln!("weft-sim: run done; the page stays served until Ctrl-C");
            loop {
                std::thread::park();
            }
        }
        // Lets an open event stream read the final snapshot.
        std::thread::sleep(std::time::Duration::from_millis(1500));
    }
    if violations > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn explore_cmd(args: &Args) -> Result<ExitCode, String> {
    let spec_dir = PathBuf::from(args.get("spec", "ess"));
    let out = PathBuf::from(args.get("out", "sim/out/explore"));
    let config = explore::Config {
        seed: args.num("seed", 1)?,
        walks: args.num("walks", 64)?,
        steps: args.num("steps", 60)? as usize,
        threads: args.num("threads", 4)?.clamp(1, 8) as usize,
        max_seconds: args.num("max-seconds", 280)?,
        setup: None,
        controllers: None,
        checks: explore::Checks::default(),
    };
    let settle = args.num("settle", 0)? as usize;
    let mut config = config;
    config.checks.every = args.num("check-every", 1)? as usize;
    config.checks.converge = settle;
    if let Some(list) = args.flags.get("invariants").filter(|v| !v.is_empty()) {
        config.checks.invariants = list.split(',').map(str::trim).filter(|s| !s.is_empty() && *s != "none").map(ToOwned::to_owned).collect();
        if let Some(bad) = config.checks.invariants.iter().find(|i| !simulate::ALL_INVARIANTS.contains(&i.as_str())) {
            return Err(format!("--invariants: no invariant `{bad}` (known: {})", simulate::ALL_INVARIANTS.join(", ")));
        }
    }
    let share: f64 = match args.flags.get("controller-share") {
        Some(v) if !v.is_empty() => v.parse().ok().filter(|p| (0.0..=1.0).contains(p)).ok_or(format!("--controller-share {v}: a number from 0 to 1"))?,
        _ => 0.0,
    };
    let patterns: Vec<experiment::Pattern> = match args.flags.get("patterns") {
        Some(p) if !p.is_empty() => {
            let text = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
            ess_yaml::from_str(&text).map_err(|e| format!("{p}: {e}"))?
        }
        _ => Vec::new(),
    };
    let from = args.flags.get("from").filter(|v| !v.is_empty()).cloned();
    if from.is_none() && (settle > 0 || share > 0.0) {
        return Err("--settle and --controller-share run the controllers of the --from experiment".into());
    }
    let exp = from.as_ref().map(|f| experiment::load(&PathBuf::from(f))).transpose()?;
    let lost = exp.as_ref().map(|e| e.lost_bindings.clone()).unwrap_or_default();
    let retry_bound = args.num("retry-bound", DEFAULT_RETRY_BOUND)?.min(u64::from(u32::MAX)) as u32;
    if retry_bound == 1 {
        return Err("--retry-bound 1: a bound is at least 2 attempts (0: the spec as written)".into());
    }
    let (cat, compile_s) = load_bounded(&spec_dir, &lost, retry_bound)?;
    let cat = Arc::new(cat);
    let cov = Arc::new(Coverage::new(&cat, "explore", config.threads));
    let cands = Candidates::default();
    let mut setup_info = serde_json::Value::Null;
    if let (Some(file), Some(exp)) = (&from, &exp) {
        let setup = simulate::record_setup(&cat, &cov, &cands, exp, settle)?;
        if !setup.refusals.is_empty() {
            return Err(format!(
                "--from {file}: {} setup steps not accepted as declared:\n  {}",
                setup.refusals.len(),
                setup.refusals.join("\n  ")
            ));
        }
        let settled = match setup.settled_at {
            Some(t) => format!("settled at tick {t}"),
            None if settle > 0 => format!("still working after {} ticks", setup.settle_ticks),
            None => "no settle".into(),
        };
        eprintln!(
            "weft-sim: setup of {} ({} requests, {} more in {} settle ticks, {settled}) in {:.2} s; every walk replays it",
            exp.name,
            setup.setup_steps,
            setup.steps.len() - setup.setup_steps,
            setup.settle_ticks,
            setup.seconds
        );
        setup_info = json!({
            "experiment": file, "name": exp.name, "steps": setup.steps.len(), "setup_steps": setup.setup_steps,
            "settle": settle, "settle_ticks": setup.settle_ticks, "settled_at": setup.settled_at,
            "controller_share": share, "seconds": setup.seconds,
            "outcomes_reached": cov.outcomes_reached.load(Ordering::Relaxed),
            "violations": setup.violations.len(),
        });
        if !setup.violations.is_empty() {
            let mut inner = cov.inner.lock().expect("coverage");
            for mut v in setup.violations {
                v.detail = format!("in the setup: {}", v.detail);
                inner.violations.push(v);
            }
        }
        if share > 0.0 || settle > 0 {
            config.controllers = Some(explore::WalkControllers { exp: exp.clone(), vars: setup.vars.clone(), share });
        }
        config.setup = Some(setup.steps);
    }
    {
        let mut inner = cov.inner.lock().expect("coverage");
        inner.info.insert("spec".into(), json!(spec_dir.display().to_string()));
        inner.info.insert("compile_s".into(), json!(compile_s));
        inner.info.insert("seed".into(), json!(config.seed));
        inner.info.insert("walks_requested".into(), json!(config.walks));
        inner.info.insert("steps_per_walk".into(), json!(config.steps));
    }
    let live = start_live(args, &cat, &cov)?;
    let started = Instant::now();
    // One pattern per run of the walk loop: the first is matched; more run in turn.
    if patterns.is_empty() {
        explore::run(&cat, &cov, &cands, &config, None);
    } else {
        for p in &patterns {
            explore::run(&cat, &cov, &cands, &config, Some(p));
        }
    }
    let run_s = started.elapsed().as_secs_f64();
    cov.done.store(true, Ordering::Relaxed);
    let loc = report::Locator::new(&spec_dir);
    let run = json!({
        "command": std::env::args().collect::<Vec<_>>().join(" "),
        "ess": ESS_REV,
        "compile_s": compile_s,
        "run_s": run_s,
        "threads": config.threads,
        "seed": config.seed,
        "walks": config.walks,
        "invariants": config.checks.invariants.iter().collect::<Vec<_>>(),
        "check_every": config.checks.every,
        "converge": config.checks.converge,
        "steps": config.steps,
        "candidate_sets": cands.len(),
        "from": setup_info,
        "retry_bound": retry_bound,
        "retry_bounded": cat.retry_bounded.len(),
    });
    let snap = report::write(&out, &cat, &cov, &loc, run)?;
    let violations = cov.violations();
    eprintln!(
        "weft-sim explore: {} walks, {} steps in {:.1} s ({:.0} steps/s, {:.1} walks/s, {} threads); outcomes {}/{}, states {}/{}, transitions {}/{}, abstract states {}, branch points {}, violations {}; commands never accepted {} (blocked-by {})",
        snap["walks"], snap["steps"], run_s, snap["steps_per_s"].as_f64().unwrap_or(0.0), snap["walks_per_s"].as_f64().unwrap_or(0.0), config.threads,
        snap["outcomes_reached"], snap["outcomes_declared"], snap["states_reached"], snap["states_declared"],
        snap["transitions_reached"], snap["transitions_declared"], snap["abstract_states"], snap["branch_points"], violations,
        snap["gaps"]["commands_never_accepted"].as_array().map_or(0, Vec::len), snap["gaps"]["blocked_by_count"]
    );
    if let Some(r) = snap["info"].get("setup_replay") {
        let f = |k: &str| r[k].as_f64().unwrap_or(0.0);
        let walks = f("walks").max(1.0);
        eprintln!(
            "weft-sim: setup replayed by {} walks in {:.1} s summed ({:.1} ms a walk); {} walks answered a step differently; walks' own steps {:.1} s summed ({:.1} ms a walk); {} controller ticks sent {} requests in {:.1} s summed ({:.1} ms a walk)",
            r["walks"], f("seconds"), 1000.0 * f("seconds") / walks, r["walks_differ"],
            f("walk_seconds"), 1000.0 * f("walk_seconds") / walks,
            r["controller_ticks"], r["controller_sends"], f("controller_seconds"), 1000.0 * f("controller_seconds") / walks
        );
    }
    if let Some(c) = snap["info"].get("checks") {
        let f = |k: &str| c[k].as_f64().unwrap_or(0.0);
        eprintln!(
            "weft-sim: system checks ({}, every {} steps) {:.1} s summed, {} duplicates sent; convergence: {} walks, {} ticks, {} requests, {:.1} s summed",
            config.checks.invariants.iter().cloned().collect::<Vec<_>>().join(","), config.checks.every, f("seconds"), f("duplicates"),
            f("converge_walks"), f("converge_ticks"), f("converge_sends"), f("converge_seconds")
        );
    }
    if violations > 0 {
        eprintln!("weft-sim: violations by kind: {}", snap["violations_by_kind"]);
        // The first violation of each kind, with the command that replays its walk.
        let mut shown = std::collections::BTreeSet::new();
        for v in snap["violations"].as_array().into_iter().flatten() {
            let kind = v["kind"].as_str().unwrap_or("");
            if shown.insert(kind.to_owned()) {
                eprintln!(
                    "  {kind}: walk {} seed {} step {} after {}: {}\n    replay: weft-sim replay --spec {} --trace {}/result.json --walk {}",
                    v["walk"], v["seed"], v["step"], v["command"], v["detail"].as_str().unwrap_or("").chars().take(200).collect::<String>(),
                    spec_dir.display(), out.display(), v["walk"]
                );
            }
        }
    }
    eprintln!("weft-sim: wrote {}/result.json and {}/index.html", out.display(), out.display());
    Ok(finish(args, live, snap, violations))
}

fn simulate_cmd(args: &Args) -> Result<ExitCode, String> {
    let spec_dir = PathBuf::from(args.get("spec", "ess"));
    let file = PathBuf::from(args.flags.get("experiment").cloned().filter(|v| !v.is_empty()).ok_or("--experiment FILE is required")?);
    let out = PathBuf::from(args.get("out", "sim/out/simulate"));
    let threads = args.num("threads", 4)?.clamp(1, 8) as usize;
    let mut exp = experiment::load(&file)?;
    if args.has("no-search") {
        exp.search = None;
    }
    let (cat, compile_s) = load_without(&spec_dir, &exp.lost_bindings)?;
    let cat = Arc::new(cat);
    let cov = Arc::new(Coverage::new(&cat, "simulate", threads));
    {
        let mut inner = cov.inner.lock().expect("coverage");
        inner.info.insert("experiment".into(), json!(exp.name));
        inner.info.insert("summary".into(), json!(exp.summary));
        inner.info.insert("compile_s".into(), json!(compile_s));
    }
    let live = start_live(args, &cat, &cov)?;
    let started = Instant::now();
    let outcome = simulate::run(&cat, &cov, &exp, threads)?;
    let run_s = started.elapsed().as_secs_f64();
    cov.done.store(true, Ordering::Relaxed);
    let loc = report::Locator::new(&spec_dir);
    let run = json!({
        "command": std::env::args().collect::<Vec<_>>().join(" "),
        "ess": ESS_REV,
        "experiment": file.display().to_string(),
        "compile_s": compile_s,
        "run_s": run_s,
        "result": outcome,
    });
    let snap = report::write(&out, &cat, &cov, &loc, run)?;
    let violations = cov.violations();
    eprintln!(
        "weft-sim simulate {}: {} steps in {:.1} s; checks {}; violations {}",
        exp.name, snap["steps"], run_s, snap["run"]["result"]["checks"], violations
    );
    eprintln!("weft-sim: wrote {}/result.json and {}/index.html", out.display(), out.display());
    // The run whose timeline is recorded (the only one, or the search's best point): a setup step
    // it did not accept as declared fails the run.
    let result = &snap["run"]["result"];
    let kpis = if result["kpis"].is_object() { &result["kpis"] } else { &result["best"]["kpis"] };
    let refused: Vec<String> = kpis["setup_refusals"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(ToOwned::to_owned)).collect())
        .unwrap_or_default();
    if !refused.is_empty() {
        eprintln!("weft-sim: {} setup steps not accepted as declared:\n  {}", refused.len(), refused.join("\n  "));
        let _ = finish(args, live, snap, violations);
        return Ok(ExitCode::from(3));
    }
    Ok(finish(args, live, snap, violations))
}

fn replay_cmd(args: &Args) -> Result<ExitCode, String> {
    let spec_dir = PathBuf::from(args.get("spec", "ess"));
    let trace = args.flags.get("trace").cloned().filter(|v| !v.is_empty()).ok_or("--trace FILE is required")?;
    let walk = args.num("walk", 0)?;
    let text = std::fs::read_to_string(&trace).map_err(|e| format!("{trace}: {e}"))?;
    let result: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{trace}: {e}"))?;
    let steps = result["traces"]
        .as_array()
        .and_then(|t| t.iter().find(|w| w["walk"].as_u64() == Some(walk)))
        .ok_or_else(|| format!("{trace}: no trace of walk {walk}"))?
        .clone();
    // A walk of `explore --from` starts from the experiment's setup: run it again first.
    let exp = result["run"]["from"]["experiment"]
        .as_str()
        .map(|f| experiment::load(&PathBuf::from(f)))
        .transpose()?;
    let lost = exp.as_ref().map(|e| e.lost_bindings.clone()).unwrap_or_default();
    let retry_bound = result["run"]["retry_bound"].as_u64().unwrap_or(0) as u32;
    let (cat, _) = load_bounded(&spec_dir, &lost, retry_bound)?;
    let cov = Coverage::new(&cat, "replay", 1);
    let setup = match &exp {
        Some(exp) => {
            let settle = result["run"]["from"]["settle"].as_u64().unwrap_or(0) as usize;
            simulate::record_setup(&cat, &cov, &Candidates::default(), exp, settle)?.steps
        }
        None => Vec::new(),
    };
    let (differ, _) = simulate::replay(&cat, &cov, walk, &steps, &setup)?;
    Ok(if differ == 0 { ExitCode::SUCCESS } else { ExitCode::from(2) })
}

fn describe_cmd(args: &Args) -> Result<ExitCode, String> {
    let spec_dir = PathBuf::from(args.get("spec", "ess"));
    let prefix = args.get("prefix", "");
    let (cat, compile_s) = load(&spec_dir)?;
    println!(
        "{}: {} commands, {} outcomes, {} entities, {} states, {} transitions, {} actors, {} bindings; compiled in {compile_s:.2} s",
        cat.ir.system(), cat.commands.len(), cat.outcomes_total, cat.entities.len(), cat.states_total,
        cat.transitions_total, cat.actors.len(), cat.ir.bindings().len()
    );
    for c in cat.commands.iter().filter(|c| c.name.to_string().starts_with(&prefix)) {
        let actors: Vec<String> = c.actors.iter().map(|a| cat.actors[*a].name.to_string().rsplit('.').next().unwrap_or("").to_owned()).collect();
        println!("\n{}  [{}]", c.name, actors.join(", "));
        for f in &c.input {
            println!("    in  {}: {}", f.name, f.type_ref);
        }
        for f in &c.response {
            println!("    out {}: {}", f.name, f.type_ref);
        }
        for t in &c.ties {
            println!("    tie input.{} = {}.{} (the row input.{} names)", t.input_field, cat.entities[t.ent].name, t.row_field, t.via);
        }
        for t in &c.set_ties {
            let pairs: Vec<String> = t.pairs.iter().map(|(r, i)| format!("input.{i} = {r}")).collect();
            let states: Vec<&str> = t.states.iter().map(|s| cat.entities[t.ent].states[*s].as_str()).collect();
            println!("    needs a {} row [{}]: {}", cat.entities[t.ent].name, states.join(", "), pairs.join(", "));
        }
        let command = &cat.ir.commands()[&c.name];
        for (o, out) in c.outcomes.iter().zip(&command.outcomes) {
            let emits: Vec<String> = out.emits.iter().map(|e| e.name().to_string().rsplit('.').next().unwrap_or("").to_owned()).collect();
            let subject = out.subject.as_ref().map(|s| format!("{} {}", s.effect.verb(), s.entity.name())).unwrap_or_default();
            println!("    -> {} ({}{}) {} {}", o.name, o.kind, if o.error { ", error" } else { "" }, subject, if emits.is_empty() { String::new() } else { format!("emits {}", emits.join(" ")) });
        }
    }
    if prefix.is_empty() || prefix == "bindings" {
        for (name, b) in cat.ir.bindings() {
            println!("binding {name}: {} -> {}", b.cause, b.command.name());
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let args = match parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let result = match args.command.as_str() {
        "explore" => explore_cmd(&args),
        "simulate" => simulate_cmd(&args),
        "replay" => replay_cmd(&args),
        "describe" => describe_cmd(&args),
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        other => Err(format!("unknown command `{other}`\n{USAGE}")),
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("weft-sim: {e}");
            ExitCode::from(2)
        }
    }
}
