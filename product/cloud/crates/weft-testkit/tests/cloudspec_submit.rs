//! A cloudspec submission recorded as a deployment source (`story:cloudspec-submission-submit`):
//! the control host takes a `cloudspec-weft-submission/1` file on `POST /cloudspec/submissions`,
//! checks it with `weft-cloudspec` and records it through `weft.deployments.SubmitDeploymentSource`
//! with kind `Cloudspec`, keeping the file's bytes by their source digest
//! (`weft_impl::documents`). The goldens are the committed copies of
//! `tests/fixtures/cloudspec/`; no network, no cloudspec binary.

use serde_json::{json, Value};
use weft_client::{Client, Expect};
use weft_impl::context::mint;
use weft_impl::documents;
use weft_testkit::steps::{self, DEPLOYER};
use weft_testkit::{ControlHost, InProcess};
use weft_types::behaviour::{DeploymentSourceStorage, TranslatorStorage};
use weft_types::deployments::{
    SourceDigest, SourceFormatVersion, SourceKind, TranslatorData, TranslatorSnapshot,
    TranslatorState, TranslatorVersion,
};
use weft_types::primitives::{Timestamp, Uuid};
use weft_types::shared::{ArtifactDigest, TranslatorId};

const SUBMIT: &str = "weft.deployments.SubmitDeploymentSource";
const SOURCE_SUBMITTED: &str = "weft.deployments.SourceSubmitted";
const FORMAT: &str = "cloudspec-weft-submission/1";

/// The digests of `snippet.submission.json`: of the file, of its architecture, of its binding.
const SNIPPET_SOURCE: &str =
    "sha256:d58f6b6cf43d2f0bb672a11c68c7bb51c0b848dbddf2b9ff999ee08567dfaead";
const SNIPPET_ARCHITECTURE: &str =
    "sha256:3ce513003eec18301050ec62932fdfdc45a7b4193d5e3b43c82a5a1cfa5c948f";
const SNIPPET_BINDING: &str =
    "sha256:74234e98afe7498fb5daf1f36ac2d78acc339464f950703b8c019892f982b90b";

/// The bytes of one committed golden, by its short name.
fn golden(name: &str) -> Vec<u8> {
    let path =
        weft_testkit::repo_root().join(format!("tests/fixtures/cloudspec/{name}.submission.json"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// One scope with a formation: the caller acting in it (a Deployer's attributes, a principal
/// included), its scope and its formation.
struct Scoped {
    caller: Value,
    scope_id: String,
    formation_id: String,
}

/// Tenant `name`, its account and the account's scope, and formation `site` of that scope.
fn scoped(client: &Client, name: &str) -> Scoped {
    let tenant_id = steps::create_tenant(client, name);
    let account_id = steps::create_account(client, &tenant_id);
    let scope_id = steps::record_account_scope(client, &account_id);
    let mut caller = steps::account_caller(&tenant_id, &account_id, &scope_id);
    caller["principal_id"] = json!(mint());
    let formation_id = steps::create_formation(client, &caller, &scope_id);
    Scoped {
        caller,
        scope_id,
        formation_id,
    }
}

/// The rows of `weft.deployments.DeploymentSources` of `scope_id`.
fn sources(client: &Client, scope_id: &str) -> Vec<Value> {
    client
        .view(
            "weft.deployments.DeploymentSources",
            &[("scope_id", scope_id)],
        )
        .ok_body(200)["rows"]
        .as_array()
        .expect("rows")
        .clone()
}

fn submissions(formation_id: &str) -> String {
    format!("/cloudspec/submissions?formation_id={formation_id}")
}

/// Statements 1, 2 and 4 of the acceptance, through the `weft` binary: the snippet golden is
/// recorded as one `Cloudspec` source with its three digests and its bytes kept; a copy with
/// another format, a copy with one string of its architecture changed, and a valid file naming
/// a formation of another scope leave no source and no document.
#[test]
fn a_posted_snippet_is_recorded_as_one_cloudspec_source_and_changed_copies_are_refused_by_name() {
    let mut host = ControlHost::start();
    let client = host.client();
    let acme = scoped(&client, "acme");
    let globex = scoped(&client, "globex");
    let post = |bytes: &[u8], formation_id: &str| {
        client
            .post_bytes_as(
                &submissions(formation_id),
                bytes,
                DEPLOYER,
                acme.caller.clone(),
            )
            .expect("the control host answers")
    };
    let snippet = golden("snippet");

    // 2. Changed copies are refused 422 by the reader's name, and write nothing.
    let mut other_format: Value = serde_json::from_slice(&snippet).expect("JSON");
    other_format["format"] = json!("cloudspec-weft-submission/2");
    let other_format = serde_json::to_vec_pretty(&other_format).expect("bytes");
    let mut changed: Value = serde_json::from_slice(&snippet).expect("JSON");
    changed["architecture"]["summary"] = json!("Another summary.");
    let changed = serde_json::to_vec_pretty(&changed).expect("bytes");
    let before = host.last_seq();
    for (bytes, refusal) in [
        (&other_format, "UnknownFormat"),
        (&changed, "DigestMismatch"),
    ] {
        let answer = post(bytes, &acme.formation_id);
        assert_eq!(
            (answer.status, answer.body.clone()),
            (422, json!({"refusal": refusal})),
            "{refusal}"
        );
    }
    let lines = host.log_after(before);
    assert_eq!(
        lines
            .iter()
            .map(|line| (line["kind"].clone(), line["status"].clone()))
            .collect::<Vec<_>>(),
        vec![
            (json!("request"), json!(422)),
            (json!("request"), json!(422))
        ],
        "one request line each, no event: {lines:?}"
    );

    // 4. A valid file naming a formation of another scope: the command's NotFound.
    let workstation = golden("workstation");
    let foreign = post(&workstation, &globex.formation_id);
    assert_eq!(
        (
            foreign.status,
            foreign.body["outcome"].clone(),
            foreign.body["error"].clone()
        ),
        (
            409,
            json!("formation-foreign"),
            json!("weft.deployments.NotFound")
        ),
        "{}",
        foreign.body
    );
    assert_eq!(sources(&client, &acme.scope_id), Vec::<Value>::new());
    assert_eq!(sources(&client, &globex.scope_id), Vec::<Value>::new());

    // 1. The snippet: the command's answer, one source with the golden's digests.
    let before = host.last_seq();
    let answer = post(&snippet, &acme.formation_id);
    assert_eq!(answer.status, 202, "{}", answer.body);
    assert_eq!(
        answer.body["outcome"],
        json!("submitted"),
        "{}",
        answer.body
    );
    assert_eq!(
        answer.body["published"][0]["event"],
        json!(SOURCE_SUBMITTED),
        "{}",
        answer.body
    );
    let rows = sources(&client, &acme.scope_id);
    assert_eq!(rows.len(), 1, "{rows:?}");
    let row = &rows[0];
    for (field, value) in [
        ("kind", json!("Cloudspec")),
        ("format_version", json!(FORMAT)),
        ("source_digest", json!(SNIPPET_SOURCE)),
        ("architecture_digest", json!(SNIPPET_ARCHITECTURE)),
        ("binding_digest", json!(SNIPPET_BINDING)),
        ("state", json!("Submitted")),
        ("formation_id", json!(acme.formation_id)),
        ("submitted_by", acme.caller["principal_id"].clone()),
        ("helm_values", json!([])),
    ] {
        assert_eq!(row[field], value, "{field} of {row}");
    }
    let lines = host.log_after(before);
    assert!(
        lines.iter().any(|line| line["kind"] == "request"
            && line["operation"] == SUBMIT
            && line["status"] == 202),
        "{lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line["kind"] == "event" && line["name"] == SOURCE_SUBMITTED),
        "{lines:?}"
    );

    // The bytes: kept for the snippet under its source digest, and nothing else kept (another
    // format, a changed architecture, a foreign formation).
    host.stop();
    let ports = weft_impl::Ports::open(&host.state_path()).expect("the state file opens");
    assert_eq!(
        documents::get(&ports, &SourceDigest(SNIPPET_SOURCE.to_owned())),
        Some(snippet.clone())
    );
    assert_eq!(documents::list(&ports), vec![snippet]);
    assert_eq!(DeploymentSourceStorage::list(&ports).len(), 1);
}

/// One translator row of `kind` and `format`, in `state`: its identity and the snapshot.
fn translator(
    kind: SourceKind,
    format: &str,
    state: TranslatorState,
) -> (String, TranslatorSnapshot) {
    let id = mint();
    let snapshot = TranslatorSnapshot {
        state,
        data: TranslatorData {
            translator_id: TranslatorId(Uuid(id.clone())),
            source_kind: kind,
            source_format_version: SourceFormatVersion(format.to_owned()),
            translator_version: TranslatorVersion("1".to_owned()),
            artifact_digest: ArtifactDigest("sha256:translator".to_owned()),
            registered_at: Timestamp("2026-11-28T00:00:00Z".to_owned()),
        },
    };
    (id, snapshot)
}

/// Statement 3 of the acceptance and every other outcome of the contract, in its precedence,
/// with the command sent directly as the Deployer: each refusal by its outcome and error, none
/// writing a row; then a `Cloudspec`, a `Compose` and a `Helm` source submitted. Translators are
/// rows of the storage port (`RegisterTranslator` has no behaviour).
#[test]
fn submit_deployment_source_answers_every_outcome_of_its_contract_in_order() {
    let host = InProcess::start();
    let client = host.client();
    let acme = scoped(client, "acme");
    let globex = scoped(client, "globex");
    let environment =
        steps::create_environment(client, &acme.caller, &acme.formation_id, &acme.scope_id);
    let foreign_environment = steps::create_environment(
        client,
        &globex.caller,
        &globex.formation_id,
        &globex.scope_id,
    );
    let mut ports = host.ports();
    let (compose, row) = translator(
        SourceKind::Compose,
        "compose/1",
        TranslatorState::Registered,
    );
    TranslatorStorage::put(&mut ports, row);
    let (helm, row) = translator(SourceKind::Helm, "helm/3", TranslatorState::Registered);
    TranslatorStorage::put(&mut ports, row);
    let (retired, row) = translator(SourceKind::Compose, "compose/1", TranslatorState::Retired);
    TranslatorStorage::put(&mut ports, row);

    let cloudspec = json!({
        "scope_id": acme.scope_id,
        "formation_id": acme.formation_id,
        "kind": "Cloudspec",
        "format_version": FORMAT,
        "source_digest": SNIPPET_SOURCE,
        "helm_values": [],
        "architecture_digest": SNIPPET_ARCHITECTURE,
        "binding_digest": SNIPPET_BINDING,
    });
    let compose_source = json!({
        "scope_id": acme.scope_id,
        "formation_id": acme.formation_id,
        "kind": "Compose",
        "format_version": "compose/1",
        "source_digest": "sha256:compose",
        "helm_values": [],
        "translator_id": compose,
    });
    let with = |base: &Value, changes: Value| {
        let mut input = base.clone();
        for (key, value) in changes.as_object().expect("an object") {
            if value.is_null() {
                input.as_object_mut().expect("an object").remove(key);
            } else {
                input[key] = value.clone();
            }
        }
        input
    };
    let values = |environment_id: &str| json!([{"environment_id": environment_id, "values_digest": "sha256:values"}]);
    let helm_source = json!({
        "scope_id": acme.scope_id,
        "formation_id": acme.formation_id,
        "kind": "Helm",
        "format_version": "helm/3",
        "source_digest": "sha256:chart",
        "helm_values": values(&environment),
        "translator_id": helm,
    });

    let refusals: Vec<(&str, Value, u16, &str, &str)> = vec![
        (
            "another scope",
            with(&cloudspec, json!({"scope_id": globex.scope_id})),
            403,
            "not-callers-scope",
            "NotFound",
        ),
        (
            "values for a cloudspec source",
            with(&cloudspec, json!({"helm_values": values(&environment)})),
            422,
            "values-not-helm",
            "ValuesOnlyForHelm",
        ),
        (
            "an embedded compose source",
            with(
                &compose_source,
                json!({"embedded_in": {"parent_source_id": mint(), "component": "site"}}),
            ),
            422,
            "not-a-spec-source",
            "NotASpecSource",
        ),
        (
            "compose without a translator",
            with(&compose_source, json!({"translator_id": null})),
            422,
            "translator-required",
            "TranslatorRequired",
        ),
        (
            "cloudspec with a translator",
            with(&cloudspec, json!({"translator_id": compose})),
            422,
            "translator-not-allowed",
            "TranslatorNotAllowed",
        ),
        (
            "cloudspec without binding_digest",
            with(&cloudspec, json!({"binding_digest": null})),
            422,
            "digests-required",
            "DigestsRequired",
        ),
        (
            "cloudspec without architecture_digest",
            with(&cloudspec, json!({"architecture_digest": null})),
            422,
            "digests-required",
            "DigestsRequired",
        ),
        (
            "compose with an architecture_digest",
            with(
                &compose_source,
                json!({"architecture_digest": SNIPPET_ARCHITECTURE}),
            ),
            422,
            "digests-not-cloudspec",
            "DigestsOnlyForCloudspec",
        ),
        (
            "compose with a binding_digest",
            with(&compose_source, json!({"binding_digest": SNIPPET_BINDING})),
            422,
            "digests-not-cloudspec",
            "DigestsOnlyForCloudspec",
        ),
        (
            "no such formation",
            with(&cloudspec, json!({"formation_id": mint()})),
            409,
            "formation-missing",
            "NotFound",
        ),
        (
            "no such formation and no such translator",
            with(
                &compose_source,
                json!({"formation_id": mint(), "translator_id": mint()}),
            ),
            409,
            "formation-missing",
            "NotFound",
        ),
        (
            "no such translator",
            with(&compose_source, json!({"translator_id": mint()})),
            409,
            "no-translator",
            "NoTranslator",
        ),
        (
            "no such translator and a foreign formation",
            with(
                &compose_source,
                json!({"translator_id": mint(), "formation_id": globex.formation_id}),
            ),
            409,
            "no-translator",
            "NoTranslator",
        ),
        (
            "a formation of another scope",
            with(&cloudspec, json!({"formation_id": globex.formation_id})),
            409,
            "formation-foreign",
            "NotFound",
        ),
        (
            "a foreign formation and a retired translator",
            with(
                &compose_source,
                json!({"formation_id": globex.formation_id, "translator_id": retired}),
            ),
            409,
            "formation-foreign",
            "NotFound",
        ),
        (
            "a retired translator",
            with(&compose_source, json!({"translator_id": retired})),
            409,
            "translator-not-for-source",
            "NoTranslator",
        ),
        (
            "a translator of another kind",
            with(&compose_source, json!({"translator_id": helm})),
            409,
            "translator-not-for-source",
            "NoTranslator",
        ),
        (
            "a translator of another format",
            with(&compose_source, json!({"format_version": "compose/2"})),
            409,
            "translator-not-for-source",
            "NoTranslator",
        ),
        (
            "values of another formation's environment",
            with(
                &helm_source,
                json!({"helm_values": values(&foreign_environment)}),
            ),
            409,
            "values-environment-foreign",
            "NotFound",
        ),
    ];
    let send = |caller: &Value, input: &Value| {
        client
            .command(SUBMIT, DEPLOYER, caller.clone(), input.clone())
            .expect("the control host answers")
    };
    for (what, input, status, outcome, error) in &refusals {
        let answer = send(&acme.caller, input);
        assert_eq!(
            (
                answer.status,
                answer.body["outcome"].clone(),
                answer.body["error"].clone()
            ),
            (
                *status,
                json!(outcome),
                json!(format!("weft.deployments.{error}"))
            ),
            "{what}: {}",
            answer.body
        );
    }
    // A caller without a principal: the submitter cannot be recorded, nothing is written.
    let mut nobody = acme.caller.clone();
    nobody
        .as_object_mut()
        .expect("an object")
        .remove("principal_id");
    let answer = send(&nobody, &cloudspec);
    assert_eq!(answer.status, 501, "{}", answer.body);
    assert_eq!(answer.body["committed"], json!(false), "{}", answer.body);
    assert_eq!(
        DeploymentSourceStorage::list(&ports).len(),
        0,
        "a refusal wrote a row"
    );

    for (input, kind) in [
        (&cloudspec, SourceKind::Cloudspec),
        (&compose_source, SourceKind::Compose),
        (&helm_source, SourceKind::Helm),
    ] {
        let answer = send(&acme.caller, input);
        assert_eq!(
            (answer.status, answer.body["outcome"].clone()),
            (202, json!("submitted")),
            "{kind:?}: {}",
            answer.body
        );
        let payload = &answer.body["published"][0]["payload"];
        assert_eq!(
            payload["source_digest"], input["source_digest"],
            "{payload}"
        );
        assert_eq!(payload["scope_id"], json!(acme.scope_id), "{payload}");
    }
    let rows = DeploymentSourceStorage::list(&ports);
    assert_eq!(rows.len(), 3, "{rows:?}");
    let principal = acme.caller["principal_id"].as_str().expect("a principal");
    for row in &rows {
        assert_eq!(row.data.submitted_by.0 .0, principal, "{row:?}");
        assert_eq!(row.data.scope_id.0 .0, acme.scope_id, "{row:?}");
        let cloudspec = row.data.kind == SourceKind::Cloudspec;
        assert_eq!(
            (
                row.data.architecture_digest.as_ref().map(|d| d.0.as_str()),
                row.data.binding_digest.as_ref().map(|d| d.0.as_str()),
            ),
            if cloudspec {
                (Some(SNIPPET_ARCHITECTURE), Some(SNIPPET_BINDING))
            } else {
                (None, None)
            },
            "{row:?}"
        );
    }
    let helm_row = rows
        .iter()
        .find(|row| row.data.kind == SourceKind::Helm)
        .expect("the helm source");
    assert_eq!(helm_row.data.helm_values.len(), 1, "{helm_row:?}");
}
