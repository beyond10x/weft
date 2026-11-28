//! The cloudspec path of a test: a submission built from a committed golden, the submission
//! route, `PlanFromSource`, a `Ready` plan and the apply route, and the artifact and runtime the
//! apply's process profile names (`story:slot-node-process`).

use serde_json::{json, Value};
use weft_client::{Answer, Client};

use crate::steps::{self, DEPLOYER, OPERATOR};

/// The artifact of the apply's process profile (`PROCESS` in weft-bin's `cloudspec_apply`).
pub const PROCESS_ARTIFACT_DIGEST: &str = "sha256:weft-skeleton-process";

/// The name of the runtime the apply's process profile runs on.
pub const PROCESS_RUNTIME: &str = "process";

const RELEASE_CONTROLLER: &str = "weft.actors.ReleaseController";

/// The bytes of the committed golden `tests/fixtures/cloudspec/<name>.submission.json`.
///
/// # Panics
///
/// When the file cannot be read.
pub fn golden(name: &str) -> Vec<u8> {
    let path = crate::repo_root().join(format!("tests/fixtures/cloudspec/{name}.submission.json"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The `snippet` golden with its one workload part replaced by `part` under `key` (the scope
/// `jobs` kept), its coverage marking the part `mapped`, and the architecture digest of the
/// copy, so the submission route takes it.
///
/// # Panics
///
/// When the golden is not the JSON this expects.
pub fn submission_with_part(key: &str, part: &Value) -> Vec<u8> {
    submission_with_parts(&[(key, part.clone())])
}

/// The `snippet` golden with its one workload part replaced by `parts` (each under its key,
/// beside the golden's scope `jobs`; a part of the same key replaces it), its coverage marking
/// each part `mapped`, and the architecture digest of the copy, so the submission route takes it.
///
/// # Panics
///
/// When the golden is not the JSON this expects.
pub fn submission_with_parts(parts: &[(&str, Value)]) -> Vec<u8> {
    let mut doc: Value = serde_json::from_slice(&golden("snippet")).expect("the golden is JSON");
    let held = doc["architecture"]["parts"]
        .as_object_mut()
        .expect("the golden has parts");
    held.remove("hello");
    for (key, part) in parts {
        held.insert((*key).to_owned(), part.clone());
    }
    let entries = doc["coverage"]["steps"][0]["entries"]
        .as_array_mut()
        .expect("the golden has coverage entries");
    entries.retain(|entry| {
        !entry["object"]
            .as_str()
            .is_some_and(|object| object.starts_with("parts.hello"))
    });
    for (key, _) in parts {
        entries.push(
            json!({"note": "a test part", "object": format!("parts.{key}"), "status": "mapped"}),
        );
    }
    doc["digests"]["architecture"] = json!(weft_cloudspec::digest(
        weft_cloudspec::canonical(&doc["architecture"]).as_bytes()
    ));
    serde_json::to_vec_pretty(&doc).expect("a JSON value writes")
}

/// The process part of `story:slot-node-process`'s acceptance, run as `deployer`: `command`,
/// `environment` (one value a number, as cloudspec writes `POOLD_SLOTS: 8`), one privileged
/// `prestart` step, `stop: {grace: 2s, kill: group}` and `restart: {policy: always, delay: 1s,
/// keep_down_on_exit: [2]}`.
pub fn process_part() -> Value {
    json!({
        "block": "workloads",
        "kind": "process",
        "in": "jobs",
        "isolation": "none",
        "package": {"kind": "process"},
        "shape": "daemon",
        "run_as": {"user": "deployer"},
        "command": ["poold", "serve"],
        "environment": {"POOLD_LOG": "info", "POOLD_SLOTS": 8},
        "prestart": [{"command": ["mkdir", "-p", "/srv/pool"], "privileged": true}],
        "stop": {"grace": "2s", "kill": "group"},
        "restart": {"policy": "always", "delay": "1s", "keep_down_on_exit": [2]},
    })
}

/// `RegisterArtifact` as Deployer: the artifact of kind `Process` the process profile names.
pub fn register_process_artifact(client: &Client, caller: &Value, scope_id: &str) {
    let answer = client
        .command(
            "weft.artifacts.RegisterArtifact",
            DEPLOYER,
            caller.clone(),
            json!({
                "artifact_digest": PROCESS_ARTIFACT_DIGEST,
                "scope_id": scope_id,
                "kind": "Process",
                "size_bytes": 1,
                "signature": "sig",
                "signer_key_id": "k1",
                "built_from": null,
            }),
        )
        .expect("the control host answers");
    assert_eq!(
        answer.body["outcome"],
        json!("registered"),
        "{}",
        answer.body
    );
}

/// `RegisterRuntime` as ClusterOperator: the runtime `process` of package kind `Process`, which
/// runs a workload of isolation `None`; its identity.
pub fn register_process_runtime(client: &Client) -> String {
    let answer = client
        .command(
            "weft.workloads.RegisterRuntime",
            OPERATOR,
            json!({}),
            json!({
                "name": PROCESS_RUNTIME,
                "version": "1",
                "package_kinds": ["Process"],
                "minimum_isolation": "None",
            }),
        )
        .expect("the control host answers");
    assert_eq!(
        answer.body["outcome"],
        json!("registered"),
        "{}",
        answer.body
    );
    payload(&answer.body, "runtime_id")
}

/// Posts `bytes` to `/cloudspec/submissions` for `formation_id` as the Deployer: the source.
pub fn submit(client: &Client, caller: &Value, formation_id: &str, bytes: &[u8]) -> String {
    let answer = client
        .post_bytes_as(
            &format!("/cloudspec/submissions?formation_id={formation_id}"),
            bytes,
            DEPLOYER,
            caller.clone(),
        )
        .expect("the control host answers");
    assert_eq!(answer.status, 202, "{}", answer.body);
    payload(&answer.body, "source_id")
}

/// `PlanFromSource` of `source_id` for `environment_id` as the Deployer: the plan.
pub fn plan(
    client: &Client,
    caller: &Value,
    source_id: &str,
    scope_id: &str,
    environment_id: &str,
) -> String {
    let answer = client
        .command(
            "weft.deployments.PlanFromSource",
            DEPLOYER,
            caller.clone(),
            json!({"source_id": source_id, "scope_id": scope_id, "environment_id": environment_id}),
        )
        .expect("the control host answers");
    assert_eq!(answer.body["outcome"], json!("planned"), "{}", answer.body);
    payload(&answer.body, "plan_id")
}

/// `RecordPlanResult` of `plan_id` with no violation, as the ReleaseController: `Ready`.
pub fn ready(client: &Client, caller: &Value, plan_id: &str) {
    let answer = client
        .command(
            "weft.deployments.RecordPlanResult",
            RELEASE_CONTROLLER,
            caller.clone(),
            json!({
                "plan_id": plan_id,
                "live_release_id": null,
                "diff": {"added": [], "changed": [], "removed": []},
                "simulation": {"reachability": [], "violations": []},
            }),
        )
        .expect("the control host answers");
    assert_eq!(answer.body["outcome"], json!("passed"), "{}", answer.body);
}

/// `POST /cloudspec/plans/<plan_id>/apply` with strategy `Replace`, as the Deployer.
pub fn apply(client: &Client, caller: &Value, plan_id: &str) -> Answer {
    client
        .post_bytes_as(
            &format!("/cloudspec/plans/{plan_id}/apply"),
            json!({"strategy": "Replace"}).to_string().as_bytes(),
            DEPLOYER,
            caller.clone(),
        )
        .expect("the control host answers")
}

/// The account path, formation `site` and environment `prod` of a control host, with the process
/// artifact and runtime registered: what an apply of a process part needs.
pub struct ProcessWorld {
    pub caller: Value,
    pub scope_id: String,
    pub formation_id: String,
    pub environment_id: String,
    pub runtime_id: String,
}

impl ProcessWorld {
    /// The world on the control host of `client`.
    pub fn new(client: &Client) -> Self {
        let path = steps::run_path(client);
        let mut caller = steps::path_caller(&path);
        caller["principal_id"] = json!(weft_impl::context::mint());
        let scope_id = path.scope_id.clone();
        let formation_id = steps::create_formation(client, &caller, &scope_id);
        let environment_id = steps::create_environment(client, &caller, &formation_id, &scope_id);
        register_process_artifact(client, &caller, &scope_id);
        let runtime_id = register_process_runtime(client);
        Self {
            caller,
            scope_id,
            formation_id,
            environment_id,
            runtime_id,
        }
    }

    /// `bytes` submitted, planned, made `Ready` and applied: the apply's answer.
    pub fn apply(&self, client: &Client, bytes: &[u8]) -> Answer {
        let source_id = submit(client, &self.caller, &self.formation_id, bytes);
        let plan_id = plan(
            client,
            &self.caller,
            &source_id,
            &self.scope_id,
            &self.environment_id,
        );
        ready(client, &self.caller, &plan_id);
        apply(client, &self.caller, &plan_id)
    }
}

/// The text member `field` of the first published event's payload.
fn payload(body: &Value, field: &str) -> String {
    body["published"][0]["payload"][field]
        .as_str()
        .unwrap_or_else(|| panic!("no {field} in {body}"))
        .to_owned()
}
