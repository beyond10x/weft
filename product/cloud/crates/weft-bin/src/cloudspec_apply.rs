//! Applying the plan of a cloudspec source (`story:cloudspec-submission-apply-workloads`, owner
//! decision D-C1 = A): `POST /cloudspec/plans/<plan_id>/apply` with the body
//! `{"strategy": "<RolloutStrategy>"}`, sent by the caller as every request is
//! ([`weft_bin::authenticate`]).
//!
//! The control host defines one workload per part with a fixed profile per part kind, here the
//! snippet's ([`SNIPPET`]). In one unit ([`Host::unit`]):
//!
//! - the plan must name a source, else `422` `{"refusal": "PlanNotFromCloudspec"}` and no
//!   command runs (a plan the caller's scope does not hold is left to `ApplyPlan`'s refusal);
//! - the file kept under the source's digest ([`weft_impl::documents`]) is read with
//!   [`Submission::read`];
//! - `weft.deployments.ApplyPlan`, as the caller;
//! - for each part with `block: workloads` and `kind: snippet` whose `parts.<key>` the coverage
//!   does not mark `refused`, in key order, as the caller: `weft.architecture.AddComponent` named
//!   after the key (a component of the formation with that name is reused),
//!   `weft.workloads.DefineWorkload` with the profile, `weft.architecture.AssignWorkload` to the
//!   plan's revision and environment; then, as `weft.actors.ReleaseController` of the plan's
//!   scope, `weft.deployments.AddReleaseStep` (`Rollout`, position n, the key, 1 replica);
//! - as the controller, `weft.deployments.StartRelease` and `weft.deployments.StartReleaseStep`
//!   of position 1 with its workload. With no such part the release stays `Planned`.
//!
//! A command that refuses makes the unit commit nothing, and the route answers its refusal.
//! Otherwise the route answers `202` `{"outcome": "applied", "release_id", "parts": [...]}`.
//! Steps after position 1 stay `Pending`: nothing completes a rollout step of a Job yet.

use std::collections::BTreeSet;

use serde_json::{json, Value};
use weft_bin::{Host, Unit};
use weft_cloudspec::Submission;
use weft_impl::context::{mint, CallerAttributes};
use weft_impl::Ports;
use weft_server::http::{self, Request, Response};
use weft_server::{weft_api, weft_control};
use weft_types::actor::{Actor, Caller};
use weft_types::behaviour::{
    ComponentStorage, DeploymentSourceStorage, PlanStorage, RuntimeStorage,
};
use weft_types::deployments::PlanSnapshot;
use weft_types::primitives::Uuid;
use weft_types::shared::PlanId;
use weft_types::workloads::RuntimeState;

/// The path of an apply is `<PLANS><plan_id><APPLY>`.
const PLANS: &str = "/cloudspec/plans/";
const APPLY: &str = "/apply";

/// The profile weft gives every workload of one part kind.
pub struct Profile {
    /// The part kind (`kind` of a part with `block: workloads`).
    pub part_kind: &'static str,
    /// The name of the runtime: the registered one of this name runs the workload.
    pub runtime: &'static str,
    /// The artifact (the skeleton's `ARTIFACT_DIGEST`).
    pub artifact_digest: &'static str,
    /// `weft.workloads.PackageKind`.
    pub package_kind: &'static str,
    /// `weft.shared.IsolationLevel`.
    pub isolation: &'static str,
    /// The cpu claimed.
    pub cpu: i64,
    /// `weft.workloads.ShapeKind`, `Job`, with `JobSettings`:
    pub shape: &'static str,
    /// `parallelism`,
    pub parallelism: i64,
    /// `gang`,
    pub gang: bool,
    /// `max_retries`,
    pub max_retries: i64,
    /// `backoff`.
    pub backoff: &'static str,
    /// `weft.workloads.RestartMode`.
    pub restart_mode: &'static str,
    /// `weft.shared.PriorityClassName`.
    pub priority: &'static str,
    /// The name of the one `Main` process, which runs the part's `command` and `environment`.
    pub process: &'static str,
}

/// The snippet profile (D-C1 = A).
pub const SNIPPET: Profile = Profile {
    part_kind: "snippet",
    runtime: "snippet",
    artifact_digest: "sha256:weft-skeleton-snippet",
    package_kind: "Snippet",
    isolation: "Container",
    cpu: 1,
    shape: "Job",
    parallelism: 1,
    gang: false,
    max_retries: 0,
    backoff: "PT1S",
    restart_mode: "Never",
    priority: "standard",
    process: "main",
};

/// The plan an apply path names: `Some` for `/cloudspec/plans/<plan_id>/apply`.
pub fn plan_of(path: &str) -> Option<&str> {
    path.strip_prefix(PLANS)?
        .strip_suffix(APPLY)
        .filter(|id| !id.is_empty() && !id.contains('/'))
}

/// The answer to an apply of `plan_id`, or the endpoint's own refusal (logged by the caller).
pub fn apply(
    host: &Host,
    request: &Request,
    plan_id: &str,
    caller: Option<&Caller>,
    attributes: CallerAttributes,
) -> Result<Response, Response> {
    let strategy = serde_json::from_slice::<Value>(&request.body)
        .ok()
        .and_then(|body| {
            body.get("strategy")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .ok_or_else(|| {
            Response::refusal(
                400,
                "an apply names its strategy: {\"strategy\": \"Replace\"}",
            )
        })?;
    let scope_id = attributes
        .scope_id
        .clone()
        .ok_or_else(|| Response::refusal(403, "the caller acts in no scope"))?;
    let ports = host.ports();
    host.unit(|unit| {
        let plan = PlanStorage::get(ports, &PlanId(Uuid(plan_id.to_owned())))
            .filter(|plan| plan.data.scope_id.0 .0 == scope_id);
        let submission = match &plan {
            Some(plan) if plan.data.source_id.is_none() => {
                return Err(Response::new(
                    422,
                    http::JSON,
                    json!({"refusal": "PlanNotFromCloudspec"}).to_string(),
                ));
            }
            Some(plan) => Some(source_file(ports, plan)?),
            None => None,
        };
        let mut sender = Sender {
            unit,
            ports,
            caller,
            attributes,
        };
        let applied = sender.send(
            "weft.deployments.ApplyPlan",
            json!({"plan_id": plan_id, "scope_id": scope_id, "strategy": strategy}),
        )?;
        let (Some(plan), Some(submission)) = (plan, submission) else {
            sender.unit.abort();
            return Err(Response::unfinished(
                "ApplyPlan accepted a plan the caller's scope does not hold",
                false,
            ));
        };
        let release_id = published(&applied, "release_id");
        roll_out(&mut sender, &plan, &submission, &release_id)
    })
}

/// Defines, assigns and steps every snippet part of `submission`, then starts the release and
/// its first step: the route's answer, or the refusal of the command that refused.
fn roll_out(
    sender: &mut Sender<'_, '_, '_>,
    plan: &PlanSnapshot,
    submission: &Submission,
    release_id: &str,
) -> Result<Response, Response> {
    let plan = &plan.data;
    let scope_id = &plan.scope_id.0 .0;
    let formation_id = &plan.formation_id.0 .0;
    let environment_id = &plan.environment_id.0 .0;
    let runtime_id = runtime(sender.ports, SNIPPET.runtime);
    let controller = CallerAttributes {
        scope_id: Some(scope_id.clone()),
        ..CallerAttributes::default()
    };
    let mut rolled = Vec::new();
    for (position, (key, part)) in (1_i64..).zip(parts(submission, &SNIPPET)) {
        let component_id = match component(sender.ports, formation_id, &key) {
            Some(id) => id,
            None => published(
                &sender.send(
                    "weft.architecture.AddComponent",
                    json!({"formation_id": formation_id, "scope_id": scope_id, "name": key}),
                )?,
                "component_id",
            ),
        };
        let identity = format!("{scope_id}/{formation_id}/{environment_id}/{key}");
        let workload_id = published(
            &sender.send(
                "weft.workloads.DefineWorkload",
                define_workload(&SNIPPET, scope_id, &identity, &key, &runtime_id, &part),
            )?,
            "workload_id",
        );
        sender.send(
            "weft.architecture.AssignWorkload",
            json!({
                "formation_id": formation_id,
                "scope_id": scope_id,
                "formation_revision_id": plan.formation_revision_id.0 .0,
                "environment_id": environment_id,
                "component_id": component_id,
                "workload_id": workload_id,
            }),
        )?;
        let step_id = published(
            &sender.as_controller(
                &controller,
                "weft.deployments.AddReleaseStep",
                json!({
                    "release_id": release_id,
                    "position": position,
                    "kind": "Rollout",
                    "component": key,
                    "replicas": 1,
                }),
            )?,
            "step_id",
        );
        rolled.push(json!({
            "part": key,
            "component_id": component_id,
            "workload_id": workload_id,
            "step_id": step_id,
        }));
    }
    if let Some(first) = rolled.first() {
        sender.as_controller(
            &controller,
            "weft.deployments.StartRelease",
            json!({"release_id": release_id}),
        )?;
        sender.as_controller(
            &controller,
            "weft.deployments.StartReleaseStep",
            json!({"step_id": first["step_id"], "job_id": null, "workload_id": first["workload_id"]}),
        )?;
    }
    Ok(Response::new(
        202,
        http::JSON,
        json!({"outcome": "applied", "release_id": release_id, "parts": rolled}).to_string(),
    ))
}

/// The commands of one apply, sent in its unit.
struct Sender<'u, 'a, 'h> {
    unit: &'u mut Unit<'a>,
    ports: &'h Ports,
    caller: Option<&'h Caller>,
    attributes: CallerAttributes,
}

impl Sender<'_, '_, '_> {
    /// `command` (a qualified name) with `input`, as the request's caller: its answer when it
    /// accepted; otherwise the unit commits nothing and the refusal is the error.
    fn send(&mut self, command: &str, input: Value) -> Result<Response, Response> {
        let attributes = self.attributes.clone();
        self.run(self.caller.copied(), attributes, command, input)
    }

    /// `command` as `weft.actors.ReleaseController` with `attributes`, as [`Sender::send`].
    fn as_controller(
        &mut self,
        attributes: &CallerAttributes,
        command: &str,
        input: Value,
    ) -> Result<Response, Response> {
        let controller = Caller {
            actor: Actor::ReleaseController,
        };
        self.run(Some(controller), attributes.clone(), command, input)
    }

    fn run(
        &mut self,
        caller: Option<Caller>,
        attributes: CallerAttributes,
        command: &str,
        input: Value,
    ) -> Result<Response, Response> {
        let short = command.rsplit('.').next().unwrap_or(command);
        let request = Request {
            method: "POST".to_owned(),
            path: format!("/weft/commands/{short}"),
            body: input.to_string().into_bytes(),
            ..Request::default()
        };
        let ports = self.ports;
        let answer = self.unit.run(
            Some(command),
            caller.as_ref().map(|caller| caller.actor.name()),
            |system| {
                ports.set_caller(attributes);
                let answer = if weft_api::ROUTES
                    .iter()
                    .any(|(_, path)| *path == request.path)
                {
                    weft_api::dispatch_with_events(system, caller.as_ref(), &request)
                } else {
                    weft_control::dispatch_with_events(system, caller.as_ref(), &request)
                };
                ports.set_caller(CallerAttributes::default());
                answer
            },
        );
        if (200..300).contains(&answer.status) {
            Ok(answer)
        } else {
            self.unit.abort();
            Err(answer)
        }
    }
}

/// The submission kept for the source `plan` names, or the `501` of a source with no readable
/// file (the submission route keeps the file with the source, so this is no state a caller
/// makes).
fn source_file(ports: &Ports, plan: &PlanSnapshot) -> Result<Submission, Response> {
    let unreadable = || {
        Response::unfinished(
            "the plan's source has no kept file that reads as a cloudspec submission",
            false,
        )
    };
    let source_id = plan.data.source_id.as_ref().ok_or_else(unreadable)?;
    let source = DeploymentSourceStorage::get(ports, source_id).ok_or_else(unreadable)?;
    let bytes =
        weft_impl::documents::get(ports, &source.data.source_digest).ok_or_else(unreadable)?;
    Submission::read(&bytes).map_err(|_| unreadable())
}

/// The parts of `profile`'s kind in `submission`, by key, without those its coverage marks
/// `refused` (an entry `parts.<key>` with status `refused`).
fn parts(submission: &Submission, profile: &Profile) -> Vec<(String, Value)> {
    let refused: BTreeSet<&str> = submission
        .coverage()
        .get("steps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|step| step.get("entries").and_then(Value::as_array))
        .flatten()
        .filter(|entry| entry.get("status").and_then(Value::as_str) == Some("refused"))
        .filter_map(|entry| entry.get("object").and_then(Value::as_str))
        .collect();
    let mut parts: Vec<(String, Value)> = submission
        .architecture()
        .get("parts")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter(|(key, part)| {
            part.get("block").and_then(Value::as_str) == Some("workloads")
                && part.get("kind").and_then(Value::as_str) == Some(profile.part_kind)
                && !refused.contains(format!("parts.{key}").as_str())
        })
        .map(|(key, part)| (key.clone(), part.clone()))
        .collect();
    parts.sort_by(|a, b| a.0.cmp(&b.0));
    parts
}

/// The registered runtime named `name` (the least identity of several); a new identity, which
/// names no runtime, when none is registered, so `DefineWorkload` answers `no-runtime`.
fn runtime(ports: &Ports, name: &str) -> String {
    RuntimeStorage::list(ports)
        .into_iter()
        .filter(|row| row.state == RuntimeState::Registered && row.data.name == name)
        .map(|row| row.data.runtime_id.0 .0)
        .min()
        .unwrap_or_else(mint)
}

/// The component of `formation_id` named `name`.
fn component(ports: &Ports, formation_id: &str, name: &str) -> Option<String> {
    ComponentStorage::list(ports)
        .into_iter()
        .find(|row| row.data.formation_id.0 .0 == formation_id && row.data.name.0 == name)
        .map(|row| row.data.component_id.0 .0)
}

/// The input of `DefineWorkload` for the part `key` with `profile`.
fn define_workload(
    profile: &Profile,
    scope_id: &str,
    identity: &str,
    key: &str,
    runtime_id: &str,
    part: &Value,
) -> Value {
    json!({
        "scope_id": scope_id,
        "identity": identity,
        "component": key,
        "package_kind": profile.package_kind,
        "artifact_digest": profile.artifact_digest,
        "runtime_id": runtime_id,
        "isolation": profile.isolation,
        "shape": profile.shape,
        "settings": {
            "shape": profile.shape.to_ascii_lowercase(),
            "value": {
                "parallelism": profile.parallelism,
                "gang": profile.gang,
                "max_retries": profile.max_retries,
                "backoff": profile.backoff,
            },
        },
        "claims": [{"name": "cpu", "class": "cpu", "amount": profile.cpu, "selector": {}}],
        "processes": [{
            "name": profile.process,
            "role": "Main",
            "command": part.get("command").cloned().unwrap_or_else(|| json!([])),
            "environment": part.get("environment").cloned().unwrap_or_else(|| json!({})),
            "checks": [],
        }],
        "restart_policy": {
            "mode": profile.restart_mode,
            "backoff_initial_seconds": 1,
            "backoff_max_seconds": 60,
            "backoff_limit": 5,
            "backoff_window_seconds": 600,
        },
        "priority": profile.priority,
    })
}

/// The text member `field` of the first event `answer` published (empty when absent: the
/// command that reads it then refuses).
fn published(answer: &Response, field: &str) -> String {
    serde_json::from_str::<Value>(&answer.body)
        .ok()
        .and_then(|body| {
            body["published"][0]["payload"][field]
                .as_str()
                .map(str::to_owned)
        })
        .unwrap_or_default()
}
