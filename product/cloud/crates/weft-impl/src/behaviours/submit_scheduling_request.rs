//! `weft.scheduling.SubmitSchedulingRequest` (`PLAN.md` § Obligations,
//! `ess/5-runtime/scheduling.yaml`), in the selection precedence of its contract:
//!
//! 1. the input guard `no-claims`;
//! 2. every existence check before every predicate, in declaration order: `unknown-account`,
//!    `unknown-instance`, `unknown-formation`, `unknown-priority`, and `unknown-gang` where a
//!    gang is named;
//! 3. the predicates in declaration order: `instance-of-other-account`,
//!    `formation-of-other-account`, `formation-not-instances` (the instance's workload runs in
//!    another formation), `retired-priority`, `already-requested` (a `Pending` or
//!    `Unschedulable` request of the instance), `gang-closed` (not `Forming`),
//!    `gang-of-other-tenant` (the gang's tenant is not the account's);
//! 4. `requested`: a `Pending` request with the account's tenant, the formation's cluster, the
//!    priority class's value, preemption and grace, and `requested_at` from the clock.

use weft_types::behaviour::{
    AccountStorage, Context, FormationStorage, GangStorage, InstanceStorage, PriorityClassStorage,
    SchedulingRequestStorage, WorkloadStorage,
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

        let Some(account) = AccountStorage::get(self, &input.account_id) else {
            return Ok(SubmitSchedulingRequestOutcome::UnknownAccount { error: NotFound });
        };
        let Some(instance) = InstanceStorage::get(self, &input.instance_id) else {
            return Ok(SubmitSchedulingRequestOutcome::UnknownInstance { error: NotFound });
        };
        let Some(formation) = FormationStorage::get(self, &input.formation_id) else {
            return Ok(SubmitSchedulingRequestOutcome::UnknownFormation { error: NotFound });
        };
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

        if instance.data.account_id != input.account_id {
            return Ok(SubmitSchedulingRequestOutcome::InstanceOfOtherAccount { error: NotFound });
        }
        if formation.data.account_id != input.account_id {
            return Ok(SubmitSchedulingRequestOutcome::FormationOfOtherAccount { error: NotFound });
        }
        if WorkloadStorage::get(self, &instance.data.workload_id)
            .is_none_or(|workload| workload.data.formation_id != input.formation_id)
        {
            return Ok(SubmitSchedulingRequestOutcome::FormationNotInstances {
                error: RequestMismatch,
            });
        }
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
            .is_some_and(|gang| gang.data.tenant_id != account.data.tenant_id)
        {
            return Ok(SubmitSchedulingRequestOutcome::GangOfOtherTenant {
                error: RequestMismatch,
            });
        }

        let request_id = SchedulingRequestId(Uuid(mint()));
        let requested = SchedulingRequested {
            request_id: request_id.clone(),
            instance_id: input.instance_id.clone(),
            tenant_id: account.data.tenant_id.clone(),
            cluster_id: formation.data.cluster_id.clone(),
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
                    tenant_id: account.data.tenant_id,
                    account_id: input.account_id,
                    formation_id: input.formation_id,
                    cluster_id: formation.data.cluster_id,
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
