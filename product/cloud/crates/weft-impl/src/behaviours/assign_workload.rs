//! `weft.architecture.AssignWorkload` (`PLAN.md` § Obligations,
//! `ess/cloud/9-composition/architecture.yaml`; story:cloud-blocks-layer-refactor-pin: the
//! refusals of weft's `DefineWorkload` that read the composition), in the selection precedence
//! of its contract:
//!
//! 1. the input guard `not-callers-scope` (a caller acting in no scope counts as another
//!    scope's);
//! 2. every existence check before every predicate, in declaration order: `no-such-formation`,
//!    `no-such-environment`, `no-such-revision`, `no-such-component`, `no-such-workload`;
//! 3. the predicates on those rows: `formation-of-other-scope`,
//!    `environment-of-other-formation`, `revision-of-other-formation`,
//!    `component-of-other-formation`, `workload-of-other-scope`, `formation-retired`,
//!    `environment-retired`;
//! 4. `already-assigned`: a `weft.architecture.ComponentWorkload` of the revision, environment
//!    and component exists;
//! 5. `assigned`: an `Assigned` row that ties the workload to the component, revision and
//!    environment.

use weft_types::architecture::obligations::AssignWorkloadBehavior;
use weft_types::architecture::{
    AssignWorkload, AssignWorkloadOutcome, ComponentWorkloadData, ComponentWorkloadSnapshot,
    ComponentWorkloadState, EnvironmentState, EnvironmentStateConflict, FormationState,
    FormationStateConflict, NoSuchComponent, NoSuchEnvironment, NoSuchFormation, NoSuchRevision,
    NoSuchWorkload, WorkloadAlreadyAssigned, WorkloadAssigned,
};
use weft_types::behaviour::{
    ComponentStorage, ComponentWorkloadStorage, Context, EnvironmentStorage,
    FormationRevisionStorage, FormationStorage, WorkloadStorage,
};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::ComponentWorkloadId;

use crate::context::mint;
use crate::Ports;

impl AssignWorkloadBehavior for Ports {
    fn assign_workload(
        &mut self,
        input: AssignWorkload,
    ) -> Result<AssignWorkloadOutcome, UnmetObligation> {
        if Context::caller_scope_id(self).as_ref() != Some(&input.scope_id) {
            return Ok(AssignWorkloadOutcome::NotCallersScope {
                error: NoSuchFormation,
            });
        }
        let Some(formation) = FormationStorage::get(self, &input.formation_id) else {
            return Ok(AssignWorkloadOutcome::NoSuchFormation {
                error: NoSuchFormation,
            });
        };
        let Some(environment) = EnvironmentStorage::get(self, &input.environment_id) else {
            return Ok(AssignWorkloadOutcome::NoSuchEnvironment {
                error: NoSuchEnvironment,
            });
        };
        let Some(revision) = FormationRevisionStorage::get(self, &input.formation_revision_id)
        else {
            return Ok(AssignWorkloadOutcome::NoSuchRevision {
                error: NoSuchRevision,
            });
        };
        let Some(component) = ComponentStorage::get(self, &input.component_id) else {
            return Ok(AssignWorkloadOutcome::NoSuchComponent {
                error: NoSuchComponent,
            });
        };
        let Some(workload) = WorkloadStorage::get(self, &input.workload_id) else {
            return Ok(AssignWorkloadOutcome::NoSuchWorkload {
                error: NoSuchWorkload,
            });
        };
        if formation.data.scope_id != input.scope_id {
            return Ok(AssignWorkloadOutcome::FormationOfOtherScope {
                error: NoSuchFormation,
            });
        }
        if environment.data.formation_id != input.formation_id {
            return Ok(AssignWorkloadOutcome::EnvironmentOfOtherFormation {
                error: NoSuchEnvironment,
            });
        }
        if revision.data.formation_id != input.formation_id {
            return Ok(AssignWorkloadOutcome::RevisionOfOtherFormation {
                error: NoSuchRevision,
            });
        }
        if component.data.formation_id != input.formation_id {
            return Ok(AssignWorkloadOutcome::ComponentOfOtherFormation {
                error: NoSuchComponent,
            });
        }
        if workload.data.scope_id != input.scope_id {
            return Ok(AssignWorkloadOutcome::WorkloadOfOtherScope {
                error: NoSuchWorkload,
            });
        }
        if formation.state == FormationState::Retired {
            return Ok(AssignWorkloadOutcome::FormationRetired {
                error: FormationStateConflict,
            });
        }
        if environment.state == EnvironmentState::Retired {
            return Ok(AssignWorkloadOutcome::EnvironmentRetired {
                error: EnvironmentStateConflict,
            });
        }
        if ComponentWorkloadStorage::list(self).iter().any(|row| {
            row.data.formation_revision_id == input.formation_revision_id
                && row.data.environment_id == input.environment_id
                && row.data.component_id == input.component_id
        }) {
            return Ok(AssignWorkloadOutcome::AlreadyAssigned {
                error: WorkloadAlreadyAssigned,
            });
        }
        let component_workload_id = ComponentWorkloadId(Uuid(mint()));
        let data = ComponentWorkloadData {
            component_workload_id: component_workload_id.clone(),
            formation_id: input.formation_id.clone(),
            formation_revision_id: input.formation_revision_id.clone(),
            environment_id: input.environment_id.clone(),
            component_id: input.component_id.clone(),
            workload_id: input.workload_id.clone(),
        };
        ComponentWorkloadStorage::put(
            self,
            ComponentWorkloadSnapshot {
                state: ComponentWorkloadState::Assigned,
                data,
            },
        );
        Ok(AssignWorkloadOutcome::Assigned {
            workload_assigned: WorkloadAssigned {
                component_workload_id,
                formation_id: input.formation_id,
                formation_revision_id: input.formation_revision_id,
                environment_id: input.environment_id,
                component_id: input.component_id,
                workload_id: input.workload_id,
            },
        })
    }
}
