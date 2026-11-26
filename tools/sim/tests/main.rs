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
    let config = Config { seed: 5, walks: 40, steps: 12, threads, max_seconds: 50, setup: None, controllers: None };
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
        let config = Config { seed: 5, walks: 48, steps: 16, threads: 2, max_seconds: 50, setup, controllers: None };
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
        let config = Config { seed: 5, walks: 32, steps: 16, threads: 2, max_seconds: 50, setup: Some(s.steps), controllers };
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
