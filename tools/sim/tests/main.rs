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

mod ratchet;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixture")
}

fn catalog() -> Catalog {
    Catalog::new(weft_sim::spec::load(&fixture()).expect("the fixture compiles"))
}

fn explore(cat: &Catalog, threads: usize) -> serde_json::Value {
    let cov = Coverage::new(cat, "explore", threads);
    let cands = Candidates::default();
    let config = Config {
        seed: 5,
        walks: 40,
        steps: 12,
        threads,
        max_seconds: 50,
        setup: None,
        controllers: None,
        checks: Default::default(),
    };
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
    assert_eq!(
        snap["outcomes_reached"], 4,
        "outcomes: {}",
        snap["outcome_hits"]
    );
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
        cmd: cat
            .commands
            .iter()
            .position(|c| c.name.to_string() == "demo.box.OpenBox")
            .unwrap(),
        actor: Some(0),
        caller: None,
        input: [(
            "label".to_owned(),
            ess_primitives::node::Node::Text("a".into()),
        )]
        .into(),
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
    let vars: std::collections::BTreeMap<String, serde_json::Value> = [
        ("i".to_owned(), json!(1)),
        ("node_1".to_owned(), json!("n1")),
        ("n".to_owned(), json!(3)),
    ]
    .into();
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
        cmd: cat
            .commands
            .iter()
            .position(|c| c.name.to_string() == "demo.box.OpenBox")
            .unwrap(),
        actor: Some(0),
        caller: None,
        input: [(
            "label".to_owned(),
            ess_primitives::node::Node::Text(label.into()),
        )]
        .into(),
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
        let c = cat
            .commands
            .iter()
            .find(|c| c.name.to_string() == cmd)
            .unwrap();
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
        let config = Config {
            seed: 5,
            walks: 48,
            steps: 16,
            threads: 2,
            max_seconds: 50,
            setup,
            controllers: None,
            checks: Default::default(),
        };
        explore::run(&cat, &cov, &cands, &config, None);
        let snap = cov.snapshot(&cat, true);
        let attempts = cov.inner.lock().unwrap().attempts.clone();
        let gaps = weft_sim::report::gaps(
            &cat,
            &snap,
            &weft_sim::report::Locator::new(&dir),
            &attempts,
        );
        (snap, gaps)
    };
    let never = |gaps: &serde_json::Value, cmd: &str| {
        gaps["commands_never_accepted"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["command"] == cmd)
            .cloned()
    };
    let (empty, empty_gaps) = run(false);
    let (from, from_gaps) = run(true);
    let hits = |snap: &serde_json::Value, g: usize| snap["outcome_hits"][g].as_u64().unwrap();

    assert_eq!(
        hits(&empty, gid("demo.shelf.Polish", "polished")),
        0,
        "no random label is gold"
    );
    assert!(
        hits(&from, gid("demo.shelf.Polish", "polished")) > 0,
        "the setup's gold shelf is polished"
    );
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
        assert!(
            hits(snap, gid("demo.shelf.Relabel", "relabelled")) > 0,
            "the label is copied from the shelf"
        );
        let jam = never(gaps, "demo.shelf.Jam").expect("Jam is never accepted");
        let class = jam["class"].as_str().unwrap();
        assert!(
            class.starts_with("blocked-by:") && class.contains("shelf.yaml:"),
            "{jam}"
        );
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
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );
    let bad = sim(&["simulate", "--experiment", &path("setup-refused.yaml")]);
    let stderr = String::from_utf8_lossy(&bad.stderr);
    assert_eq!(bad.status.code(), Some(3), "{stderr}");
    assert!(
        stderr.contains("polish-nothing (demo.shelf.Polish) answered no-such-shelf"),
        "{stderr}"
    );
    let bad = sim(&[
        "explore",
        "--walks",
        "2",
        "--steps",
        "2",
        "--threads",
        "1",
        "--from",
        &path("setup-refused.yaml"),
    ]);
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
    assert!(
        out.join("result.json").is_file(),
        "the cut run still writes its result"
    );
    let whole = run(&["--walks", "8", "--max-seconds", "60"]);
    assert!(
        whole.status.success(),
        "{}",
        String::from_utf8_lossy(&whole.stderr)
    );
}

#[test]
fn a_lost_binding_the_model_does_not_declare_is_refused() {
    let err = weft_sim::spec::load_without(&fixture(), &["no-such-binding".to_owned()])
        .expect_err("unknown binding");
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
        let c = cat
            .commands
            .iter()
            .find(|c| c.name.to_string() == cmd)
            .unwrap();
        c.outcomes.iter().find(|o| o.name == outcome).unwrap().gid
    };
    let run = |settle: usize, share: f64| {
        let cov = Coverage::new(&cat, "explore", 2);
        let cands = Candidates::default();
        let s = weft_sim::simulate::record_setup(&cat, &cov, &cands, &exp, settle).unwrap();
        assert!(s.refusals.is_empty(), "{:?}", s.refusals);
        let info = (s.setup_steps, s.steps.len(), s.settle_ticks, s.settled_at);
        let controllers = (share > 0.0).then(|| explore::WalkControllers {
            exp: exp.clone(),
            vars: s.vars.clone(),
            share,
        });
        let config = Config {
            seed: 5,
            walks: 32,
            steps: 16,
            threads: 2,
            max_seconds: 50,
            setup: Some(s.steps),
            controllers,
            checks: Default::default(),
        };
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
    assert_eq!(
        hits(&settled, placed),
        1,
        "the settle's placement counts once"
    );
    assert!(
        hits(&settled, finished) > 0,
        "walks start from the placed job"
    );
    assert_eq!(settled["info"]["setup_replay"]["walks_differ"], 0.0);
    assert_eq!(settled["info"]["setup_replay"]["controller_ticks"], 0.0);

    let (ticked, _) = run(4, 0.25);
    assert!(hits(&ticked, placed) > 1, "ticks place the jobs walks add");
    assert!(hits(&ticked, finished) > hits(&settled, finished));
    let r = &ticked["info"]["setup_replay"];
    assert!(
        r["controller_ticks"].as_f64().unwrap() > 0.0
            && r["controller_sends"].as_f64().unwrap() > 0.0,
        "{r}"
    );
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
    explore::Checks {
        invariants: invariants.iter().map(|s| (*s).to_owned()).collect(),
        every,
        converge,
    }
}

/// Explores `dir` from an empty store (or from `exp`'s setup) with `checks`.
fn explore_checked(
    cat: &Catalog,
    exp: Option<&experiment::Experiment>,
    checks: explore::Checks,
) -> serde_json::Value {
    let cov = Coverage::new(cat, "explore", 2);
    let cands = Candidates::default();
    let (setup, controllers) = match exp {
        Some(exp) => {
            let s =
                weft_sim::simulate::record_setup(cat, &cov, &cands, exp, checks.converge).unwrap();
            assert!(s.refusals.is_empty(), "{:?}", s.refusals);
            let controllers = explore::WalkControllers {
                exp: exp.clone(),
                vars: s.vars.clone(),
                share: 0.0,
            };
            (Some(s.steps), Some(controllers))
        }
        None => (None, None),
    };
    let config = Config {
        seed: 5,
        walks: 32,
        steps: 16,
        threads: 2,
        max_seconds: 50,
        setup,
        controllers,
        checks,
    };
    explore::run(cat, &cov, &cands, &config, None);
    cov.snapshot(cat, true)
}

fn violations<'v>(snap: &'v serde_json::Value, kind: &str) -> Vec<&'v serde_json::Value> {
    snap["violations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["kind"] == kind)
        .collect()
}

/// Replays the walk of `v` from the run's trace: no answer differs, and the replay's own checks
/// find a system invariant violation again.
fn replay_finds_it(cat: &Catalog, snap: &serde_json::Value, v: &serde_json::Value) {
    let walk = v["walk"].as_u64().unwrap();
    let trace = snap["traces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["walk"] == walk)
        .unwrap_or_else(|| panic!("no trace of walk {walk}"));
    let step = v["step"].as_u64().unwrap() as usize;
    assert!(
        trace["steps"].as_array().unwrap().len() >= step,
        "the trace reaches step {step}"
    );
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
    let view_checks = [
        "no-double-placement",
        "quota-conservation",
        "tenant-isolation",
    ];
    let guarded =
        Catalog::new(weft_sim::spec::load(&fixture_invariants()).expect("the fixture compiles"));
    let snap = explore_checked(&guarded, None, checks(&view_checks, 1, 0));
    assert_eq!(snap["violation_count"], 0, "{}", snap["violations"]);

    let cat =
        Catalog::new(weft_sim::spec::load(&unguarded()).expect("the unguarded fixture compiles"));
    let snap = explore_checked(&cat, None, checks(&view_checks, 1, 0));
    let found = violations(&snap, "no-double-placement");
    assert!(!found.is_empty(), "{}", snap["violations"]);
    assert_eq!(
        found.len(),
        snap["violation_count"].as_u64().unwrap() as usize,
        "only no-double-placement"
    );
    let v = found[0];
    assert!(
        v["detail"].as_str().unwrap().starts_with("instance "),
        "{v}"
    );
    assert!(
        v["step"].as_u64().unwrap() >= 1 && v["seed"].as_u64().is_some(),
        "{v}"
    );
    // Once per walk: the state stays broken, the walk reports its first step.
    let walks: std::collections::BTreeSet<u64> =
        found.iter().map(|v| v["walk"].as_u64().unwrap()).collect();
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
    let guarded =
        Catalog::new(weft_sim::spec::load(&fixture_invariants()).expect("the fixture compiles"));
    let assign = guarded
        .commands
        .iter()
        .position(|c| c.name.to_string() == "weft.nodes.Assign")
        .unwrap();
    assert_eq!(
        guarded.redelivered.iter().copied().collect::<Vec<_>>(),
        vec![assign]
    );
    let snap = explore_checked(&guarded, None, checks(&["idempotency"], 1, 0));
    assert_eq!(snap["violation_count"], 0, "{}", snap["violations"]);
    assert!(
        snap["info"]["checks"]["duplicates"].as_f64().unwrap() > 0.0,
        "{}",
        snap["info"]
    );

    let cat =
        Catalog::new(weft_sim::spec::load(&unguarded()).expect("the unguarded fixture compiles"));
    let snap = explore_checked(&cat, None, checks(&["idempotency"], 1, 0));
    let found = violations(&snap, "idempotency");
    assert!(!found.is_empty(), "{}", snap["violations"]);
    assert_eq!(
        found.len(),
        snap["violation_count"].as_u64().unwrap() as usize
    );
    let v = found[0];
    assert_eq!(v["command"], "weft.nodes.Assign");
    assert!(
        v["detail"]
            .as_str()
            .unwrap()
            .contains("weft.nodes.Assignments: "),
        "{v}"
    );
    let walk = v["walk"].as_u64().unwrap();
    let trace = snap["traces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["walk"] == walk)
        .unwrap();
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
    let setup =
        weft_sim::simulate::record_setup(&cat, &cov, &Candidates::default(), &loops, 4).unwrap();
    assert_eq!(setup.settled_at, None);
    assert_eq!(
        setup.violations.len(),
        1,
        "the setup itself does not settle"
    );
    assert_eq!(setup.violations[0].kind, "no-convergence");
    let snap = explore_checked(&cat, Some(&loops), checks(&none, 0, 4));
    let found = violations(&snap, "no-convergence");
    assert!(!found.is_empty());
    assert_eq!(
        found.len(),
        snap["violation_count"].as_u64().unwrap() as usize
    );
    let v = found[0];
    assert!(
        v["detail"].as_str().unwrap().starts_with(
            "the controllers still had work after 4 ticks with no new input: reassigner "
        ),
        "{v}"
    );
    // The convergence ticks are in the walk's trace (`by: converge`), so a replay sends them.
    let walk = v["walk"].as_u64().unwrap();
    let trace = snap["traces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["walk"] == walk)
        .unwrap();
    let ticks = trace["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["by"] == "converge")
        .count();
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
    assert!(
        refusals[0].starts_with(
            "f-in-a-again (weft.scheduling.PlaceInstance) answered crosses-closed-link"
        ),
        "{refusals:?}"
    );
    assert!(violations.is_empty(), "{violations:?}");
}

/// closed-link fires without the guard: the last placement puts formation f in group a beside its
/// live placement in b, which a closed link joins; one violation, at that step.
#[test]
fn closed_link_fires_on_a_formation_placed_across_a_closed_link() {
    let dir = without_guard(
        &fixture_closed_link(),
        "scheduling.yaml",
        "crosses-closed-link",
    );
    let (refusals, violations) = closed_link_setup(&dir);
    assert!(refusals.is_empty(), "{refusals:?}");
    assert_eq!(violations.len(), 1, "{violations:?}");
    let v = &violations[0];
    assert_eq!(v.kind, "closed-link");
    assert_eq!(v.step, 8, "{v:?}");
    assert!(
        v.detail
            .starts_with("formation 00000000-0000-4000-8000-0000000000f1: placements "),
        "{}",
        v.detail
    );
    assert!(
        v.detail
            .contains("in group 00000000-0000-4000-8000-00000000000a")
            && v.detail
                .contains("in group 00000000-0000-4000-8000-00000000000b"),
        "{}",
        v.detail
    );
}

// ---- search output: the climb, row by row ---------------------------------------------------

fn fixture_search() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixture-search")
}

/// The search fixture's experiment with each `(from, to)` text replaced, written under the test
/// target directory as `<name>.yaml`.
fn search_experiment(name: &str, edits: &[(&str, &str)]) -> PathBuf {
    let mut text = std::fs::read_to_string(fixture_search().join("search.yaml")).unwrap();
    for (from, to) in edits {
        assert!(text.contains(from), "the fixture has `{from}`");
        text = text.replace(from, to);
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("search");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.yaml"));
    std::fs::write(&path, text).unwrap();
    path
}

fn search_command(name: &str, edits: &[(&str, &str)]) -> (std::process::Command, PathBuf) {
    let exp = search_experiment(name, edits);
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("search")
        .join(name);
    let _ = std::fs::remove_dir_all(&out);
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_weft-sim"));
    cmd.arg("simulate")
        .arg("--spec")
        .arg(fixture_search())
        .arg("--experiment")
        .arg(&exp)
        .arg("--out")
        .arg(&out)
        .args(["--threads", "2"]);
    (cmd, out)
}

/// `weft-sim simulate` on the search fixture: the rows of `search` in its `result.json`, and
/// the whole file.
fn search_rows(name: &str, edits: &[(&str, &str)]) -> (Vec<serde_json::Value>, serde_json::Value) {
    let (mut cmd, out) = search_command(name, edits);
    let run = cmd.output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("result.json")).unwrap()).unwrap();
    let rows = result["search"].as_array().expect("search rows").clone();
    assert!(!rows.is_empty());
    (rows, result)
}

/// The rows of a fit run; `test` names its output directory (tests run in parallel).
fn fit_rows(test: &str) -> Vec<serde_json::Value> {
    search_rows(&format!("{test}-fit"), &[]).0
}

fn exhaustive_rows(test: &str) -> Vec<serde_json::Value> {
    search_rows(
        &format!("{test}-exhaustive"),
        &[("mode: fit", "mode: exhaustive")],
    )
    .0
}

fn index_of(row: &serde_json::Value) -> Vec<u64> {
    row["index"]
        .as_array()
        .unwrap_or_else(|| panic!("index: {row}"))
        .iter()
        .map(|i| i.as_u64().unwrap())
        .collect()
}

/// Rows grouped by `round`, in order.
fn rounds(rows: &[serde_json::Value]) -> Vec<Vec<&serde_json::Value>> {
    let mut out: Vec<Vec<&serde_json::Value>> = Vec::new();
    for row in rows {
        let round = row["round"]
            .as_u64()
            .unwrap_or_else(|| panic!("round: {row}"));
        match out.last_mut() {
            Some(last) if last[0]["round"].as_u64() == Some(round) => last.push(row),
            _ => {
                assert_eq!(
                    round as usize,
                    out.len() + 1,
                    "rounds count from 1 in order: {row}"
                );
                out.push(vec![row]);
            }
        }
    }
    out
}

/// Acceptance 1: every row of a fit and an exhaustive run carries the climb fields beside the
/// old ones.
#[test]
fn every_search_row_carries_the_climb_fields() {
    for (mode, rows) in [
        ("fit", fit_rows("fields")),
        ("exhaustive", exhaustive_rows("fields")),
    ] {
        for (i, row) in rows.iter().enumerate() {
            for key in [
                "point", "kpis", "fitness", "feasible", "seq", "round", "index", "parent",
                "decision", "best", "walk",
            ] {
                assert!(
                    row.get(key).is_some(),
                    "{mode} row {i} lacks `{key}`: {row}"
                );
            }
            assert_eq!(row["seq"], i + 1, "{mode}: {row}");
            assert_eq!(index_of(row).len(), 2, "one index per mutation axis: {row}");
            let decision = row["decision"].as_str().unwrap();
            assert!(
                ["start", "accepted", "rejected", "restart"].contains(&decision),
                "{mode}: {row}"
            );
        }
        assert_eq!(
            rows.len(),
            16,
            "{mode}: the fixture has 16 points and the budget covers them"
        );
    }
    let exhaustive = exhaustive_rows("fields-rounds");
    assert!(exhaustive
        .iter()
        .all(|r| r["round"] == 1 && r["parent"].is_null()));
}

/// Acceptance 2: the climb starts once, accepts at most one neighbour a round, and every
/// neighbour names the current point of its round, one step away on one axis.
#[test]
fn a_fit_climb_names_its_parent_and_accepts_at_most_one_neighbour_a_round() {
    let rows = fit_rows("parent");
    assert_eq!(rows[0]["decision"], "start");
    assert!(rows[0]["parent"].is_null());
    assert_eq!(rows.iter().filter(|r| r["decision"] == "start").count(), 1);
    let by_seq = |seq: u64| &rows[seq as usize - 1];
    let mut current = rows[0]["seq"].as_u64().unwrap();
    let mut accepted = 0;
    for round in rounds(&rows).into_iter().skip(1) {
        if round[0]["decision"] == "restart" {
            assert_eq!(round.len(), 1, "a restart is a round of its own: {round:?}");
            current = round[0]["seq"].as_u64().unwrap();
            continue;
        }
        let taken: Vec<_> = round
            .iter()
            .filter(|r| r["decision"] == "accepted")
            .collect();
        assert!(taken.len() <= 1, "{round:?}");
        for row in &round {
            assert!(
                row["decision"] == "accepted" || row["decision"] == "rejected",
                "{row}"
            );
            assert_eq!(row["parent"], current, "{row}");
            let (a, b) = (index_of(row), index_of(by_seq(current)));
            let steps: Vec<u64> = a.iter().zip(&b).map(|(x, y)| x.abs_diff(*y)).collect();
            assert_eq!(steps.iter().filter(|d| **d == 1).count(), 1, "{row}");
            assert_eq!(steps.iter().sum::<u64>(), 1, "{row}");
        }
        if let Some(row) = taken.first() {
            accepted += 1;
            current = row["seq"].as_u64().unwrap();
        }
    }
    assert!(accepted >= 3, "the fixture's climb takes steps: {accepted}");
    assert!(rows.iter().any(|r| r["decision"] == "rejected"));
}

/// Acceptance 3: after a round with no improvement the next row is an evaluated restart at a
/// point no earlier row has.
#[test]
fn a_round_without_improvement_is_followed_by_an_evaluated_restart() {
    let rows = fit_rows("restart");
    for (i, row) in rows.iter().enumerate() {
        if row["decision"] == "restart" {
            assert!(row["parent"].is_null(), "{row}");
            assert!(
                row["kpis"]["steps"].as_u64().is_some_and(|s| s > 0),
                "its own KPIs: {row}"
            );
            assert!(row["fitness"].is_number(), "{row}");
            assert!(
                rows[..i].iter().all(|r| index_of(r) != index_of(row)),
                "a point not visited: {row}"
            );
        }
    }
    let rounds = rounds(&rows);
    let mut checked = 0;
    for (r, round) in rounds.iter().enumerate().skip(1) {
        if round.iter().all(|row| row["decision"] == "rejected") {
            if let Some(next) = rounds.get(r + 1) {
                assert_eq!(
                    next[0]["decision"],
                    "restart",
                    "after round {}: {}",
                    r + 1,
                    next[0]
                );
                checked += 1;
            }
        }
    }
    assert!(
        checked >= 1,
        "the fixture's climb finds a round without improvement"
    );
}

/// Acceptance 4: a budget smaller than the points bounds the rows, restarts counted.
#[test]
fn a_fit_budget_counts_its_restarts() {
    let (rows, _) = search_rows("fit-budget", &[("budget: 24", "budget: 15")]);
    assert!(
        rows.iter().any(|r| r["decision"] == "restart"),
        "the run restarts"
    );
    assert!(rows.len() <= 15, "{} rows", rows.len());
}

/// Acceptance 5: `best` is the largest feasible fitness up to and including the row, null
/// while no row is feasible.
#[test]
fn best_is_the_largest_feasible_fitness_so_far() {
    for (mode, rows) in [
        ("fit", fit_rows("best")),
        ("exhaustive", exhaustive_rows("best")),
    ] {
        assert_eq!(
            rows[0]["feasible"], false,
            "{mode}: the fixture starts infeasible"
        );
        let mut best: Option<f64> = None;
        for row in &rows {
            if row["feasible"] == true {
                let f = row["fitness"].as_f64().unwrap();
                best = Some(best.map_or(f, |b| b.max(f)));
            }
            match best {
                None => assert!(row["best"].is_null(), "{mode}: {row}"),
                Some(b) => assert_eq!(row["best"].as_f64(), Some(b), "{mode}: {row}"),
            }
        }
        assert!(best.is_some(), "{mode}: some point is feasible");
    }
}

/// Acceptance 6: walk numbers are distinct and never 0 (walk 0 is the best point's rerun).
#[test]
fn fit_walk_numbers_are_distinct_and_never_zero() {
    let rows = fit_rows("walks");
    let walks: Vec<u64> = rows
        .iter()
        .map(|r| r["walk"].as_u64().unwrap_or_else(|| panic!("walk: {r}")))
        .collect();
    assert!(!walks.contains(&0));
    let distinct: std::collections::BTreeSet<_> = walks.iter().collect();
    assert_eq!(distinct.len(), walks.len(), "{walks:?}");
}

fn http_get(port: u16, path: &str) -> Option<String> {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .ok()?;
    write!(
        stream,
        "GET {path} HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut text = String::new();
    stream.read_to_string(&mut text).ok()?;
    let (head, body) = text.split_once("\r\n\r\n")?;
    (head.split(' ').nth(1) == Some("200")).then(|| body.to_owned())
}

/// Acceptance 7 and 8: with `--serve` the snapshot holds `info.search` before the first row is
/// recorded (the search holds until the test has read it), and the written `result.json`
/// carries the same object.
#[test]
fn the_search_header_is_live_before_the_first_row_and_written_with_the_result() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("search");
    std::fs::create_dir_all(&dir).unwrap();
    let hold = dir.join("serve.hold");
    let _ = std::fs::remove_file(&hold);
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let (mut cmd, out) = search_command("serve", &[]);
    let log = dir.join("serve.stderr");
    cmd.args(["--serve", &format!("127.0.0.1:{port}"), "--exit-when-done"])
        .env("WEFT_SIM_SEARCH_HOLD", &hold)
        .stdout(std::process::Stdio::null())
        .stderr(std::fs::File::create(&log).unwrap());
    let mut child = cmd.spawn().unwrap();
    let started = std::time::Instant::now();
    let snap = loop {
        if let Some(body) = http_get(port, "/snapshot") {
            let snap: serde_json::Value = serde_json::from_str(&body).unwrap();
            if snap["info"].get("search").is_some() {
                break snap;
            }
        }
        if started.elapsed().as_secs() > 20 {
            let _ = child.kill();
            panic!(
                "no snapshot with info.search: {}",
                std::fs::read_to_string(&log).unwrap_or_default()
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    std::fs::write(&hold, "").unwrap();
    assert_eq!(
        snap["search"],
        serde_json::json!([]),
        "read before the first row"
    );
    let expected = serde_json::json!({
        "mode": "fit",
        "seed": 7,
        "budget": 24,
        "limit": 16,
        "weights": {"steps": 1.0},
        "gates": ["setup-failures"],
        "axes": [
            {"name": "opens", "kind": "var", "point": "opens", "values": [1, 2, 3, 4]},
            {"name": "closes", "kind": "var", "point": "closes", "values": [2, 0, 1, 3]},
        ],
    });
    assert_eq!(snap["info"]["search"], expected);
    let status = child.wait().unwrap();
    assert!(
        status.success(),
        "{}",
        std::fs::read_to_string(&log).unwrap_or_default()
    );
    let result: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("result.json")).unwrap()).unwrap();
    assert_eq!(result["info"]["search"], snap["info"]["search"]);
    assert_eq!(result["search"].as_array().map(Vec::len), Some(16));
}

// ---- search output: adversarial cases ---------------------------------------------------------

/// `info.search.budget` is documented as "the most points a `fit` search evaluates": a budget of
/// 0 evaluates no point (or the run refuses the budget); it does not record the start anyway.
#[test]
fn a_fit_budget_of_zero_records_no_row() {
    let (mut cmd, out) = search_command("adv-budget-zero", &[("budget: 24", "budget: 0")]);
    let run = cmd.output().unwrap();
    let stderr = String::from_utf8_lossy(&run.stderr);
    if !run.status.success() {
        assert!(
            stderr.contains("budget"),
            "a refusal names the budget: {stderr}"
        );
        return;
    }
    let result: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("result.json")).unwrap()).unwrap();
    let rows = result["search"].as_array().expect("search rows");
    assert_eq!(rows.len(), 0, "budget 0 recorded rows: {rows:?}");
}

/// The test hook holds only while it names a file: an empty `WEFT_SIM_SEARCH_HOLD` names none and
/// must not stall the search for 30 s.
#[test]
fn an_empty_search_hold_variable_does_not_hold_the_search() {
    let (mut cmd, _out) = search_command("adv-hold-empty", &[]);
    cmd.env("WEFT_SIM_SEARCH_HOLD", "")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let mut child = cmd.spawn().unwrap();
    let started = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if started.elapsed().as_secs() >= 10 {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "the search still runs after {:?} with an empty WEFT_SIM_SEARCH_HOLD",
                started.elapsed()
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Acceptance 1 and the docs: `index` is, per axis in the order of `info.search.axes`, the index
/// of the row's value in that axis's `values`.
#[test]
fn every_row_index_names_its_point_through_the_axes() {
    for name in ["adv-index-fit", "adv-index-exhaustive"] {
        let edits: &[(&str, &str)] = if name.ends_with("exhaustive") {
            &[("mode: fit", "mode: exhaustive")]
        } else {
            &[]
        };
        let (rows, result) = search_rows(name, edits);
        let axes = result["info"]["search"]["axes"]
            .as_array()
            .expect("info.search.axes");
        for row in &rows {
            let index = index_of(row);
            assert_eq!(index.len(), axes.len(), "{row}");
            for (axis, i) in axes.iter().zip(&index) {
                let value = &axis["values"][*i as usize];
                assert_eq!(
                    &row["point"][axis["name"].as_str().unwrap()],
                    value,
                    "{name}: axis {} index {i}: {row}",
                    axis["name"]
                );
            }
        }
    }
}

/// Acceptance 2 and the docs: the accepted neighbour becomes the current point, so the next round
/// is its neighbours, unless it has no unvisited neighbour left (then a restart).
#[test]
fn the_round_after_an_accepted_neighbour_climbs_from_it() {
    let (rows, result) = search_rows("adv-accepted-next", &[]);
    let lens: Vec<u64> = result["info"]["search"]["axes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["values"].as_array().unwrap().len() as u64)
        .collect();
    let rounds = rounds(&rows);
    let mut checked = 0;
    for (r, round) in rounds.iter().enumerate() {
        let Some(accepted) = round.iter().find(|row| row["decision"] == "accepted") else {
            continue;
        };
        let Some(next) = rounds.get(r + 1) else {
            continue;
        };
        let at = index_of(accepted);
        let visited: Vec<Vec<u64>> = rounds[..=r]
            .iter()
            .flatten()
            .map(|row| index_of(row))
            .collect();
        let mut open = 0;
        for axis in 0..at.len() {
            for up in [false, true] {
                let mut n = at.clone();
                if up && n[axis] + 1 < lens[axis] {
                    n[axis] += 1;
                } else if !up && n[axis] > 0 {
                    n[axis] -= 1;
                } else {
                    continue;
                }
                if !visited.contains(&n) {
                    open += 1;
                }
            }
        }
        if open > 0 {
            assert_ne!(
                next[0]["decision"], "restart",
                "seq {} accepted with {open} unvisited neighbours: {}",
                accepted["seq"], next[0]
            );
            assert!(
                next.iter().all(|row| row["parent"] == accepted["seq"]),
                "{next:?}"
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 3,
        "the fixture's climb continues from accepted points: {checked}"
    );
}

/// Docs: in an `exhaustive` run a row after the first is `accepted` exactly when it raised the
/// best feasible fitness.
#[test]
fn an_exhaustive_row_is_accepted_exactly_when_it_raises_the_best() {
    let (rows, _) = search_rows(
        "adv-exhaustive-decision",
        &[("mode: fit", "mode: exhaustive")],
    );
    assert_eq!(rows[0]["decision"], "start");
    let mut accepted = 0;
    for pair in rows.windows(2) {
        let (before, row) = (&pair[0], &pair[1]);
        let raised = row["best"]
            .as_f64()
            .is_some_and(|b| before["best"].as_f64().is_none_or(|p| b > p));
        let expected = if raised { "accepted" } else { "rejected" };
        assert_eq!(row["decision"], expected, "{row}");
        accepted += usize::from(raised);
    }
    assert!(
        accepted >= 2,
        "the fixture's exhaustive run raises the best: {accepted}"
    );
}

/// The search rows, `info.search` and the best point depend on the seed, not on `--threads`.
#[test]
fn search_rows_do_not_depend_on_threads() {
    let exp = search_experiment("adv-threads", &[]);
    let mut seen = Vec::new();
    for threads in ["1", "8"] {
        let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("search")
            .join(format!("adv-threads-{threads}"));
        let _ = std::fs::remove_dir_all(&out);
        let run = std::process::Command::new(env!("CARGO_BIN_EXE_weft-sim"))
            .arg("simulate")
            .arg("--spec")
            .arg(fixture_search())
            .arg("--experiment")
            .arg(&exp)
            .arg("--out")
            .arg(&out)
            .args(["--threads", threads])
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(out.join("result.json")).unwrap())
                .unwrap();
        seen.push((
            result["search"].clone(),
            result["info"]["search"].clone(),
            result["run"]["result"]["best"].clone(),
        ));
    }
    assert_eq!(seen[0], seen[1]);
}

// ---- replay of a search point ---------------------------------------------------------------

/// `weft-sim` run in this crate's directory with relative paths, as the page's replay command
/// names them.
fn sim_here(args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_weft-sim"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(args)
        .output()
        .unwrap()
}

/// A fit run of the search fixture, with relative paths, written to `target/tmp/replay/<name>`:
/// that directory, relative to this crate, and its `result.json`.
fn relative_fit_run(name: &str) -> (String, serde_json::Value) {
    let out = format!("target/tmp/replay/{name}");
    let _ = std::fs::remove_dir_all(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&out));
    let run = sim_here(&[
        "simulate",
        "--spec",
        "tests/fixture-search",
        "--experiment",
        "tests/fixture-search/search.yaml",
        "--out",
        &out,
        "--threads",
        "2",
    ]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(&out)
            .join("result.json"),
    )
    .unwrap();
    (out, serde_json::from_str(&text).unwrap())
}

fn read_result(dir: &str) -> serde_json::Value {
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(dir)
            .join("result.json"),
    )
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

/// Acceptance 1 and 2: `replay --walk K` on a `simulate` result reruns the search row of walk K
/// (a rejected neighbour and a restart), prints each KPI of the rerun beside the recorded one,
/// exits 0 when all are equal, and writes a result and a page whose timeline is that point's run.
#[test]
fn a_search_point_replays_at_its_walk_with_the_kpis_it_recorded() {
    let (out, result) = relative_fit_run("fit");
    assert_eq!(
        result["run"]["spec"], "tests/fixture-search",
        "the run names its spec as given"
    );
    assert_eq!(
        result["run"]["out"],
        out.as_str(),
        "the run names its output as given"
    );
    let rows = result["search"].as_array().unwrap();
    for decision in ["rejected", "restart"] {
        let row = rows
            .iter()
            .find(|r| r["decision"] == decision && r["walk"] != r["seq"])
            .or_else(|| rows.iter().find(|r| r["decision"] == decision))
            .unwrap();
        let walk = row["walk"].as_u64().unwrap().to_string();
        let to = format!("{out}-walk-{walk}");
        let trace = format!("{out}/result.json");
        let run = sim_here(&[
            "replay",
            "--spec",
            "tests/fixture-search",
            "--trace",
            &trace,
            "--walk",
            &walk,
            "--out",
            &to,
        ]);
        let stdout = String::from_utf8_lossy(&run.stdout);
        let stderr = String::from_utf8_lossy(&run.stderr);
        assert_eq!(
            run.status.code(),
            Some(0),
            "{decision} walk {walk}: {stdout}{stderr}"
        );
        let kpis = row["kpis"].as_object().unwrap();
        assert!(!kpis.is_empty());
        for (key, value) in kpis {
            let line = format!("  {key}: {value} (recorded {value})");
            assert!(
                stdout.lines().any(|l| l == line),
                "{decision}: no line `{line}` in\n{stdout}"
            );
        }
        assert!(
            stdout.contains(&format!("walk {walk}: 0 of {} KPIs differ", kpis.len())),
            "{stdout}"
        );

        let replayed = read_result(&to);
        assert_eq!(
            replayed["run"]["replay"]["walk"].as_u64(),
            row["walk"].as_u64()
        );
        assert_eq!(replayed["run"]["replay"]["point"], row["point"]);
        assert_eq!(replayed["run"]["result"]["kpis"], row["kpis"]);
        // The timeline of that point: its setup sends `opens` opens and `closes` closes.
        let timeline = replayed["timeline"].as_array().expect("a timeline");
        let sends =
            row["point"]["opens"].as_u64().unwrap() + row["point"]["closes"].as_u64().unwrap();
        assert_eq!(
            timeline.iter().filter(|t| t["phase"] == "setup").count() as u64,
            sends,
            "{timeline:?}"
        );
        let page = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(&to)
                .join("index.html"),
        )
        .unwrap();
        assert!(page.contains(r#"<section id="timeline-box""#));
        assert_eq!(
            inlined(&page, "SNAP")["timeline"],
            replayed["timeline"],
            "the page shows that timeline"
        );
    }
}

/// Acceptance 3, and a rerun that differs: a walk no search row and no trace holds is refused,
/// naming the walk and the file, with exit status 2; a recorded KPI that the rerun does not meet
/// is printed as differing and the replay exits 2. The `explore` path of `replay` is unchanged.
#[test]
fn replay_refuses_an_unknown_walk_and_fails_on_a_kpi_that_differs() {
    let (out, result) = relative_fit_run("refuse");
    let trace = format!("{out}/result.json");
    let unknown = sim_here(&[
        "replay",
        "--spec",
        "tests/fixture-search",
        "--trace",
        &trace,
        "--walk",
        "999",
    ]);
    let stderr = String::from_utf8_lossy(&unknown.stderr);
    assert_eq!(unknown.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("walk 999") && stderr.contains(&trace),
        "{stderr}"
    );

    let mut tampered = result.clone();
    let row = tampered["search"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["decision"] == "rejected")
        .unwrap();
    let walk = row["walk"].as_u64().unwrap().to_string();
    let steps = row["kpis"]["steps"].as_u64().unwrap();
    row["kpis"]["steps"] = serde_json::json!(steps + 1);
    let file = format!("{out}/tampered.json");
    std::fs::write(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&file),
        tampered.to_string(),
    )
    .unwrap();
    let run = sim_here(&[
        "replay",
        "--spec",
        "tests/fixture-search",
        "--trace",
        &file,
        "--walk",
        &walk,
        "--out",
        &format!("{out}-tampered"),
    ]);
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert_eq!(run.status.code(), Some(2), "{stdout}");
    assert!(
        stdout
            .lines()
            .any(|l| l == format!("  steps: {steps} (recorded {}) differs", steps + 1)),
        "{stdout}"
    );
    assert!(stdout.contains(&format!("walk {walk}: 1 of ")), "{stdout}");

    let explore = format!("{out}-explore");
    let run = sim_here(&[
        "explore",
        "--spec",
        "tests/fixture",
        "--walks",
        "2",
        "--steps",
        "3",
        "--threads",
        "1",
        "--out",
        &explore,
    ]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    // The first walk the explore result keeps a trace of.
    let traced = read_result(&explore)["traces"][0]["walk"]
        .as_u64()
        .expect("a traced walk")
        .to_string();
    let replay = sim_here(&[
        "replay",
        "--spec",
        "tests/fixture",
        "--trace",
        &format!("{explore}/result.json"),
        "--walk",
        &traced,
    ]);
    let stdout = String::from_utf8_lossy(&replay.stdout);
    assert_eq!(
        replay.status.code(),
        Some(0),
        "{stdout}{}",
        String::from_utf8_lossy(&replay.stderr)
    );
    assert!(stdout.contains("0 answers differ"), "{stdout}");
}

// ---- the page's search player -----------------------------------------------------------------

/// The command that writes the fixture pages again (`website/ux/fixtures/sim-player*.html`), named by
/// the tests that compare them; the browser checks of the player read them.
const WRITE_PLAYER_FIXTURE: &str =
    "cd tools/sim && WEFT_SIM_WRITE_PLAYER_FIXTURE=1 cargo nextest run the_player_page";

/// The snapshot the fixture page inlines: the counters, hits and search of a fit run of the search
/// fixture, and the spec and output paths it was given (relative). Times, the command line and the
/// file paths of the gaps vary by host and run, so they are left out.
fn player_snapshot(result: &serde_json::Value) -> serde_json::Value {
    let mut snap = serde_json::Map::new();
    for key in [
        "mode",
        "running",
        "walks",
        "steps",
        "threads",
        "outcomes_declared",
        "outcomes_reached",
        "states_declared",
        "states_reached",
        "transitions_declared",
        "transitions_reached",
        "abstract_states",
        "branch_points",
        "violation_count",
        "violations",
        "outcome_hits",
        "state_hits",
        "transition_hits",
        "search",
    ] {
        snap.insert(key.to_owned(), result[key].clone());
    }
    snap.insert("info".to_owned(), serde_json::json!({"experiment": result["info"]["experiment"], "search": result["info"]["search"]}));
    // The paths the run was given, relative to this crate: the page names them in its replay command.
    snap.insert(
        "run".to_owned(),
        serde_json::json!({"spec": result["run"]["spec"], "out": result["run"]["out"]}),
    );
    serde_json::Value::Object(snap)
}

/// The page of the fixture search's fit run, rendered from `snap`, with the fixture's model.
fn player_page(snap: &serde_json::Value) -> String {
    let cat = Catalog::new(weft_sim::spec::load(&fixture_search()).expect("the fixture compiles"));
    weft_sim::report::page(&cat.model_json(), snap, false)
}

/// The value a page inlines on its line `let <name> = …;`, read back as JSON.
fn inlined(page: &str, name: &str) -> serde_json::Value {
    let prefix = format!("let {name} = ");
    let line = page
        .lines()
        .find_map(|l| {
            l.strip_prefix(&prefix)
                .or_else(|| l.strip_prefix(&format!("const {name} = ")))
        })
        .unwrap_or_else(|| panic!("no line inlining {name}"));
    serde_json::from_str(line.strip_suffix(';').expect("ends with ;"))
        .expect("the inlined value is JSON")
}

/// Writes `page` to the committed fixture `name` under `website/ux/fixtures/` when asked, else
/// compares it byte for byte.
fn hold_fixture(name: &str, page: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../website/ux/fixtures")
        .join(name);
    if std::env::var_os("WEFT_SIM_WRITE_PLAYER_FIXTURE").is_some() {
        std::fs::write(&path, page).unwrap();
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    if committed != page {
        let at = committed
            .bytes()
            .zip(page.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        panic!(
            "website/ux/fixtures/{name} differs from the page rendered from the fixture search at byte {at} ({} bytes committed, {} rendered); write it again with: {WRITE_PLAYER_FIXTURE}",
            committed.len(),
            page.len()
        );
    }
}

/// Acceptance 11 and 14 of the player: the page rendered from the fixture search holds the player
/// and the inlined rows, equals the committed fixture page byte for byte, and its live mode passes
/// each snapshot of `/events` to `window.weftPlayer.receive`.
#[test]
fn the_player_page_of_the_fixture_search_is_the_committed_fixture() {
    let (_, result) = relative_fit_run("player-page");
    let rows = result["search"].as_array().unwrap().clone();
    assert!(
        rows.iter().any(|r| r["decision"] == "restart"),
        "the fixture run holds a restart"
    );
    let snap = player_snapshot(&result);
    let page = player_page(&snap);
    assert!(
        page.contains(r#"<div id="player""#),
        "the page has the player"
    );
    assert!(
        page.contains("data-replay-command"),
        "the player names the replay of the selected point"
    );
    assert_eq!(snap["run"]["spec"], "tests/fixture-search");
    assert_eq!(
        inlined(&page, "SNAP"),
        snap,
        "the page inlines the snapshot and its rows"
    );
    assert!(
        !page.contains(env!("CARGO_MANIFEST_DIR")),
        "no path of this host"
    );

    let cat = Catalog::new(weft_sim::spec::load(&fixture_search()).expect("the fixture compiles"));
    let live = weft_sim::report::page(&cat.model_json(), &serde_json::Value::Null, true);
    assert!(live.contains("const LIVE = true;"));
    assert!(
        live.contains("source.onmessage = (ev) => { const s = JSON.parse(ev.data); window.weftPlayer.receive(s);"),
        "the live page passes each snapshot of /events to the player"
    );
    hold_fixture("sim-player.html", &page);
}

/// Markup in a recorded value. The text a KPI or an axis value carries goes into the page's
/// script: `<!--<script` there would put the HTML parser into the double-escaped script state, and
/// the page's own `</script>` would no longer end the script. The inlined values hold no comment
/// opener or script tag, read back unchanged, and the committed page
/// `website/ux/fixtures/sim-player-markup.html` shows them as text (`sim-player.spec.ts`).
#[test]
fn the_player_page_inlines_markup_in_recorded_values_as_data() {
    const AXIS_VALUE: &str = "<!--<script>";
    const KPI: &str = "</script><!--<script>";
    let (_, result) = relative_fit_run("player-markup");
    let mut snap = player_snapshot(&result);
    snap["info"]["search"]["axes"][0]["values"][3] = serde_json::json!(AXIS_VALUE);
    for row in snap["search"].as_array_mut().unwrap() {
        if row["index"][0] == 3 {
            row["point"]["opens"] = serde_json::json!(AXIS_VALUE);
        }
        row["kpis"]["chain_stop"] = serde_json::json!(KPI);
    }
    let page = player_page(&snap);
    hold_fixture("sim-player-markup.html", &page);
    for name in ["MODEL", "SNAP"] {
        let line = page
            .lines()
            .find(|l| {
                l.starts_with(&format!("let {name} = "))
                    || l.starts_with(&format!("const {name} = "))
            })
            .unwrap()
            .to_ascii_lowercase();
        for opener in ["<!--", "<script", "</script"] {
            assert!(!line.contains(opener), "{name} inlines `{opener}`");
        }
    }
    assert_eq!(
        inlined(&page, "SNAP"),
        snap,
        "the values read back unchanged"
    );
}

/// A weight that is not a finite number makes every fitness infinite or NaN, so no climb step
/// compares: the experiment is refused at load, naming the weight. A finite weight loads.
#[test]
fn a_search_weight_that_is_not_finite_is_refused_at_load() {
    for (name, weight) in [("inf", ".inf"), ("minus-inf", "-.inf"), ("nan", ".nan")] {
        let path = search_experiment(
            &format!("weight-{name}"),
            &[(
                "weights: {steps: 1}",
                &format!("weights: {{steps: {weight}}}"),
            )],
        );
        let err = experiment::load(&path).expect_err(weight);
        assert!(
            err.contains("weight") && err.contains("`steps`"),
            "{weight}: {err}"
        );
    }
    let path = search_experiment(
        "weight-finite",
        &[("weights: {steps: 1}", "weights: {steps: -2.5}")],
    );
    assert!(experiment::load(&path).is_ok());
}
