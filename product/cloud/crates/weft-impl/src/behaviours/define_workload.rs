//! `weft.workloads.DefineWorkload` (`PLAN.md` § Obligations, `ess/4-applications/workloads.yaml`),
//! in the selection precedence of its contract:
//!
//! 1. the input guards in declaration order: `not-callers-account` (a caller acting in no
//!    account counts as another account's), `spec-kind`, `box-package`, `no-main`,
//!    `invalid-check`, `invalid-restart-policy`, and the external `settings-shape-mismatch`
//!    (`before_row`: the tag of `settings` names another shape than `shape`);
//! 2. every existence check before every predicate, in declaration order:
//!    `formation-missing`, `environment-missing`, `revision-missing`, `no-runtime`,
//!    `component-missing` (no `weft.formations.Component` of the formation has the name);
//! 3. the predicates on those rows: `formation-foreign`, `environment-foreign`,
//!    `revision-foreign`, `formation-retired`, `environment-retired`, `runtime-retired`,
//!    `runtime-kind`, `below-runtime-isolation`;
//! 4. the external branches ahead of `defined`, decided from the `weft.artifacts.Artifact` row
//!    of the same store: `no-artifact`, `artifact-foreign` (registered to another account),
//!    `artifact-revoked`, `artifact-kind` (an artifact kind other than the package kind, the
//!    same variant name);
//! 5. the external branch `over-quota` of the outcome group `quota`, decided from the rows of
//!    the same store ([`crate::quota::exceeded_alone`]): the claims of one instance of the
//!    workload alone would take the account, or an account above it, past a quota. What the
//!    account's live instances hold does not count; `CreateInstance` asks for each instance. It
//!    is answered before `defined`, the branch the contract takes "otherwise" (its default);
//! 6. `defined`: a `Defined` workload with the values sent.

use weft_types::artifacts::{ArtifactKind, ArtifactState};
use weft_types::behaviour::{
    ArtifactStorage, ComponentStorage, Context, EnvironmentStorage, FormationRevisionStorage,
    FormationStorage, RuntimeStorage, WorkloadStorage,
};
use weft_types::formations::{EnvironmentState, FormationState};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::{IsolationLevel, WorkloadId};
use weft_types::workloads::obligations::DefineWorkloadBehavior;
use weft_types::workloads::{
    ArtifactIsRevoked, ArtifactKindMismatch, ArtifactNotFound, BoxPackageKind, Check, CheckKind,
    DefineWorkload, DefineWorkloadOutcome, InvalidCheck, InvalidRestartPolicy,
    IsolationBelowMinimum, NoMainProcess, NotFound, PackageKind, ProcessRole, QuotaExceeded,
    RestartPolicy, RuntimeIsRetired, RuntimeKindUnsupported, RuntimeNotFound, RuntimeState,
    SettingsShapeMismatch, ShapeKind, ShapeSettings, SpecKindNotRunnable, StateConflict,
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
        if Context::caller_account_id(self).as_ref() != Some(&input.account_id) {
            return Ok(DefineWorkloadOutcome::NotCallersAccount { error: NotFound });
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

        let Some(formation) = FormationStorage::get(self, &input.formation_id) else {
            return Ok(DefineWorkloadOutcome::FormationMissing { error: NotFound });
        };
        let Some(environment) = EnvironmentStorage::get(self, &input.environment_id) else {
            return Ok(DefineWorkloadOutcome::EnvironmentMissing { error: NotFound });
        };
        let Some(revision) = FormationRevisionStorage::get(self, &input.formation_revision_id)
        else {
            return Ok(DefineWorkloadOutcome::RevisionMissing { error: NotFound });
        };
        let Some(runtime) = RuntimeStorage::get(self, &input.runtime_id) else {
            return Ok(DefineWorkloadOutcome::NoRuntime {
                error: RuntimeNotFound,
            });
        };
        if !ComponentStorage::list(self).iter().any(|row| {
            row.data.formation_id == input.formation_id && row.data.name == input.component
        }) {
            return Ok(DefineWorkloadOutcome::ComponentMissing { error: NotFound });
        }

        if formation.data.account_id != input.account_id {
            return Ok(DefineWorkloadOutcome::FormationForeign { error: NotFound });
        }
        if environment.data.formation_id != input.formation_id {
            return Ok(DefineWorkloadOutcome::EnvironmentForeign { error: NotFound });
        }
        if revision.data.formation_id != input.formation_id {
            return Ok(DefineWorkloadOutcome::RevisionForeign { error: NotFound });
        }
        if formation.state == FormationState::Retired {
            return Ok(DefineWorkloadOutcome::FormationRetired {
                error: StateConflict,
            });
        }
        if environment.state == EnvironmentState::Retired {
            return Ok(DefineWorkloadOutcome::EnvironmentRetired {
                error: StateConflict,
            });
        }
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
        if artifact.data.account_id != input.account_id {
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
        if quota::exceeded_alone(self, &input.account_id, &input.claims) {
            return Ok(DefineWorkloadOutcome::OverQuota {
                error: QuotaExceeded,
            });
        }

        let workload_id = WorkloadId(Uuid(mint()));
        let defined = WorkloadDefined {
            workload_id: workload_id.clone(),
            account_id: input.account_id.clone(),
            formation_id: input.formation_id.clone(),
            environment_id: input.environment_id.clone(),
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
                    account_id: input.account_id,
                    formation_id: input.formation_id,
                    formation_revision_id: input.formation_revision_id,
                    environment_id: input.environment_id,
                    component: input.component,
                    package_kind: input.package_kind,
                    artifact_digest: input.artifact_digest,
                    runtime_id: input.runtime_id,
                    isolation: input.isolation,
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
/// `minimum`, as the specification lists the weaker levels for each minimum.
fn below(isolation: IsolationLevel, minimum: IsolationLevel) -> bool {
    match minimum {
        IsolationLevel::MicroVm => isolation != IsolationLevel::MicroVm,
        IsolationLevel::UserKernel => !matches!(
            isolation,
            IsolationLevel::MicroVm | IsolationLevel::UserKernel
        ),
        IsolationLevel::Container => isolation == IsolationLevel::ProcessSandbox,
        IsolationLevel::ProcessSandbox => false,
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
    }
}
