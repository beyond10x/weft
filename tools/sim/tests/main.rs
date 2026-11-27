//! The one integration-test binary of weft-sim. No test reads the weft spec (it changes under
//! other sessions); the fixture under `tests/fixture` is a two-command model.

use std::path::PathBuf;

use weft_sim::catalog::Catalog;
use weft_sim::coverage::Coverage;
use weft_sim::experiment;
use weft_sim::explore::{self, Config};
use weft_sim::gen::Candidates;
use weft_sim::rng::Rng;
use weft_sim::walk::{bound_emits, Session};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixture")
}

fn catalog() -> Catalog {
    Catalog::new(weft_sim::spec::load(&fixture()).expect("the fixture compiles"))
}

fn explore(cat: &Catalog, threads: usize) -> serde_json::Value {
    let cov = Coverage::new(cat, "explore", threads);
    let cands = Candidates::default();
    let config = Config { seed: 5, walks: 40, steps: 12, threads, max_seconds: 50, setup: None, controllers: None, checks: Default::default() };
    explore::run(cat, &cov, &cands, &config, None);
    cov.snapshot(cat, true)
}

#[test]
fn the_generator_is_deterministic() {
    let mut a = Rng::new(9);
    let mut b = Rng::new(9);
    for _ in 0..100 {
        assert_eq!(a.next_u64(), b.next_u64());
    }
    assert_ne!(Rng::walk_seed(1, 0), Rng::walk_seed(1, 1));
    let mut r = Rng::new(3);
    assert_eq!(r.weighted(&[0.0, 0.0]), None);
    for _ in 0..50 {
        assert_eq!(r.weighted(&[0.0, 2.0, 0.0]), Some(1));
    }
}

#[test]
fn messages_count_one_cause_whatever_the_identifier() {
    let a = weft_sim::walk::normalize("row 00000000-0000-4000-8000-000000000003 is gone");
    let b = weft_sim::walk::normalize("row 00000000-0000-4000-8000-000000000017 is gone");
    assert_eq!(a, "row <id> is gone");
    assert_eq!(a, b);
}

#[test]
fn the_live_page_refuses_an_address_off_this_host() {
    assert!(weft_sim::serve::check_addr("127.0.0.1:7480").is_ok());
    assert!(weft_sim::serve::check_addr("[::1]:7480").is_ok());
    assert!(weft_sim::serve::check_addr("0.0.0.0:7480").is_err());
    assert!(weft_sim::serve::check_addr("192.168.1.4:7480").is_err());
}

#[test]
fn explore_reaches_every_outcome_of_the_fixture() {
    let cat = catalog();
    let snap = explore(&cat, 2);
    assert_eq!(snap["outcomes_declared"], 4);
    assert_eq!(snap["outcomes_reached"], 4, "outcomes: {}", snap["outcome_hits"]);
    assert_eq!(snap["states_reached"], 2);
    assert_eq!(snap["transitions_reached"], 1);
    assert_eq!(snap["violation_count"], 0);
    assert!(snap["abstract_states"].as_u64().unwrap() >= 3);
    // CloseBox from a state holding one open box reaches `closed` and, sent again, `not-open`.
    assert!(snap["branch_points"].as_u64().unwrap() >= 1);
}

#[test]
fn walks_do_not_depend_on_the_thread_count() {
    let cat = catalog();
    let one = explore(&cat, 1);
    let three = explore(&cat, 3);
    assert_eq!(one["outcome_hits"], three["outcome_hits"]);
    assert_eq!(one["abstract_states"], three["abstract_states"]);
    assert_eq!(one["steps"], three["steps"]);
}

#[test]
fn a_session_keeps_a_shadow_row_per_answer_and_reads_views() {
    let cat = catalog();
    let cov = Coverage::new(&cat, "test", 1);
    let mut sess = Session::new(&cat, &cov, bound_emits(&cat));
    sess.begin(0, 1).unwrap();
    let open = weft_sim::walk::Request {
        cmd: cat.commands.iter().position(|c| c.name.to_string() == "demo.box.OpenBox").unwrap(),
        actor: Some(0),
        caller: None,
        input: [("label".to_owned(), ess_primitives::node::Node::Text("a".into()))].into(),
        force: None,
    };
    let answer = sess.send(&open);
    assert_eq!(answer.label, "opened");
    assert_eq!(sess.rows.len(), 1);
    let rows = sess.view("demo.box.Boxes", Default::default()).unwrap();
    assert_eq!(rows.len(), 1);
    let pattern: experiment::Pattern = ess_yaml::from_str(
        "{name: two-open, rows: [{entity: demo.box.Box, state: Open}, {entity: demo.box.Box, state: Open}]}",
    )
    .unwrap();
    assert!(pattern.matches(&sess).is_none());
    sess.send(&open);
    assert!(pattern.matches(&sess).is_some());
}

#[test]
fn the_committed_experiments_parse() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sim/experiments");
    let mut n = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "yaml") {
            let exp = experiment::load(&path).unwrap_or_else(|e| panic!("{e}"));
            assert!(!exp.setup.is_empty());
            n += 1;
        }
    }
    assert!(n >= 2);
}

#[test]
fn a_variable_name_may_be_built_from_another() {
    use serde_json::json;
    let vars: std::collections::BTreeMap<String, serde_json::Value> =
        [("i".to_owned(), json!(1)), ("node_1".to_owned(), json!("n1")), ("n".to_owned(), json!(3))].into();
    let sub = |s: &str| weft_sim::simulate::substitute_vars(&json!(s), &vars);
    assert_eq!(sub("${node_${i}}"), json!("n1"));
    assert_eq!(sub("${n}"), json!(3));
    assert_eq!(sub("g${i}-${n}"), json!("g1-3"));
    assert_eq!(sub("${missing}"), serde_json::Value::Null);
}

#[test]
fn a_view_pattern_joins_rows_and_an_absent_row_must_have_no_match() {
    let cat = catalog();
    let cov = Coverage::new(&cat, "test", 1);
    let mut sess = Session::new(&cat, &cov, bound_emits(&cat));
    sess.begin(0, 1).unwrap();
    let open = |label: &str| weft_sim::walk::Request {
        cmd: cat.commands.iter().position(|c| c.name.to_string() == "demo.box.OpenBox").unwrap(),
        actor: Some(0),
        caller: None,
        input: [("label".to_owned(), ess_primitives::node::Node::Text(label.into()))].into(),
        force: None,
    };
    let pattern = |text: &str| -> experiment::Pattern { ess_yaml::from_str(text).unwrap() };
    let two_labels = pattern(
        "{name: two, rows: [{view: demo.box.Boxes, as: a, state: Open}, {view: demo.box.Boxes, as: b}], where: [a.label != b.label]}",
    );
    let no_twin = pattern(
        "{name: lone, rows: [{view: demo.box.Boxes, as: a}, {view: demo.box.Boxes, as: b, absent: true}], where: [a.label == b.label, a.box_id != b.box_id]}",
    );
    assert!(two_labels.reads_views());
    sess.send(&open("x"));
    assert!(two_labels.matches(&sess).is_none());
    assert!(no_twin.matches(&sess).is_some(), "one box has no twin");
    sess.send(&open("x"));
    assert!(two_labels.matches(&sess).is_none(), "the same label twice");
    assert!(no_twin.matches(&sess).is_none(), "each box has a twin");
    sess.send(&open("y"));
    let rows = two_labels.matches(&sess).expect("x and y");
    assert_eq!(rows.len(), 2);
    assert!(no_twin.matches(&sess).is_some(), "y has no twin");
}

fn fixture_from() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixture-from")
}

/// `explore --from`: Polish needs a shelf labelled gold, which only the setup makes; Relabel
/// needs the shelf's own label (a guard tie); Jam is refused `stuck` on every shelf there is.
#[test]
fn explore_from_a_setup_reaches_what_an_empty_store_cannot_and_names_the_guard_that_blocks() {
    let dir = fixture_from();
    let cat = Catalog::new(weft_sim::spec::load(&dir).expect("the fixture compiles"));
    let gid = |cmd: &str, outcome: &str| {
        let c = cat.commands.iter().find(|c| c.name.to_string() == cmd).unwrap();
        c.outcomes.iter().find(|o| o.name == outcome).unwrap().gid
    };
    let run = |from: bool| {
        let cov = Coverage::new(&cat, "explore", 2);
        let cands = Candidates::default();
        let setup = from.then(|| {
            let exp = experiment::load(&dir.join("setup.yaml")).unwrap();
            let s = weft_sim::simulate::record_setup(&cat, &cov, &cands, &exp, 0).unwrap();
            assert!(s.refusals.is_empty(), "{:?}", s.refusals);
            s.steps
        });
        let config = Config { seed: 5, walks: 48, steps: 16, threads: 2, max_seconds: 50, setup, controllers: None, checks: Default::default() };
        explore::run(&cat, &cov, &cands, &config, None);
        let snap = cov.snapshot(&cat, true);
        let attempts = cov.inner.lock().unwrap().attempts.clone();
        let gaps = weft_sim::report::gaps(&cat, &snap, &weft_sim::report::Locator::new(&dir), &attempts);
        (snap, gaps)
    };
    let never = |gaps: &serde_json::Value, cmd: &str| {
        gaps["commands_never_accepted"].as_array().unwrap().iter().find(|n| n["command"] == cmd).cloned()
    };
    let (empty, empty_gaps) = run(false);
    let (from, from_gaps) = run(true);
    let hits = |snap: &serde_json::Value, g: usize| snap["outcome_hits"][g].as_u64().unwrap();

    assert_eq!(hits(&empty, gid("demo.shelf.Polish", "polished")), 0, "no random label is gold");
    assert!(hits(&from, gid("demo.shelf.Polish", "polished")) > 0, "the setup's gold shelf is polished");
    // From an empty store no shelf a walk makes is gold: Polish is blocked by the guard the setup
    // passes.
    let polish = never(&empty_gaps, "demo.shelf.Polish").unwrap();
    assert_eq!(polish["blocked_by"]["outcome"], "not-gold", "{polish}");
    assert_eq!(empty_gaps["blocked_by_count"], 2);
    assert!(never(&from_gaps, "demo.shelf.Polish").is_none());
    assert_eq!(from_gaps["blocked_by_count"], 1);
    assert_eq!(from["info"]["setup_replay"]["walks"], 48.0);
    assert_eq!(from["info"]["setup_replay"]["walks_differ"], 0.0);

    for (snap, gaps) in [(&empty, &empty_gaps), (&from, &from_gaps)] {
        assert!(hits(snap, gid("demo.shelf.Relabel", "relabelled")) > 0, "the label is copied from the shelf");
        let jam = never(gaps, "demo.shelf.Jam").expect("Jam is never accepted");
        let class = jam["class"].as_str().unwrap();
        assert!(class.starts_with("blocked-by:") && class.contains("shelf.yaml:"), "{jam}");
        assert_eq!(jam["blocked_by"]["outcome"], "stuck", "{jam}");
    }
}

/// A setup step not accepted as declared fails the run: `simulate` exits 3 after writing its
/// result, `explore --from` exits 2 before any walk.
#[test]
fn a_setup_step_not_accepted_as_declared_fails_the_run() {
    let dir = fixture_from();
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("setup-refused");
    let sim = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_weft-sim"))
            .args(args)
            .arg("--spec")
            .arg(&dir)
            .arg("--out")
            .arg(&out)
            .output()
            .unwrap()
    };
    let path = |f: &str| dir.join(f).display().to_string();
    let ok = sim(&["simulate", "--experiment", &path("setup.yaml")]);
    assert!(ok.status.success(), "{}", String::from_utf8_lossy(&ok.stderr));
    let bad = sim(&["simulate", "--experiment", &path("setup-refused.yaml")]);
    let stderr = String::from_utf8_lossy(&bad.stderr);
    assert_eq!(bad.status.code(), Some(3), "{stderr}");
    assert!(stderr.contains("polish-nothing (demo.shelf.Polish) answered no-such-shelf"), "{stderr}");
    let bad = sim(&["explore", "--walks", "2", "--steps", "2", "--threads", "1", "--from", &path("setup-refused.yaml")]);
    let stderr = String::from_utf8_lossy(&bad.stderr);
    assert_eq!(bad.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("polish-nothing"), "{stderr}");
}

#[test]
fn an_explore_run_cut_by_max_seconds_fails_and_still_writes_its_result() {
    // Hardening pass 3, finding D1, decided A: how many walks fit the cap depends on the host's
    // load, so a run that did not finish must not read as passed.
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("explore-cut");
    let _ = std::fs::remove_dir_all(&out);
    let run = |extra: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_weft-sim"))
            .args(["explore", "--steps", "4", "--threads", "1"])
            .args(extra)
            .arg("--spec")
            .arg(fixture())
            .arg("--out")
            .arg(&out)
            .output()
            .unwrap()
    };
    let cut = run(&["--walks", "1000000", "--max-seconds", "0"]);
    let stderr = String::from_utf8_lossy(&cut.stderr);
    assert_eq!(cut.status.code(), Some(4), "{stderr}");
    assert!(stderr.contains("cut by --max-seconds"), "{stderr}");
    assert!(out.join("result.json").is_file(), "the cut run still writes its result");
    let whole = run(&["--walks", "8", "--max-seconds", "60"]);
    assert!(whole.status.success(), "{}", String::from_utf8_lossy(&whole.stderr));
}

#[test]
fn a_lost_binding_the_model_does_not_declare_is_refused() {
    let err = weft_sim::spec::load_without(&fixture(), &["no-such-binding".to_owned()]).expect_err("unknown binding");
    assert!(err.contains("0 of them are declared"), "{err}");
    assert!(weft_sim::spec::load_without(&fixture(), &[]).is_ok());
}

/// `explore --from --settle --controller-share`: PlaceJob needs the job's ticket, which only the
/// view shows (the placer controller reads it), and FinishJob a placed job. From the setup alone
/// no walk finishes a job; after a settle the setup's job is placed; with controller ticks in the
/// walks the jobs the walks add are placed too.
#[test]
fn settle_and_controller_ticks_start_walks_from_what_the_controllers_did() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixture-settle");
    let cat = Catalog::new(weft_sim::spec::load(&dir).expect("the fixture compiles"));
    let exp = experiment::load(&dir.join("setup.yaml")).unwrap();
    let gid = |cmd: &str, outcome: &str| {
        let c = cat.commands.iter().find(|c| c.name.to_string() == cmd).unwrap();
        c.outcomes.iter().find(|o| o.name == outcome).unwrap().gid
    };
    let run = |settle: usize, share: f64| {
        let cov = Coverage::new(&cat, "explore", 2);
        let cands = Candidates::default();
        let s = weft_sim::simulate::record_setup(&cat, &cov, &cands, &exp, settle).unwrap();
        assert!(s.refusals.is_empty(), "{:?}", s.refusals);
        let info = (s.setup_steps, s.steps.len(), s.settle_ticks, s.settled_at);
        let controllers = (share > 0.0).then(|| explore::WalkControllers { exp: exp.clone(), vars: s.vars.clone(), share });
        let config = Config { seed: 5, walks: 32, steps: 16, threads: 2, max_seconds: 50, setup: Some(s.steps), controllers, checks: Default::default() };
        explore::run(&cat, &cov, &cands, &config, None);
        (cov.snapshot(&cat, true), info)
    };
    let hits = |snap: &serde_json::Value, g: usize| snap["outcome_hits"][g].as_u64().unwrap();
    let placed = gid("demo.job.PlaceJob", "placed");
    let finished = gid("demo.job.FinishJob", "finished");

    let (plain, info) = run(0, 0.0);
    assert_eq!(info, (1, 1, 0, None));
    assert_eq!(hits(&plain, placed), 0, "no walk draws the ticket");
    assert_eq!(hits(&plain, finished), 0, "no job is placed");

    // Tick 0 places the setup's job; tick 1 has no work: settled.
    let (settled, info) = run(4, 0.0);
    assert_eq!(info, (1, 2, 2, Some(1)));
    assert_eq!(hits(&settled, placed), 1, "the settle's placement counts once");
    assert!(hits(&settled, finished) > 0, "walks start from the placed job");
    assert_eq!(settled["info"]["setup_replay"]["walks_differ"], 0.0);
    assert_eq!(settled["info"]["setup_replay"]["controller_ticks"], 0.0);

    let (ticked, _) = run(4, 0.25);
    assert!(hits(&ticked, placed) > 1, "ticks place the jobs walks add");
    assert!(hits(&ticked, finished) > hits(&settled, finished));
    let r = &ticked["info"]["setup_replay"];
    assert!(r["controller_ticks"].as_f64().unwrap() > 0.0 && r["controller_sends"].as_f64().unwrap() > 0.0, "{r}");
}

// ---- system invariants inside explore -------------------------------------------------------

fn fixture_invariants() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixture-invariants")
}

/// The invariants fixture without Assign's `instance-assigned` guard (the lines between the guard
/// comments): an instance can be assigned twice, and a second delivery of Assign assigns again.
fn unguarded() -> PathBuf {
    without_guard(&fixture_invariants(), "nodes.yaml", "instance-assigned")
}

/// A copy of the fixture `from` (under the test's target directory) whose `file` lacks the lines
/// between its guard comments, the outcome `guard` among them.
fn without_guard(from: &std::path::Path, file: &str, guard: &str) -> PathBuf {
    let name = format!("{}-unguarded", from.file_name().unwrap().to_str().unwrap());
    let to = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(&to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let mut text = std::fs::read_to_string(&path).unwrap();
        if path.file_name().unwrap() == file {
            let begin = text.find("      # guard begin").unwrap();
            let end = text.find("      # guard end\n").unwrap() + "      # guard end\n".len();
            text.replace_range(begin..end, "");
            assert!(!text.contains(guard));
        }
        std::fs::write(to.join(path.file_name().unwrap()), text).unwrap();
    }
    to
}

fn checks(invariants: &[&str], every: usize, converge: usize) -> explore::Checks {
    explore::Checks { invariants: invariants.iter().map(|s| (*s).to_owned()).collect(), every, converge }
}

/// Explores `dir` from an empty store (or from `exp`'s setup) with `checks`.
fn explore_checked(cat: &Catalog, exp: Option<&experiment::Experiment>, checks: explore::Checks) -> serde_json::Value {
    let cov = Coverage::new(cat, "explore", 2);
    let cands = Candidates::default();
    let (setup, controllers) = match exp {
        Some(exp) => {
            let s = weft_sim::simulate::record_setup(cat, &cov, &cands, exp, checks.converge).unwrap();
            assert!(s.refusals.is_empty(), "{:?}", s.refusals);
            let controllers = explore::WalkControllers { exp: exp.clone(), vars: s.vars.clone(), share: 0.0 };
            (Some(s.steps), Some(controllers))
        }
        None => (None, None),
    };
    let config = Config { seed: 5, walks: 32, steps: 16, threads: 2, max_seconds: 50, setup, controllers, checks };
    explore::run(cat, &cov, &cands, &config, None);
    cov.snapshot(cat, true)
}

fn violations<'v>(snap: &'v serde_json::Value, kind: &str) -> Vec<&'v serde_json::Value> {
    snap["violations"].as_array().unwrap().iter().filter(|v| v["kind"] == kind).collect()
}

/// Replays the walk of `v` from the run's trace: no answer differs, and the replay's own checks
/// find a system invariant violation again.
fn replay_finds_it(cat: &Catalog, snap: &serde_json::Value, v: &serde_json::Value) {
    let walk = v["walk"].as_u64().unwrap();
    let trace = snap["traces"].as_array().unwrap().iter().find(|t| t["walk"] == walk).unwrap_or_else(|| panic!("no trace of walk {walk}"));
    let step = v["step"].as_u64().unwrap() as usize;
    assert!(trace["steps"].as_array().unwrap().len() >= step, "the trace reaches step {step}");
    let cov = Coverage::new(cat, "replay", 1);
    let (differ, broken) = weft_sim::simulate::replay(cat, &cov, walk, trace, &[]).unwrap();
    assert_eq!(differ, 0, "the replay answers as the walk did");
    assert!(broken > 0, "the replay finds the violation");
}

/// no-double-placement (and the other view invariants) after every step of an explore walk:
/// without the guard a walk assigns an instance twice; the violation names the walk, seed and
/// step, and its trace replays it. With the guard, none; with the checks off, none reported.
#[test]
fn no_double_placement_is_checked_inside_explore_walks_and_replays() {
    let view_checks = ["no-double-placement", "quota-conservation", "tenant-isolation"];
    let guarded = Catalog::new(weft_sim::spec::load(&fixture_invariants()).expect("the fixture compiles"));
    let snap = explore_checked(&guarded, None, checks(&view_checks, 1, 0));
    assert_eq!(snap["violation_count"], 0, "{}", snap["violations"]);

    let cat = Catalog::new(weft_sim::spec::load(&unguarded()).expect("the unguarded fixture compiles"));
    let snap = explore_checked(&cat, None, checks(&view_checks, 1, 0));
    let found = violations(&snap, "no-double-placement");
    assert!(!found.is_empty(), "{}", snap["violations"]);
    assert_eq!(found.len(), snap["violation_count"].as_u64().unwrap() as usize, "only no-double-placement");
    let v = found[0];
    assert!(v["detail"].as_str().unwrap().starts_with("instance "), "{v}");
    assert!(v["step"].as_u64().unwrap() >= 1 && v["seed"].as_u64().is_some(), "{v}");
    // Once per walk: the state stays broken, the walk reports its first step.
    let walks: std::collections::BTreeSet<u64> = found.iter().map(|v| v["walk"].as_u64().unwrap()).collect();
    assert_eq!(walks.len(), found.len());
    replay_finds_it(&cat, &snap, v);

    let off = explore_checked(&cat, None, checks(&view_checks, 0, 0));
    assert_eq!(off["violation_count"], 0);
}

/// idempotency inside explore: an accepted Assign (a binding invokes it `at_least_once`) is sent
/// again and must change no view. Guarded: refused `instance-assigned`, nothing changes.
/// Unguarded: a second assignment; the trace holds the duplicate (`by: duplicate`).
#[test]
fn a_redelivered_command_is_sent_again_and_must_change_no_view() {
    let guarded = Catalog::new(weft_sim::spec::load(&fixture_invariants()).expect("the fixture compiles"));
    let assign = guarded.commands.iter().position(|c| c.name.to_string() == "weft.nodes.Assign").unwrap();
    assert_eq!(guarded.redelivered.iter().copied().collect::<Vec<_>>(), vec![assign]);
    let snap = explore_checked(&guarded, None, checks(&["idempotency"], 1, 0));
    assert_eq!(snap["violation_count"], 0, "{}", snap["violations"]);
    assert!(snap["info"]["checks"]["duplicates"].as_f64().unwrap() > 0.0, "{}", snap["info"]);

    let cat = Catalog::new(weft_sim::spec::load(&unguarded()).expect("the unguarded fixture compiles"));
    let snap = explore_checked(&cat, None, checks(&["idempotency"], 1, 0));
    let found = violations(&snap, "idempotency");
    assert!(!found.is_empty(), "{}", snap["violations"]);
    assert_eq!(found.len(), snap["violation_count"].as_u64().unwrap() as usize);
    let v = found[0];
    assert_eq!(v["command"], "weft.nodes.Assign");
    assert!(v["detail"].as_str().unwrap().contains("weft.nodes.Assignments: "), "{v}");
    let walk = v["walk"].as_u64().unwrap();
    let trace = snap["traces"].as_array().unwrap().iter().find(|t| t["walk"] == walk).unwrap();
    let step = &trace["steps"][v["step"].as_u64().unwrap() as usize - 1];
    assert_eq!(step["by"], "duplicate", "{step}");
    replay_finds_it(&cat, &snap, v);
}

/// no-convergence with `--settle`: after each walk the experiment's controllers tick with no new
/// input. The releaser releases the active assignments and then has nothing to do; the
/// reassigner's Assign is refused on every tick, so a walk that leaves an assignment active never
/// settles.
#[test]
fn a_walk_whose_controllers_never_settle_is_a_no_convergence_violation() {
    let dir = fixture_invariants();
    let cat = Catalog::new(weft_sim::spec::load(&dir).expect("the fixture compiles"));
    let none: [&str; 0] = [];

    let converges = experiment::load(&dir.join("converges.yaml")).unwrap();
    let snap = explore_checked(&cat, Some(&converges), checks(&none, 0, 4));
    assert_eq!(snap["violation_count"], 0, "{}", snap["violations"]);
    let c = &snap["info"]["checks"];
    assert_eq!(c["converge_walks"], 32.0, "{c}");
    assert!(c["converge_ticks"].as_f64().unwrap() >= 32.0, "{c}");

    let loops = experiment::load(&dir.join("loops.yaml")).unwrap();
    let cov = Coverage::new(&cat, "explore", 1);
    let setup = weft_sim::simulate::record_setup(&cat, &cov, &Candidates::default(), &loops, 4).unwrap();
    assert_eq!(setup.settled_at, None);
    assert_eq!(setup.violations.len(), 1, "the setup itself does not settle");
    assert_eq!(setup.violations[0].kind, "no-convergence");
    let snap = explore_checked(&cat, Some(&loops), checks(&none, 0, 4));
    let found = violations(&snap, "no-convergence");
    assert!(!found.is_empty());
    assert_eq!(found.len(), snap["violation_count"].as_u64().unwrap() as usize);
    let v = found[0];
    assert!(v["detail"].as_str().unwrap().starts_with("the controllers still had work after 4 ticks with no new input: reassigner "), "{v}");
    // The convergence ticks are in the walk's trace (`by: converge`), so a replay sends them.
    let walk = v["walk"].as_u64().unwrap();
    let trace = snap["traces"].as_array().unwrap().iter().find(|t| t["walk"] == walk).unwrap();
    let ticks = trace["steps"].as_array().unwrap().iter().filter(|s| s["by"] == "converge").count();
    assert!(ticks >= 4, "at least one refused Assign a tick: {ticks}");
}

// ---- closed-link ----------------------------------------------------------------------------

fn fixture_closed_link() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixture-closed-link")
}

/// Runs the closed-link fixture's setup on `dir`: the setup's refusals and its violations.
fn closed_link_setup(dir: &std::path::Path) -> (Vec<String>, Vec<weft_sim::coverage::Violation>) {
    let cat = Catalog::new(weft_sim::spec::load(dir).expect("the fixture compiles"));
    let exp = experiment::load(&fixture_closed_link().join("setup.yaml")).unwrap();
    let cov = Coverage::new(&cat, "simulate", 1);
    let s = weft_sim::simulate::record_setup(&cat, &cov, &Candidates::default(), &exp, 0).unwrap();
    (s.refusals, s.violations)
}

/// closed-link passes with the guard: a formation in groups an open link joins, another formation
/// across the closed link, a released placement beside a live one in the closed pair; the last
/// placement, across the closed link, is refused `crosses-closed-link`.
#[test]
fn closed_link_passes_when_the_guard_refuses_the_crossing() {
    let (refusals, violations) = closed_link_setup(&fixture_closed_link());
    assert_eq!(refusals.len(), 1, "{refusals:?}");
    assert!(refusals[0].starts_with("f-in-a-again (weft.scheduling.PlaceInstance) answered crosses-closed-link"), "{refusals:?}");
    assert!(violations.is_empty(), "{violations:?}");
}

/// closed-link fires without the guard: the last placement puts formation f in group a beside its
/// live placement in b, which a closed link joins; one violation, at that step.
#[test]
fn closed_link_fires_on_a_formation_placed_across_a_closed_link() {
    let dir = without_guard(&fixture_closed_link(), "scheduling.yaml", "crosses-closed-link");
    let (refusals, violations) = closed_link_setup(&dir);
    assert!(refusals.is_empty(), "{refusals:?}");
    assert_eq!(violations.len(), 1, "{violations:?}");
    let v = &violations[0];
    assert_eq!(v.kind, "closed-link");
    assert_eq!(v.step, 8, "{v:?}");
    assert!(v.detail.starts_with("formation 00000000-0000-4000-8000-0000000000f1: placements "), "{}", v.detail);
    assert!(v.detail.contains("in group 00000000-0000-4000-8000-00000000000a") && v.detail.contains("in group 00000000-0000-4000-8000-00000000000b"), "{}", v.detail);
}
