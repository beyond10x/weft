//! `weft.scheduling.SubmitSchedulingRequest` (`PLAN.md` § Obligations,
//! `ess/cloud/6-platform/scheduling.yaml`), in the selection precedence of its contract:
//!
//! 1. the input guard `no-claims`;
//! 2. every existence check before every predicate, in declaration order: `unknown-scope` (no
//!    `weft.scope.Scope` of the identity, in any state), `unknown-priority`, and `unknown-gang`
//!    where a gang is named;
//! 3. the predicates in declaration order: `retired-priority`, `already-requested` (a `Pending`
//!    or `Unschedulable` request of the instance), `gang-closed` (not `Forming`),
//!    `gang-of-other-scope`;
//! 4. the external branches of scope isolation, decided from the rows of the same store:
//!    `no-such-instance` (no `weft.workloads.Instance` of the identity), `instance-of-other-scope`
//!    (the instance's `scope_id` is not the request's), `identity-not-instances` (an `identity`
//!    is given and the instance's workload does not hold it, or the workload row is missing);
//! 5. `requested`: a `Pending` request in the scope, with the priority class's value,
//!    preemption and grace, and `requested_at` from the clock.
//!
//! story:cloud-blocks-layer-refactor-pin: the request names no formation (cloudspec layer
//! rule); `weft.scheduling.PlaceInstance` ties the request to its tenant, its formation (through
//! `weft.architecture.ComponentWorkload`) and its cluster when the scheduler places it.

use weft_types::behaviour::{
    Context, GangStorage, InstanceStorage, PriorityClassStorage, SchedulingRequestStorage,
    ScopeStorage, WorkloadStorage,
};
use weft_types::infrastructure::PriorityClassState;
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::scheduling::obligations::SubmitSchedulingRequestBehavior;
use weft_types::scheduling::{
    GangClosed, GangState, InstanceAlreadyRequested, NoClaims, NotFound, PriorityClassIsRetired,
    RequestMismatch, SchedulingRequestData, SchedulingRequestSnapshot, SchedulingRequestState,
    SchedulingRequested, SubmitSchedulingRequest, SubmitSchedulingRequestOutcome, UnknownGang,
    UnknownPriorityClass,
};
use weft_types::shared::SchedulingRequestId;

use crate::context::mint;
use crate::Ports;

impl SubmitSchedulingRequestBehavior for Ports {
    fn submit_scheduling_request(
        &mut self,
        input: SubmitSchedulingRequest,
    ) -> Result<SubmitSchedulingRequestOutcome, UnmetObligation> {
        if input.claims.is_empty() {
            return Ok(SubmitSchedulingRequestOutcome::NoClaims { error: NoClaims });
        }

        if ScopeStorage::get(self, &input.scope_id).is_none() {
            return Ok(SubmitSchedulingRequestOutcome::UnknownScope { error: NotFound });
        }
        let Some(class) = PriorityClassStorage::get(self, &input.priority) else {
            return Ok(SubmitSchedulingRequestOutcome::UnknownPriority {
                error: UnknownPriorityClass,
            });
        };
        let gang = match &input.gang_id {
            Some(id) => match GangStorage::get(self, id) {
                Some(row) => Some(row),
                None => {
                    return Ok(SubmitSchedulingRequestOutcome::UnknownGang { error: UnknownGang })
                }
            },
            None => None,
        };

        if class.state == PriorityClassState::Retired {
            return Ok(SubmitSchedulingRequestOutcome::RetiredPriority {
                error: PriorityClassIsRetired,
            });
        }
        if SchedulingRequestStorage::list(self).iter().any(|request| {
            request.data.instance_id == input.instance_id
                && matches!(
                    request.state,
                    SchedulingRequestState::Pending | SchedulingRequestState::Unschedulable
                )
        }) {
            return Ok(SubmitSchedulingRequestOutcome::AlreadyRequested {
                error: InstanceAlreadyRequested,
            });
        }
        if gang
            .as_ref()
            .is_some_and(|gang| gang.state != GangState::Forming)
        {
            return Ok(SubmitSchedulingRequestOutcome::GangClosed { error: GangClosed });
        }
        if gang
            .as_ref()
            .is_some_and(|gang| gang.data.scope_id != input.scope_id)
        {
            return Ok(SubmitSchedulingRequestOutcome::GangOfOtherScope {
                error: RequestMismatch,
            });
        }

        let Some(instance) = InstanceStorage::get(self, &input.instance_id) else {
            return Ok(SubmitSchedulingRequestOutcome::NoSuchInstance { error: NotFound });
        };
        if instance.data.scope_id != input.scope_id {
            return Ok(SubmitSchedulingRequestOutcome::InstanceOfOtherScope { error: NotFound });
        }
        if let Some(identity) = &input.identity {
            let holds = WorkloadStorage::get(self, &instance.data.workload_id)
                .is_some_and(|workload| &workload.data.identity == identity);
            if !holds {
                return Ok(SubmitSchedulingRequestOutcome::IdentityNotInstances {
                    error: RequestMismatch,
                });
            }
        }

        let request_id = SchedulingRequestId(Uuid(mint()));
        let requested = SchedulingRequested {
            request_id: request_id.clone(),
            instance_id: input.instance_id.clone(),
            scope_id: input.scope_id.clone(),
            min_isolation: input.min_isolation,
            priority: input.priority.clone(),
        };
        let requested_at = Context::generate_timestamp(self);
        SchedulingRequestStorage::put(
            self,
            SchedulingRequestSnapshot {
                state: SchedulingRequestState::Pending,
                data: SchedulingRequestData {
                    request_id,
                    instance_id: input.instance_id,
                    scope_id: input.scope_id,
                    claims: input.claims,
                    min_isolation: input.min_isolation,
                    zones: input.zones,
                    priority: input.priority,
                    priority_value: class.data.value,
                    preempts: class.data.preempts,
                    preemptible: class.data.preemptible,
                    grace_seconds: class.data.grace_seconds,
                    gang_id: input.gang_id,
                    requested_at,
                    unschedulable_reason: None,
                    unschedulable_detail: None,
                },
            },
        );
        Ok(SubmitSchedulingRequestOutcome::Requested {
            scheduling_requested: requested,
        })
    }
}
