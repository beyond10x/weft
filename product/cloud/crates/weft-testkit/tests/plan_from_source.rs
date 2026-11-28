//! A submitted cloudspec architecture planned for an environment
//! (`story:cloudspec-submission-plan-from-source`): `weft.deployments.PlanFromSource` reads the
//! file kept under the source digest (`weft_impl::documents`), checks it with `weft-cloudspec`,
//! and in one command records a `FormationRevision` of the source's formation whose document
//! digest is the source's architecture digest and a `Plan` that names the source and no
//! translation. The goldens are the committed copies of `tests/fixtures/cloudspec/`; no network,
//! no cloudspec binary.

use serde_json::{json, Value};
use weft_client::{Client, Expect};
use weft_impl::context::mint;
use weft_impl::documents;
use weft_testkit::steps::{self, ACCOUNT_ADMIN, DEPLOYER};
use weft_testkit::{ControlHost, InProcess};
use weft_types::architecture::FormationRevisionState;
use weft_types::behaviour::{DeploymentSourceStorage, FormationRevisionStorage, PlanStorage};
use weft_types::deployments::{
    DeploymentSourceData, DeploymentSourceSnapshot, DeploymentSourceState, PlanState, SourceDigest,
    SourceFormatVersion, SourceKind,
};
use weft_types::primitives::{Timestamp, Uuid};
use weft_types::shared::{DeploymentSourceId, FormationId, PrincipalId, ScopeId};

const PLAN_FROM_SOURCE: &str = "weft.deployments.PlanFromSource";
const PLAN_CREATED: &str = "weft.deployments.PlanCreated";
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

/// One scope with a formation and an environment of it: the caller acting in the scope (a
/// Deployer's attributes, a principal included), its scope, formation and environment.
struct Scoped {
    caller: Value,
    scope_id: String,
    formation_id: String,
    environment_id: String,
}

/// Tenant `name`, its account and the account's scope, formation `site` of that scope and
/// environment `prod` of the formation.
fn scoped(client: &Client, name: &str) -> Scoped {
    let tenant_id = steps::create_tenant(client, name);
    let account_id = steps::create_account(client, &tenant_id);
    let scope_id = steps::record_account_scope(client, &account_id);
    let mut caller = steps::account_caller(&tenant_id, &account_id, &scope_id);
    caller["principal_id"] = json!(mint());
    let formation_id = steps::create_formation(client, &caller, &scope_id);
    let environment_id = steps::create_environment(client, &caller, &formation_id, &scope_id);
    Scoped {
        caller,
        scope_id,
        formation_id,
        environment_id,
    }
}

/// The input of `PlanFromSource`.
fn plan_input(source_id: &str, scope_id: &str, environment_id: &str) -> Value {
    json!({"source_id": source_id, "scope_id": scope_id, "environment_id": environment_id})
}

/// The `formation_revision_id` of the `PlanCreated` an answer published.
fn published_revision(answer: &Value) -> String {
    assert_eq!(
        answer["published"][0]["event"],
        json!(PLAN_CREATED),
        "{answer}"
    );
    answer["published"][0]["payload"]["formation_revision_id"]
        .as_str()
        .unwrap_or_else(|| panic!("no formation_revision_id in {answer}"))
        .to_owned()
}

/// Statement 1 of the acceptance, through the `weft` binary: the snippet golden posted to
/// `/cloudspec/submissions` is planned twice for the formation's environment. Each answer is
/// `planned` with a `PlanCreated` request and event line in the log; each plan names the source
/// and no translation, is `Simulating`, and has a new `Recorded` revision of the source's
/// formation with the snippet's architecture digest, numbered 1 and then 2.
#[test]
fn a_submitted_snippet_is_planned_into_a_new_revision_of_its_formation_naming_the_source() {
    let mut host = ControlHost::start();
    let client = host.client();
    let acme = scoped(&client, "acme");
    let submitted = client
        .post_bytes_as(
            &format!("/cloudspec/submissions?formation_id={}", acme.formation_id),
            &golden("snippet"),
            DEPLOYER,
            acme.caller.clone(),
        )
        .expect("the control host answers");
    assert_eq!(submitted.status, 202, "{}", submitted.body);
    let source_id = submitted.body["published"][0]["payload"]["source_id"]
        .as_str()
        .unwrap_or_else(|| panic!("no source_id in {}", submitted.body))
        .to_owned();

    let mut revisions = Vec::new();
    for _ in 0..2 {
        let before = host.last_seq();
        let answer = client
            .command(
                PLAN_FROM_SOURCE,
                DEPLOYER,
                acme.caller.clone(),
                plan_input(&source_id, &acme.scope_id, &acme.environment_id),
            )
            .expect("the control host answers");
        assert_eq!(
            (answer.status, answer.body["outcome"].clone()),
            (202, json!("planned")),
            "{}",
            answer.body
        );
        assert_eq!(
            answer.body["published"][0]["payload"]["environment_id"],
            json!(acme.environment_id),
            "{}",
            answer.body
        );
        let lines = host.log_after(before);
        assert!(
            lines.iter().any(|line| line["kind"] == "request"
                && line["operation"] == PLAN_FROM_SOURCE
                && line["status"] == 202),
            "{lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|line| line["kind"] == "event" && line["name"] == PLAN_CREATED),
            "{lines:?}"
        );
        revisions.push(published_revision(&answer.body));
    }

    host.stop();
    let ports = weft_impl::Ports::open(&host.state_path()).expect("the state file opens");
    let plans = PlanStorage::list(&ports);
    assert_eq!(plans.len(), 2, "{plans:?}");
    let principal = acme.caller["principal_id"].as_str().expect("a principal");
    for (number, revision_id) in (1_i64..).zip(&revisions) {
        let plan = plans
            .iter()
            .find(|plan| &plan.data.formation_revision_id.0 .0 == revision_id)
            .unwrap_or_else(|| panic!("no plan of revision {revision_id}: {plans:?}"));
        assert_eq!(plan.state, PlanState::Simulating, "{plan:?}");
        assert_eq!(
            plan.data.source_id.as_ref().map(|id| id.0 .0.as_str()),
            Some(source_id.as_str()),
            "{plan:?}"
        );
        assert_eq!(plan.data.translation_id, None, "{plan:?}");
        assert_eq!(plan.data.scope_id.0 .0, acme.scope_id, "{plan:?}");
        assert_eq!(plan.data.formation_id.0 .0, acme.formation_id, "{plan:?}");
        assert_eq!(
            plan.data.environment_id.0 .0, acme.environment_id,
            "{plan:?}"
        );
        assert_eq!(plan.data.created_by.0 .0, principal, "{plan:?}");
        let revision = FormationRevisionStorage::get(&ports, &plan.data.formation_revision_id)
            .unwrap_or_else(|| panic!("no revision {revision_id}"));
        assert_eq!(
            revision.state,
            FormationRevisionState::Recorded,
            "{revision:?}"
        );
        assert_eq!(
            revision.data.formation_id.0 .0, acme.formation_id,
            "{revision:?}"
        );
        assert_eq!(revision.data.number, number, "{revision:?}");
        assert_eq!(
            revision.data.document_digest.0, SNIPPET_ARCHITECTURE,
            "{revision:?}"
        );
    }
    assert_eq!(FormationRevisionStorage::list(&ports).len(), 2);
}

/// A seeded source of `scope_id` and `formation_id`: its identity and the snapshot.
fn source(
    scope_id: &str,
    formation_id: &str,
    kind: SourceKind,
    state: DeploymentSourceState,
    source_digest: &str,
) -> (String, DeploymentSourceSnapshot) {
    let id = mint();
    let cloudspec = kind == SourceKind::Cloudspec;
    let snapshot = DeploymentSourceSnapshot {
        state,
        data: DeploymentSourceData {
            source_id: DeploymentSourceId(Uuid(id.clone())),
            scope_id: ScopeId(Uuid(scope_id.to_owned())),
            formation_id: FormationId(Uuid(formation_id.to_owned())),
            kind,
            format_version: SourceFormatVersion(
                if cloudspec { FORMAT } else { "compose/1" }.to_owned(),
            ),
            source_digest: SourceDigest(source_digest.to_owned()),
            helm_values: Vec::new(),
            embedded_in: None,
            architecture_digest: cloudspec.then(|| SourceDigest(SNIPPET_ARCHITECTURE.to_owned())),
            binding_digest: cloudspec.then(|| SourceDigest(SNIPPET_BINDING.to_owned())),
            submitted_by: PrincipalId(Uuid(mint())),
            submitted_at: Timestamp("2026-11-28T00:00:00Z".to_owned()),
        },
    };
    (id, snapshot)
}

/// Statements 2 and 3 of the acceptance, with the command sent directly as the Deployer: each
/// refusal of the contract by its outcome and error, in its precedence where two hold together;
/// a source with no stored file, with another file under its digest, and a caller without a
/// principal answer 501 with nothing committed. None writes a `Plan` or a `FormationRevision`.
/// Sources are rows of the storage port; the snippet's bytes are kept as the submission route
/// keeps them.
#[test]
fn plan_from_source_answers_each_refusal_of_its_contract_and_writes_nothing() {
    let host = InProcess::start();
    let client = host.client();
    let acme = scoped(client, "acme");
    let globex = scoped(client, "globex");
    let created = client
        .command(
            "weft.architecture.CreateFormation",
            ACCOUNT_ADMIN,
            acme.caller.clone(),
            json!({"scope_id": acme.scope_id, "name": "depot", "zones": ["z1"], "parity": []}),
        )
        .ok_body(202);
    let depot = created["published"][0]["payload"]["formation_id"]
        .as_str()
        .unwrap_or_else(|| panic!("no formation_id in {created}"))
        .to_owned();
    let depot_environment = steps::create_environment(client, &acme.caller, &depot, &acme.scope_id);

    let mut ports = host.ports();
    documents::put(
        &ports,
        &SourceDigest(SNIPPET_SOURCE.to_owned()),
        &golden("snippet"),
    );
    let mut seed = |scope: &Scoped, kind, state, digest: &str| {
        let (id, row) = source(&scope.scope_id, &scope.formation_id, kind, state, digest);
        DeploymentSourceStorage::put(&mut ports, row);
        id
    };
    let snippet = seed(
        &acme,
        SourceKind::Cloudspec,
        DeploymentSourceState::Submitted,
        SNIPPET_SOURCE,
    );
    let compose = seed(
        &acme,
        SourceKind::Compose,
        DeploymentSourceState::Submitted,
        "sha256:compose",
    );
    let withdrawn = seed(
        &acme,
        SourceKind::Cloudspec,
        DeploymentSourceState::Withdrawn,
        SNIPPET_SOURCE,
    );
    let withdrawn_compose = seed(
        &acme,
        SourceKind::Compose,
        DeploymentSourceState::Withdrawn,
        "sha256:compose",
    );
    let foreign = seed(
        &globex,
        SourceKind::Cloudspec,
        DeploymentSourceState::Submitted,
        SNIPPET_SOURCE,
    );
    let unkept = seed(
        &acme,
        SourceKind::Cloudspec,
        DeploymentSourceState::Submitted,
        "sha256:no-file-kept",
    );
    let other_file = "sha256:another-file";
    let mismatched = seed(
        &acme,
        SourceKind::Cloudspec,
        DeploymentSourceState::Submitted,
        other_file,
    );
    documents::put(
        &ports,
        &SourceDigest(other_file.to_owned()),
        &golden("workstation"),
    );

    let input =
        |source: &str, scope: &str, environment: &str| plan_input(source, scope, environment);
    let refusals: Vec<(&str, Value, u16, &str, &str)> = vec![
        (
            "another scope",
            input(&snippet, &globex.scope_id, &acme.environment_id),
            403,
            "not-callers-scope",
            "NotFound",
        ),
        (
            "an unknown source",
            input(&mint(), &acme.scope_id, &acme.environment_id),
            409,
            "no-such-source",
            "NotFound",
        ),
        (
            "an unknown source and an unknown environment",
            input(&mint(), &acme.scope_id, &mint()),
            409,
            "no-such-source",
            "NotFound",
        ),
        (
            "an unknown environment",
            input(&snippet, &acme.scope_id, &mint()),
            409,
            "no-such-environment",
            "NotFound",
        ),
        (
            "a source of another scope and an unknown environment",
            input(&foreign, &acme.scope_id, &mint()),
            409,
            "no-such-environment",
            "NotFound",
        ),
        (
            "a source of another scope",
            input(&foreign, &acme.scope_id, &acme.environment_id),
            409,
            "source-of-other-scope",
            "NotFound",
        ),
        (
            "a compose source",
            input(&compose, &acme.scope_id, &acme.environment_id),
            409,
            "source-not-cloudspec",
            "SourceNotCloudspec",
        ),
        (
            "a withdrawn compose source",
            input(&withdrawn_compose, &acme.scope_id, &acme.environment_id),
            409,
            "source-not-cloudspec",
            "SourceNotCloudspec",
        ),
        (
            "a withdrawn cloudspec source",
            input(&withdrawn, &acme.scope_id, &acme.environment_id),
            409,
            "source-withdrawn",
            "SourceIsWithdrawn",
        ),
        (
            "a withdrawn source and an environment of another formation",
            input(&withdrawn, &acme.scope_id, &depot_environment),
            409,
            "source-withdrawn",
            "SourceIsWithdrawn",
        ),
        (
            "an environment of another formation",
            input(&snippet, &acme.scope_id, &depot_environment),
            409,
            "environment-foreign",
            "NotFound",
        ),
        (
            "an environment of another scope",
            input(&snippet, &acme.scope_id, &globex.environment_id),
            409,
            "environment-foreign",
            "NotFound",
        ),
    ];
    let send = |caller: &Value, input: &Value| {
        client
            .command(PLAN_FROM_SOURCE, DEPLOYER, caller.clone(), input.clone())
            .expect("the control host answers")
    };
    let nothing_written = |what: &str| {
        assert_eq!(PlanStorage::list(&ports), Vec::new(), "{what} wrote a plan");
        assert_eq!(
            FormationRevisionStorage::list(&ports),
            Vec::new(),
            "{what} wrote a revision"
        );
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
        nothing_written(what);
    }

    let mut nobody = acme.caller.clone();
    nobody
        .as_object_mut()
        .expect("an object")
        .remove("principal_id");
    for (what, caller, source) in [
        ("a source with no stored file", &acme.caller, &unkept),
        (
            "a source with another file stored",
            &acme.caller,
            &mismatched,
        ),
        ("a caller without a principal", &nobody, &snippet),
    ] {
        let answer = send(caller, &input(source, &acme.scope_id, &acme.environment_id));
        assert_eq!(
            (answer.status, answer.body["committed"].clone()),
            (501, json!(false)),
            "{what}: {}",
            answer.body
        );
        nothing_written(what);
    }

    let answer = send(
        &acme.caller,
        &input(&snippet, &acme.scope_id, &acme.environment_id),
    );
    assert_eq!(
        (answer.status, answer.body["outcome"].clone()),
        (202, json!("planned")),
        "{}",
        answer.body
    );
    let revision_id = published_revision(&answer.body);
    let revisions = FormationRevisionStorage::list(&ports);
    assert_eq!(revisions.len(), 1, "{revisions:?}");
    assert_eq!(revisions[0].data.revision_id.0 .0, revision_id);
    assert_eq!(revisions[0].data.number, 1);
}
