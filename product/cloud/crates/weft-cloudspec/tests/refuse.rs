//! Copies of the goldens changed in one place, in memory, are refused by name, at the first check
//! that fails: the shape, the format, the architecture digest, the binding digest, the binding's
//! architecture, the binding's roles.

use serde_json::{json, Value};
use weft_cloudspec::{canonical, digest, Member, Refusal, Submission};

use crate::{bytes, golden_value};

fn refused(v: &Value) -> Refusal {
    match Submission::read(&bytes(v)) {
        Ok(_) => panic!("accepted: {v}"),
        Err(r) => r,
    }
}

/// The digest of a member's canonical JSON.
fn of(v: &Value) -> String {
    digest(canonical(v).as_bytes())
}

/// Both digests written again from the members.
fn recompute(v: &mut Value) {
    v["digests"]["architecture"] = of(&v["architecture"]).into();
    v["digests"]["binding"] = of(&v["binding"]).into();
}

fn assert_not_a_submission(v: &Value, what: &str) {
    let r = refused(v);
    assert!(matches!(r, Refusal::NotASubmission { .. }), "{what}: {r:?}");
    assert_eq!(r.name(), "NotASubmission", "{what}");
}

#[test]
fn an_empty_array_is_not_a_submission() {
    assert_not_a_submission(&json!([]), "[]");
}

#[test]
fn bytes_that_are_not_json_are_not_a_submission() {
    for b in [
        &b""[..],
        b"{",
        b"\xff\xfe",
        b"null",
        b"\"cloudspec-weft-submission/1\"",
    ] {
        let r = Submission::read(b).expect_err("refused");
        assert!(matches!(r, Refusal::NotASubmission { .. }), "{b:?}: {r:?}");
    }
}

#[test]
fn snippet_without_its_digests_binding_key_is_not_a_submission() {
    let mut v = golden_value("snippet");
    v["digests"]
        .as_object_mut()
        .expect("digests")
        .remove("binding");
    assert_not_a_submission(&v, "digests.binding removed");
}

#[test]
fn digests_without_both_string_values_is_not_a_submission() {
    let base = golden_value("snippet");
    let mut cases: Vec<(&str, Value)> = Vec::new();
    let mut v = base.clone();
    v["digests"]
        .as_object_mut()
        .expect("digests")
        .remove("architecture");
    cases.push(("digests.architecture removed", v));
    for (key, value) in [
        ("architecture", Value::Null),
        ("binding", Value::Null),
        ("binding", json!(1)),
        ("architecture", json!(["sha256:00"])),
    ] {
        let mut v = base.clone();
        v["digests"][key] = value;
        cases.push(("digests member not a string", v));
    }
    for value in [Value::Null, json!([]), json!("sha256:00")] {
        let mut v = base.clone();
        v["digests"] = value;
        cases.push(("digests not an object", v));
    }
    for (what, v) in &cases {
        assert_not_a_submission(v, what);
    }
}

#[test]
fn a_document_without_one_of_the_five_members_is_not_a_submission() {
    for member in ["format", "architecture", "binding", "coverage", "digests"] {
        let mut v = golden_value("workstation");
        v.as_object_mut().expect("an object").remove(member);
        assert_not_a_submission(&v, member);
    }
}

#[test]
fn a_document_with_a_sixth_member_is_not_a_submission() {
    let mut v = golden_value("snippet");
    v["extra"] = json!({});
    assert_not_a_submission(&v, "a sixth member");
}

#[test]
fn another_format_is_unknown() {
    let mut v = golden_value("snippet");
    v["format"] = "cloudspec-weft-submission/2".into();
    let r = refused(&v);
    assert_eq!(
        r,
        Refusal::UnknownFormat {
            found: json!("cloudspec-weft-submission/2")
        }
    );
    assert_eq!(r.name(), "UnknownFormat");
    for found in [json!("cloudspec-architecture/1"), json!(1), Value::Null] {
        v["format"] = found.clone();
        assert_eq!(refused(&v), Refusal::UnknownFormat { found });
    }
}

#[test]
fn the_format_is_checked_before_the_digests() {
    let mut v = golden_value("snippet");
    v["format"] = "cloudspec-weft-submission/2".into();
    v["architecture"]["summary"] = "changed".into();
    assert_eq!(refused(&v).name(), "UnknownFormat");
}

#[test]
fn one_changed_string_of_the_architecture_is_a_digest_mismatch() {
    let mut v = golden_value("snippet");
    let declared = v["digests"]["architecture"]
        .as_str()
        .expect("a digest")
        .to_owned();
    v["architecture"]["parts"]["hello"]["command"][2] = "echo goodbye".into();
    let computed = of(&v["architecture"]);
    assert_ne!(declared, computed);
    let r = refused(&v);
    assert_eq!(
        r,
        Refusal::DigestMismatch {
            member: Member::Architecture,
            declared,
            computed
        }
    );
    assert_eq!(r.name(), "DigestMismatch");
}

#[test]
fn a_changed_binding_of_workstation_is_a_digest_mismatch_of_the_binding() {
    let mut v = golden_value("workstation");
    let declared = v["digests"]["binding"]
        .as_str()
        .expect("a digest")
        .to_owned();
    v["binding"]["binding"] = "joined".into();
    let computed = of(&v["binding"]);
    let r = refused(&v);
    assert_eq!(
        r,
        Refusal::DigestMismatch {
            member: Member::Binding,
            declared,
            computed
        }
    );
    assert_eq!(r.name(), "DigestMismatch");
}

#[test]
fn a_binding_added_to_snippet_without_its_digest_is_a_digest_mismatch() {
    let mut v = golden_value("snippet");
    v["binding"] = json!({"architecture": "hello", "binding": "b", "roles": {}});
    match refused(&v) {
        Refusal::DigestMismatch { member, .. } => assert_eq!(member, Member::Binding),
        r => panic!("{r:?}"),
    }
}

#[test]
fn the_architecture_digest_is_checked_before_the_binding_digest() {
    let mut v = golden_value("workstation");
    v["architecture"]["summary"] = "changed".into();
    v["binding"]["binding"] = "joined".into();
    match refused(&v) {
        Refusal::DigestMismatch { member, .. } => assert_eq!(member, Member::Architecture),
        r => panic!("{r:?}"),
    }
}

#[test]
fn a_binding_for_another_architecture_is_refused() {
    let mut v = golden_value("workstation");
    v["binding"]["architecture"] = "laptop".into();
    recompute(&mut v);
    let r = refused(&v);
    assert_eq!(
        r,
        Refusal::BindingForOtherArchitecture {
            binding: json!("laptop"),
            architecture: json!("workstation")
        }
    );
    assert_eq!(r.name(), "BindingForOtherArchitecture");
}

#[test]
fn a_binding_that_names_no_architecture_is_for_another_architecture() {
    let mut v = golden_value("workstation");
    v["binding"]
        .as_object_mut()
        .expect("binding")
        .remove("architecture");
    recompute(&mut v);
    assert_eq!(
        refused(&v),
        Refusal::BindingForOtherArchitecture {
            binding: Value::Null,
            architecture: json!("workstation")
        }
    );
}

#[test]
fn a_role_no_part_names_is_refused() {
    let mut v = golden_value("workstation");
    v["binding"]["roles"]["nobody"] = json!({"image": "registry.example/nobody@sha256:00"});
    recompute(&mut v);
    let r = refused(&v);
    assert_eq!(
        r,
        Refusal::BindingRoleNamesNoPart {
            role: "nobody".into()
        }
    );
    assert_eq!(r.name(), "BindingRoleNamesNoPart");
}

#[test]
fn the_binding_architecture_is_checked_before_its_roles() {
    let mut v = golden_value("workstation");
    v["binding"]["architecture"] = "laptop".into();
    v["binding"]["roles"]["nobody"] = json!({});
    recompute(&mut v);
    assert_eq!(refused(&v).name(), "BindingForOtherArchitecture");
}

#[test]
fn each_refusal_has_its_own_name() {
    let names = [
        Refusal::NotASubmission {
            reason: String::new(),
        },
        Refusal::UnknownFormat { found: Value::Null },
        Refusal::DigestMismatch {
            member: Member::Architecture,
            declared: String::new(),
            computed: String::new(),
        },
        Refusal::BindingForOtherArchitecture {
            binding: Value::Null,
            architecture: Value::Null,
        },
        Refusal::BindingRoleNamesNoPart {
            role: String::new(),
        },
    ]
    .map(|r| r.name());
    assert_eq!(
        names,
        [
            "NotASubmission",
            "UnknownFormat",
            "DigestMismatch",
            "BindingForOtherArchitecture",
            "BindingRoleNamesNoPart"
        ]
    );
    assert_eq!(Member::Architecture.as_str(), "architecture");
    assert_eq!(Member::Binding.as_str(), "binding");
}
