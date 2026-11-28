//! Applying the plan of a cloudspec source (`story:cloudspec-submission-apply-workloads`, owner
//! decision D-C1 = A): `POST /cloudspec/plans/<plan_id>/apply` with the body
//! `{"strategy": "<RolloutStrategy>"}`, sent by the caller as every request is
//! ([`weft_bin::authenticate`]).
//!
//! The control host defines one workload per part with a fixed profile per part kind: the
//! snippet's ([`SNIPPET`]) and the host process's ([`PROCESS`], `story:slot-node-process`: shape
//! `Daemon`, isolation `None`, and the part's `run_as`, `prestart`, `stop` and `restart`). In one
//! unit ([`Host::unit`]):
//!
//! - the plan must name a source, else `422` `{"refusal": "PlanNotFromCloudspec"}` and no
//!   command runs (a plan the caller's scope does not hold is left to `ApplyPlan`'s refusal);
//! - the file kept under the source's digest ([`weft_impl::documents`]) is read with
//!   [`Submission::read`];
//! - `weft.deployments.ApplyPlan`, as the caller;
//! - for each part with `block: workloads` and `kind: snippet` or `kind: process` whose
//!   `parts.<key>` the coverage does not mark `refused`, in key order, as the caller:
//!   `weft.architecture.AddComponent` named
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
    /// `weft.workloads.ShapeKind` with its settings.
    pub shape: Shape,
    /// `weft.workloads.RestartMode` of the restart policy, when the part names no `restart`.
    pub restart_mode: &'static str,
    /// `weft.shared.PriorityClassName`.
    pub priority: &'static str,
    /// The name of the one `Main` process, which runs the part's `command` and `environment`.
    pub process: &'static str,
    /// A host process: the part's `run_as`, `prestart`, `stop` and `restart` are the workload's,
    /// and its `restart` sets the restart policy ([`restart_policy`]).
    pub host_process: bool,
}

/// The shape of a profile's workloads (`weft.workloads.ShapeKind`) and its settings
/// (`weft.workloads.ShapeSettings`).
pub enum Shape {
    /// `Job`, with `JobSettings`: `parallelism`, `gang`, `max_retries`, `backoff`.
    Job {
        parallelism: i64,
        gang: bool,
        max_retries: i64,
        backoff: &'static str,
    },
    /// `Daemon`, with `DaemonSettings` selecting every node (`node_selector` empty): which node
    /// runs an instance is placement's (`story:slot-node-pool-placement`).
    Daemon,
}

/// The snippet profile (D-C1 = A).
pub const SNIPPET: Profile = Profile {
    part_kind: "snippet",
    runtime: "snippet",
    artifact_digest: "sha256:weft-skeleton-snippet",
    package_kind: "Snippet",
    isolation: "Container",
    cpu: 1,
    shape: Shape::Job {
        parallelism: 1,
        gang: false,
        max_retries: 0,
        backoff: "PT1S",
    },
    restart_mode: "Never",
    priority: "standard",
    process: "main",
    host_process: false,
};

/// The process profile (`story:slot-node-process`): a program run on its node's host as a
/// supervised `Daemon`, with no sandbox (isolation `None`, which `DefineWorkload` takes only with
/// a named host user, `run_as`), on the runtime `process`.
pub const PROCESS: Profile = Profile {
    part_kind: "process",
    runtime: "process",
    artifact_digest: "sha256:weft-skeleton-process",
    package_kind: "Process",
    isolation: "None",
    cpu: 1,
    shape: Shape::Daemon,
    restart_mode: "Never",
    priority: "standard",
    process: "main",
    host_process: true,
};

/// The profiles of the part kinds an apply defines workloads of.
const PROFILES: [&Profile; 2] = [&SNIPPET, &PROCESS];

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
    let controller = CallerAttributes {
        scope_id: Some(scope_id.clone()),
        ..CallerAttributes::default()
    };
    let mut rolled = Vec::new();
    let mut profiled: Vec<(String, Value, &Profile, String)> = Vec::new();
    for profile in PROFILES {
        let runtime_id = runtime(sender.ports, profile.runtime);
        for (key, part) in parts(submission, profile) {
            profiled.push((key, part, profile, runtime_id.clone()));
        }
    }
    profiled.sort_by(|a, b| a.0.cmp(&b.0));
    for (position, (key, part, profile, runtime_id)) in (1_i64..).zip(profiled) {
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
                define_workload(profile, scope_id, &identity, &key, &runtime_id, &part),
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
    let (shape, settings) = match profile.shape {
        Shape::Job {
            parallelism,
            gang,
            max_retries,
            backoff,
        } => (
            "Job",
            json!({
                "shape": "job",
                "value": {
                    "parallelism": parallelism,
                    "gang": gang,
                    "max_retries": max_retries,
                    "backoff": backoff,
                },
            }),
        ),
        Shape::Daemon => (
            "Daemon",
            json!({"shape": "daemon", "value": {"node_selector": {}}}),
        ),
    };
    let mut input = json!({
        "scope_id": scope_id,
        "identity": identity,
        "component": key,
        "package_kind": profile.package_kind,
        "artifact_digest": profile.artifact_digest,
        "runtime_id": runtime_id,
        "isolation": profile.isolation,
        "shape": shape,
        "settings": settings,
        "claims": [{"name": "cpu", "class": "cpu", "amount": profile.cpu, "selector": {}}],
        "processes": [{
            "name": profile.process,
            "role": "Main",
            "command": part.get("command").cloned().unwrap_or_else(|| json!([])),
            "environment": environment(part.get("environment")),
            "checks": [],
        }],
        "restart_policy": restart_policy(profile, part),
        "priority": profile.priority,
    });
    if profile.host_process {
        input["run_as"] = part.get("run_as").cloned().unwrap_or(Value::Null);
        input["prestart"] = prestart(part.get("prestart"));
        input["stop"] = stop(part.get("stop"));
        input["restart"] = restart(part.get("restart"));
    }
    input
}

/// The part's `environment` as `weft.workloads.Process.environment` takes it: each value as its
/// text, a number or a Boolean as JSON writes it (cloudspec writes `POOLD_SLOTS: 8`).
fn environment(environment: Option<&Value>) -> Value {
    let Some(Value::Object(variables)) = environment else {
        return json!({});
    };
    variables
        .iter()
        .map(|(name, value)| {
            let text = match value {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            (name.clone(), Value::String(text))
        })
        .collect::<serde_json::Map<_, _>>()
        .into()
}

/// The restart policy (`weft.workloads.RestartPolicy`) of a part: the profile's mode, or for a
/// host process the mode of its `restart.policy`, with its `delay` (whole seconds, at least 1) as
/// the initial backoff, so the node's report of a restart after that delay lies within the
/// policy (`weft.node.RestartProcess`, `backoff-out-of-range`).
fn restart_policy(profile: &Profile, part: &Value) -> Value {
    let restart = part.get("restart").filter(|_| profile.host_process);
    let mode = restart
        .and_then(|restart| restart.get("policy"))
        .and_then(Value::as_str)
        .map_or_else(|| profile.restart_mode.to_owned(), variant);
    let initial = restart
        .and_then(|restart| restart.get("delay"))
        .and_then(Value::as_str)
        .and_then(seconds)
        .unwrap_or(1)
        .max(1);
    json!({
        "mode": mode,
        "backoff_initial_seconds": initial,
        "backoff_max_seconds": initial.max(60),
        "backoff_limit": 5,
        "backoff_window_seconds": 600,
    })
}

/// A part's `prestart` as `weft.workloads.PrestartStep`s, in list order: each step's `command`
/// and `privileged` unchanged.
fn prestart(prestart: Option<&Value>) -> Value {
    let Some(Value::Array(steps)) = prestart else {
        return Value::Null;
    };
    steps
        .iter()
        .map(|step| json!({"command": step.get("command"), "privileged": step.get("privileged")}))
        .collect()
}

/// A part's `stop` as `weft.workloads.Stop`: the `grace` unchanged, the `kill` scope by its
/// variant name (`group` is `Group`).
fn stop(stop: Option<&Value>) -> Value {
    let Some(stop) = stop.filter(|stop| stop.is_object()) else {
        return Value::Null;
    };
    json!({
        "grace": stop.get("grace"),
        "kill": stop.get("kill").and_then(Value::as_str).map(variant),
    })
}

/// A part's `restart` as `weft.workloads.ProcessRestart`: the `policy` by its variant name
/// (`always` is `Always`, `on-failure` is `OnFailure`), `delay` and `keep_down_on_exit`
/// unchanged.
fn restart(restart: Option<&Value>) -> Value {
    let Some(restart) = restart.filter(|restart| restart.is_object()) else {
        return Value::Null;
    };
    json!({
        "policy": restart.get("policy").and_then(Value::as_str).map(variant),
        "delay": restart.get("delay"),
        "keep_down_on_exit": restart.get("keep_down_on_exit"),
    })
}

/// The variant name of a cloudspec word: each word of `on-failure`, `on_failure` or `group`
/// capitalized and joined (`OnFailure`, `Group`); a variant name stays as it is.
fn variant(word: &str) -> String {
    word.split(['-', '_'])
        .map(|piece| {
            let mut chars = piece.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect()
}

/// The seconds of a duration `<n>s`, `<n>m` or `<n>h` (`n` a whole number), as cloudspec writes
/// `stop.grace` and `restart.delay`.
fn seconds(duration: &str) -> Option<i64> {
    let (number, scale) = if let Some(number) = duration.strip_suffix('s') {
        (number, 1)
    } else if let Some(number) = duration.strip_suffix('m') {
        (number, 60)
    } else {
        (duration.strip_suffix('h')?, 3600)
    };
    number.parse::<i64>().ok()?.checked_mul(scale)
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
