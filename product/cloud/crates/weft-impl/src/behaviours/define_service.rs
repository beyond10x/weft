//! `weft.workloads.DefineService` (`PLAN.md` § Obligations, `ess/cloud/5-workloads/workloads.yaml`), in
//! the selection precedence of its contract:
//!
//! 1. the input guards in declaration order: `not-callers-scope` (a caller acting in no
//!    scope counts as another scope's), `bad-bounds`, `sleep-needs-policy`,
//!    `policy-needs-min-zero`;
//! 2. `no-workload`;
//! 3. the predicates in declaration order: `workload-of-other-scope`, `workload-retired`,
//!    `not-a-service`, `exists` (a service of the workload, in any state);
//! 4. the external branch `over-quota` of the outcome group `quota`, decided from the rows of
//!    the same store ([`crate::quota::exceeded_alone`]): the claims of `replicas` instances of
//!    the workload alone would take the scope's quota, or a quota above it, past its limit. What the
//!    scope's live instances hold does not count; `CreateInstance` asks for each replica. It
//!    is answered before `defined`, the branch the contract takes "otherwise" (its default);
//! 5. `defined`: an `Awake` service in the workload's scope, none of its replicas running yet.

use weft_types::behaviour::{Context, ServiceStorage, WorkloadStorage};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::ServiceId;
use weft_types::workloads::obligations::DefineServiceBehavior;
use weft_types::workloads::{
    DefineService, DefineServiceOutcome, InvalidAmount, QuotaExceeded, ServiceData, ServiceDefined,
    ServiceExists, ServiceSnapshot, ServiceState, ShapeKind, ShapeMismatch, SleepPolicyRequired,
    SleepPolicyRequiresMinZero, WorkloadIsRetired, WorkloadNotFound, WorkloadState,
};

use crate::context::mint;
use crate::quota;
use crate::Ports;

impl DefineServiceBehavior for Ports {
    fn define_service(
        &mut self,
        input: DefineService,
    ) -> Result<DefineServiceOutcome, UnmetObligation> {
        if Context::caller_scope_id(self).as_ref() != Some(&input.scope_id) {
            return Ok(DefineServiceOutcome::NotCallersScope {
                error: WorkloadNotFound,
            });
        }
        let rule = &input.scale_rule;
        if rule.min_replicas < 0
            || rule.max_replicas < rule.min_replicas
            || input.replicas < rule.min_replicas
            || input.replicas > rule.max_replicas
        {
            return Ok(DefineServiceOutcome::BadBounds {
                error: InvalidAmount,
            });
        }
        if rule.min_replicas == 0 && input.sleep_policy.is_none() {
            return Ok(DefineServiceOutcome::SleepNeedsPolicy {
                error: SleepPolicyRequired,
            });
        }
        if rule.min_replicas > 0 && input.sleep_policy.is_some() {
            return Ok(DefineServiceOutcome::PolicyNeedsMinZero {
                error: SleepPolicyRequiresMinZero,
            });
        }
        let Some(workload) = WorkloadStorage::get(self, &input.workload_id) else {
            return Ok(DefineServiceOutcome::NoWorkload {
                error: WorkloadNotFound,
            });
        };
        if workload.data.scope_id != input.scope_id {
            return Ok(DefineServiceOutcome::WorkloadOfOtherScope {
                error: WorkloadNotFound,
            });
        }
        if workload.state == WorkloadState::Retired {
            return Ok(DefineServiceOutcome::WorkloadRetired {
                error: WorkloadIsRetired,
            });
        }
        if workload.data.shape != ShapeKind::Service {
            return Ok(DefineServiceOutcome::NotAService {
                error: ShapeMismatch,
            });
        }
        if ServiceStorage::list(self)
            .iter()
            .any(|row| row.data.workload_id == input.workload_id)
        {
            return Ok(DefineServiceOutcome::Exists {
                error: ServiceExists,
            });
        }
        let claims = quota::times(&workload.data.claims, input.replicas);
        if quota::exceeded_alone(self, &workload.data.scope_id, &claims) {
            return Ok(DefineServiceOutcome::OverQuota {
                error: QuotaExceeded,
            });
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
                    scope_id: workload.data.scope_id,
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
        Ok(DefineServiceOutcome::Defined {
            service_defined: defined,
        })
    }
}
