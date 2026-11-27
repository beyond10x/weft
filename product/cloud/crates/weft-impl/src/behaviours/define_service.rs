//! `weft.workloads.DefineService` (`PLAN.md` § Obligations, `ess/4-applications/workloads.yaml`), in
//! the selection precedence of its contract:
//!
//! 1. the input guards in declaration order: `not-callers-account` (a caller acting in no
//!    account counts as another account's), `bad-bounds`, `sleep-needs-policy`,
//!    `policy-needs-min-zero`;
//! 2. `no-workload`;
//! 3. the predicates in declaration order: `workload-of-other-account`, `workload-retired`,
//!    `not-a-service`, `exists` (a service of the workload, in any state);
//! 4. `defined`: an `Awake` service in the workload's account, none of its replicas running yet.
//!    `over-quota`, an external branch declared after `defined`, is never reached.

use weft_types::behaviour::{Context, ServiceStorage, WorkloadStorage};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::ServiceId;
use weft_types::workloads::obligations::DefineServiceBehavior;
use weft_types::workloads::{
    DefineService, DefineServiceOutcome, InvalidAmount, ServiceData, ServiceDefined, ServiceExists, ServiceSnapshot,
    ServiceState, ShapeKind, ShapeMismatch, SleepPolicyRequired, SleepPolicyRequiresMinZero, WorkloadIsRetired,
    WorkloadNotFound, WorkloadState,
};

use crate::context::mint;
use crate::Ports;

impl DefineServiceBehavior for Ports {
    fn define_service(&mut self, input: DefineService) -> Result<DefineServiceOutcome, UnmetObligation> {
        if Context::caller_account_id(self).as_ref() != Some(&input.account_id) {
            return Ok(DefineServiceOutcome::NotCallersAccount { error: WorkloadNotFound });
        }
        let rule = &input.scale_rule;
        if rule.min_replicas < 0
            || rule.max_replicas < rule.min_replicas
            || input.replicas < rule.min_replicas
            || input.replicas > rule.max_replicas
        {
            return Ok(DefineServiceOutcome::BadBounds { error: InvalidAmount });
        }
        if rule.min_replicas == 0 && input.sleep_policy.is_none() {
            return Ok(DefineServiceOutcome::SleepNeedsPolicy { error: SleepPolicyRequired });
        }
        if rule.min_replicas > 0 && input.sleep_policy.is_some() {
            return Ok(DefineServiceOutcome::PolicyNeedsMinZero { error: SleepPolicyRequiresMinZero });
        }
        let Some(workload) = WorkloadStorage::get(self, &input.workload_id) else {
            return Ok(DefineServiceOutcome::NoWorkload { error: WorkloadNotFound });
        };
        if workload.data.account_id != input.account_id {
            return Ok(DefineServiceOutcome::WorkloadOfOtherAccount { error: WorkloadNotFound });
        }
        if workload.state == WorkloadState::Retired {
            return Ok(DefineServiceOutcome::WorkloadRetired { error: WorkloadIsRetired });
        }
        if workload.data.shape != ShapeKind::Service {
            return Ok(DefineServiceOutcome::NotAService { error: ShapeMismatch });
        }
        if ServiceStorage::list(self)
            .iter()
            .any(|row| row.data.workload_id == input.workload_id)
        {
            return Ok(DefineServiceOutcome::Exists { error: ServiceExists });
        }
        let service_id = ServiceId(Uuid(mint()));
        let defined = ServiceDefined {
            service_id: service_id.clone(),
            workload_id: input.workload_id.clone(),
            replicas: input.replicas,
            scale_rule: input.scale_rule.clone(),
        };
        ServiceStorage::put(
            self,
            ServiceSnapshot {
                state: ServiceState::Awake,
                data: ServiceData {
                    service_id,
                    workload_id: input.workload_id,
                    account_id: workload.data.account_id,
                    replicas: input.replicas,
                    running_replicas: 0,
                    backoff_until: None,
                    scale_rule: input.scale_rule,
                    disruption_budget: input.disruption_budget,
                    sleep_policy: input.sleep_policy,
                    wake_deadline: input.wake_deadline,
                    sleep_snapshots: Vec::new(),
                },
            },
        );
        Ok(DefineServiceOutcome::Defined { service_defined: defined })
    }
}
