//! The three goldens are accepted, with the digests cloudspec wrote, recomputed here.

use serde_json::{json, Value};
use weft_cloudspec::{canonical, digest, Submission};

use crate::golden;

/// `sha256:` of the canonical JSON `null`: the binding digest of a submission without a binding.
const NO_BINDING: &str = "sha256:74234e98afe7498fb5daf1f36ac2d78acc339464f950703b8c019892f982b90b";

fn accepted(name: &str) -> Submission {
    Submission::read(&golden(name)).unwrap_or_else(|r| panic!("{name} is refused: {r:?}"))
}

/// The counts of the last coverage step (the `generate weft` step).
fn weft_counts(s: &Submission) -> &Value {
    let steps = s.coverage()["steps"].as_array().expect("coverage.steps");
    let last = steps.last().expect("a coverage step");
    assert_eq!(last["step"], "generate weft");
    &last["counts"]
}

#[test]
fn snippet_is_accepted_with_the_digests_cloudspec_wrote() {
    let s = accepted("snippet");
    assert_eq!(
        s.architecture_digest(),
        "sha256:3ce513003eec18301050ec62932fdfdc45a7b4193d5e3b43c82a5a1cfa5c948f"
    );
    assert_eq!(s.binding_digest(), NO_BINDING);
    assert_eq!(
        digest(canonical(s.architecture()).as_bytes()),
        s.architecture_digest()
    );
    assert_eq!(s.binding(), None);
    assert_eq!(s.architecture()["architecture"], "hello");
    assert_eq!(
        weft_counts(&s),
        &json!({"mapped": 4, "mapped_with_loss": 0, "refused": 0})
    );
    assert_eq!(
        s.source_digest(),
        "sha256:d58f6b6cf43d2f0bb672a11c68c7bb51c0b848dbddf2b9ff999ee08567dfaead"
    );
}

#[test]
fn workstation_is_accepted_with_its_split_binding() {
    let s = accepted("workstation");
    assert_eq!(
        s.architecture_digest(),
        "sha256:436cf55bec6932566fc300e0941db8d14e9ecc4a8c8dc241f1d2a39c6484c551"
    );
    assert_eq!(
        s.binding_digest(),
        "sha256:975ed79dba0dfc05d83f4681faf90a15aad653a2ef4dc0b329033af751485e5f"
    );
    let binding = s.binding().expect("workstation has a binding");
    assert_eq!(binding["binding"], "split");
    assert_eq!(binding["architecture"], "workstation");
    assert_eq!(s.architecture()["architecture"], "workstation");
    let roles: Vec<&String> = binding["roles"]
        .as_object()
        .expect("binding.roles")
        .keys()
        .collect();
    assert_eq!(roles, ["agent-runtime"]);
    let parts = s.architecture()["parts"].as_object().expect("parts");
    let mut naming: Vec<&str> = parts
        .iter()
        .filter(|(_, p)| p["package"]["runtime"]["role"] == "agent-runtime")
        .map(|(n, _)| n.as_str())
        .collect();
    naming.sort();
    assert_eq!(naming, ["operator", "worker"]);
    assert_eq!(
        s.source_digest(),
        "sha256:ee10ee247b875d03f2d6ec520f833abe3cc0657aa3dbe3e53af372f0154e5245"
    );
}

#[test]
fn host_daemon_is_accepted_with_28_refused_in_the_weft_step() {
    let s = accepted("host-daemon");
    assert_eq!(
        s.architecture_digest(),
        "sha256:df3a9619a26c1258933cfa107b15aa88e137c5e59051d18817721f540cb99eac"
    );
    assert_eq!(s.binding_digest(), NO_BINDING);
    assert_eq!(s.binding(), None);
    assert_eq!(weft_counts(&s)["refused"], 28);
    assert_eq!(
        s.source_digest(),
        "sha256:25303268d8346b841fe1f97a8a30a5b996d006e8dae0dbf9426d86bcaa5201c4"
    );
}

#[test]
fn the_digest_does_not_depend_on_the_files_layout() {
    // The same document written compactly is another file with the same member digests.
    let doc: Value = serde_json::from_slice(&golden("workstation")).expect("JSON");
    let compact = serde_json::to_vec(&doc).expect("serializes");
    let a = accepted("workstation");
    let b = Submission::read(&compact).expect("the compact copy is accepted");
    assert_eq!(a.architecture_digest(), b.architecture_digest());
    assert_eq!(a.binding_digest(), b.binding_digest());
    assert_ne!(a.source_digest(), b.source_digest());
    assert_eq!(b.source_digest(), digest(&compact));
}

#[test]
fn canonical_json_sorts_keys_by_their_utf8_bytes_with_no_white_space() {
    let v: Value = serde_json::from_str(
        r#"{ "b": 1, "a": [ {"é": null, "z": "x\"y", "Z": 1.5} ], "B": true }"#,
    )
    .expect("JSON");
    assert_eq!(
        canonical(&v),
        r#"{"B":true,"a":[{"Z":1.5,"z":"x\"y","é":null}],"b":1}"#
    );
    assert_eq!(canonical(&Value::Null), "null");
    assert_eq!(digest(b"null"), NO_BINDING);
}
