//! `weft.workloads.DefineWorkload` (`PLAN.md` § Obligations, `ess/cloud/5-workloads/workloads.yaml`),
//! in the selection precedence of its contract:
//!
//! 1. the input guards in declaration order: `not-callers-scope` (a caller acting in no
//!    scope counts as another scope's), `spec-kind`, `box-package`, `run-as-missing`
//!    (isolation `None` with no host user in `run_as`), `no-main`,
//!    `invalid-check`, `invalid-restart-policy`, and the external `settings-shape-mismatch`
//!    (`before_row`: the tag of `settings` names another shape than `shape`);
//! 2. `no-runtime`, then the predicates on the runtime: `runtime-retired`, `runtime-kind`,
//!    `below-runtime-isolation` (story:cloud-blocks-layer-refactor-pin: the checks of the
//!    formation, environment, revision and component are `weft.architecture.AssignWorkload`'s,
//!    the workload names only its component-level `identity`);
//! 3. the external branches ahead of `defined`, decided from the `weft.artifacts.Artifact` row
//!    of the same store: `no-artifact`, `artifact-foreign` (registered to another scope),
//!    `artifact-revoked`, `artifact-kind` (an artifact kind other than the package kind, the
//!    same variant name);
//! 4. the external branch `over-quota` of the outcome group `quota`, decided from the rows of
//!    the same store ([`crate::quota::exceeded_alone`]): the claims of one instance of the
//!    workload alone would take the scope's quota, or a quota above it, past its limit. What the
//!    scope's live instances hold does not count; `CreateInstance` asks for each instance. It
//!    is answered before `defined`, the branch the contract takes "otherwise" (its default);
//! 5. `defined`: a `Defined` workload with the values sent.

use weft_types::artifacts::{ArtifactKind, ArtifactState};
use weft_types::behaviour::{ArtifactStorage, Context, RuntimeStorage, WorkloadStorage};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::{IsolationLevel, WorkloadId};
use weft_types::workloads::obligations::DefineWorkloadBehavior;
use weft_types::workloads::{
    ArtifactIsRevoked, ArtifactKindMismatch, ArtifactNotFound, BoxPackageKind, Check, CheckKind,
    DefineWorkload, DefineWorkloadOutcome, InvalidCheck, InvalidRestartPolicy,
    IsolationBelowMinimum, NoMainProcess, NotFound, PackageKind, ProcessRole, QuotaExceeded,
    RestartPolicy, RunAsMissing, RuntimeIsRetired, RuntimeKindUnsupported, RuntimeNotFound,
    RuntimeState, SettingsShapeMismatch, ShapeKind, ShapeSettings, SpecKindNotRunnable,
    WorkloadData, WorkloadDefined, WorkloadSnapshot, WorkloadState,
};

use crate::context::mint;
use crate::quota;
use crate::Ports;

impl DefineWorkloadBehavior for Ports {
    fn define_workload(
        &mut self,
        input: DefineWorkload,
    ) -> Result<DefineWorkloadOutcome, UnmetObligation> {
        if Context::caller_scope_id(self).as_ref() != Some(&input.scope_id) {
            return Ok(DefineWorkloadOutcome::NotCallersScope { error: NotFound });
        }
        if matches!(
            input.package_kind,
            PackageKind::EssSystem
                | PackageKind::Agent
                | PackageKind::Training
                | PackageKind::Graph
        ) {
            return Ok(DefineWorkloadOutcome::SpecKind {
                error: SpecKindNotRunnable,
            });
        }
        if input.shape == ShapeKind::Box
            && !matches!(
                input.package_kind,
                PackageKind::Image | PackageKind::Machine
            )
        {
            return Ok(DefineWorkloadOutcome::BoxPackage {
                error: BoxPackageKind,
            });
        }
        if input.isolation == IsolationLevel::None && input.run_as.is_none() {
            return Ok(DefineWorkloadOutcome::RunAsMissing {
                error: RunAsMissing,
            });
        }
        if !input
            .processes
            .iter()
            .any(|process| process.role == ProcessRole::Main)
        {
            return Ok(DefineWorkloadOutcome::NoMain {
                error: NoMainProcess,
            });
        }
        if input
            .processes
            .iter()
            .any(|process| process.checks.iter().any(invalid_check))
        {
            return Ok(DefineWorkloadOutcome::InvalidCheck {
                error: InvalidCheck,
            });
        }
        if invalid_restart_policy(&input.restart_policy) {
            return Ok(DefineWorkloadOutcome::InvalidRestartPolicy {
                error: InvalidRestartPolicy,
            });
        }
        if settings_shape(&input.settings) != input.shape {
            return Ok(DefineWorkloadOutcome::SettingsShapeMismatch {
                error: SettingsShapeMismatch,
            });
        }

        let Some(runtime) = RuntimeStorage::get(self, &input.runtime_id) else {
            return Ok(DefineWorkloadOutcome::NoRuntime {
                error: RuntimeNotFound,
            });
        };
        if runtime.state == RuntimeState::Retired {
            return Ok(DefineWorkloadOutcome::RuntimeRetired {
                error: RuntimeIsRetired,
            });
        }
        if !runtime.data.package_kinds.contains(&input.package_kind) {
            return Ok(DefineWorkloadOutcome::RuntimeKind {
                error: RuntimeKindUnsupported,
            });
        }
        if below(input.isolation, runtime.data.minimum_isolation) {
            return Ok(DefineWorkloadOutcome::BelowRuntimeIsolation {
                error: IsolationBelowMinimum,
            });
        }

        let Some(artifact) = ArtifactStorage::get(self, &input.artifact_digest) else {
            return Ok(DefineWorkloadOutcome::NoArtifact {
                error: ArtifactNotFound,
            });
        };
        if artifact.data.scope_id != input.scope_id {
            return Ok(DefineWorkloadOutcome::ArtifactForeign {
                error: ArtifactNotFound,
            });
        }
        if artifact.state == ArtifactState::Revoked {
            return Ok(DefineWorkloadOutcome::ArtifactRevoked {
                error: ArtifactIsRevoked,
            });
        }
        if artifact.data.kind != artifact_kind(input.package_kind) {
            return Ok(DefineWorkloadOutcome::ArtifactKind {
                error: ArtifactKindMismatch,
            });
        }
        if quota::exceeded_alone(self, &input.scope_id, &input.claims) {
            return Ok(DefineWorkloadOutcome::OverQuota {
                error: QuotaExceeded,
            });
        }

        let workload_id = WorkloadId(Uuid(mint()));
        let defined = WorkloadDefined {
            workload_id: workload_id.clone(),
            scope_id: input.scope_id.clone(),
            identity: input.identity.clone(),
            component: input.component.clone(),
            package_kind: input.package_kind,
            artifact_digest: input.artifact_digest.clone(),
            shape: input.shape,
            isolation: input.isolation,
        };
        WorkloadStorage::put(
            self,
            WorkloadSnapshot {
                state: WorkloadState::Defined,
                data: WorkloadData {
                    workload_id,
                    scope_id: input.scope_id,
                    identity: input.identity,
                    component: input.component,
                    package_kind: input.package_kind,
                    artifact_digest: input.artifact_digest,
                    runtime_id: input.runtime_id,
                    isolation: input.isolation,
                    run_as: input.run_as,
                    upgrade: input.upgrade,
                    volumes: input.volumes,
                    ports: input.ports,
                    limits: input.limits,
                    process_priority: input.process_priority,
                    stop: input.stop,
                    prestart: input.prestart,
                    restart: input.restart,
                    environment_files: input.environment_files,
                    shape: input.shape,
                    settings: input.settings,
                    claims: input.claims,
                    processes: input.processes,
                    restart_policy: input.restart_policy,
                    priority: input.priority,
                },
            },
        );
        Ok(DefineWorkloadOutcome::Defined {
            workload_defined: defined,
        })
    }
}

/// The check of `invalid-check`: a check that misses what its kind needs, or whose port,
/// lengths or thresholds are out of range.
fn invalid_check(check: &Check) -> bool {
    let needs_port = matches!(
        check.kind,
        CheckKind::Http | CheckKind::Tcp | CheckKind::Grpc
    );
    (needs_port && check.port.is_none())
        || (check.kind == CheckKind::Http && check.path.is_none())
        || (check.kind == CheckKind::Command && check.command.is_none())
        || check.port.is_some_and(|port| !(1..=65535).contains(&port))
        || check.initial_delay_seconds < 0
        || check.period_seconds < 1
        || check.timeout_seconds < 1
        || check.failure_threshold < 1
        || check.success_threshold < 1
}

/// The check of `invalid-restart-policy`.
fn invalid_restart_policy(policy: &RestartPolicy) -> bool {
    policy.backoff_initial_seconds < 1
        || policy.backoff_max_seconds < policy.backoff_initial_seconds
        || policy.backoff_limit < 1
        || policy.backoff_window_seconds < 1
}

/// The shape the tag of `settings` names (the check of `settings-shape-mismatch`).
fn settings_shape(settings: &ShapeSettings) -> ShapeKind {
    match settings {
        ShapeSettings::Box(_) => ShapeKind::Box,
        ShapeSettings::Daemon(_) => ShapeKind::Daemon,
        ShapeSettings::Invocation(_) => ShapeKind::Invocation,
        ShapeSettings::Job(_) => ShapeKind::Job,
        ShapeSettings::Schedule(_) => ShapeKind::Schedule,
        ShapeSettings::Service(_) => ShapeKind::Service,
        ShapeSettings::Session(_) => ShapeKind::Session,
    }
}

/// The predicate of `below-runtime-isolation`: `isolation` is weaker than the runtime's
/// `minimum`, as the specification lists the weaker levels for each minimum (`None`, no sandbox,
/// is the weakest).
fn below(isolation: IsolationLevel, minimum: IsolationLevel) -> bool {
    match minimum {
        IsolationLevel::MicroVm => isolation != IsolationLevel::MicroVm,
        IsolationLevel::UserKernel => !matches!(
            isolation,
            IsolationLevel::MicroVm | IsolationLevel::UserKernel
        ),
        IsolationLevel::Container => matches!(
            isolation,
            IsolationLevel::ProcessSandbox | IsolationLevel::None
        ),
        IsolationLevel::ProcessSandbox => isolation == IsolationLevel::None,
        IsolationLevel::None => false,
    }
}

/// The artifact kind of the same name as `kind`.
fn artifact_kind(kind: PackageKind) -> ArtifactKind {
    match kind {
        PackageKind::Snippet => ArtifactKind::Snippet,
        PackageKind::Bundle => ArtifactKind::Bundle,
        PackageKind::Wasm => ArtifactKind::Wasm,
        PackageKind::Image => ArtifactKind::Image,
        PackageKind::Process => ArtifactKind::Process,
        PackageKind::Machine => ArtifactKind::Machine,
        PackageKind::Model => ArtifactKind::Model,
        PackageKind::Browser => ArtifactKind::Browser,
        PackageKind::Build => ArtifactKind::Build,
        PackageKind::EssSystem => ArtifactKind::EssSystem,
        PackageKind::Agent => ArtifactKind::Agent,
        PackageKind::Training => ArtifactKind::Training,
        PackageKind::Graph => ArtifactKind::Graph,
        PackageKind::WebBundle => ArtifactKind::WebBundle,
        PackageKind::BuildContext => ArtifactKind::BuildContext,
    }
}
