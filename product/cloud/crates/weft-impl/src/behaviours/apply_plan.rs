//! `weft.deployments.ApplyPlan` (`PLAN.md` § Obligations, `ess/6-delivery/deployments.yaml`), in
//! declaration order:
//!
//! - `not-callers-account` (a caller acting in no scope counts as another scope's);
//! - `no-such-plan`, `plan-of-other-account` (the plan is another scope's);
//! - `simulation-violations` (the plan is `Blocked`), `plan-not-ready` (any other state but
//!   `Ready`);
//! - `created` otherwise: a `Release` in `Planned` with the plan's scope, formation, environment
//!   and revision, the strategy sent and the caller as its creator. The plan stays `Ready`: one
//!   release of a plan starts, which `weft.deployments.StartRelease` checks.
//!
//! The creator is the caller's principal; a caller without one is a request the model declares
//! no outcome for, refused naming the command before any row is written.

use weft_types::behaviour::{Context, PlanStorage, ReleaseStorage};
use weft_types::deployments::obligations::ApplyPlanBehavior;
use weft_types::deployments::{
    ApplyPlan, ApplyPlanOutcome, NotFound, PlanNotReady, PlanState, ReleaseCreated, ReleaseData,
    ReleaseSnapshot, ReleaseState, SimulationViolations,
};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::ReleaseId;

use crate::context::mint;
use crate::Ports;

impl ApplyPlanBehavior for Ports {
    fn apply_plan(&mut self, input: ApplyPlan) -> Result<ApplyPlanOutcome, UnmetObligation> {
        if Context::caller_scope_id(self).as_ref() != Some(&input.scope_id) {
            return Ok(ApplyPlanOutcome::NotCallersAccount { error: NotFound });
        }
        let Some(plan) = PlanStorage::get(self, &input.plan_id) else {
            return Ok(ApplyPlanOutcome::NoSuchPlan { error: NotFound });
        };
        if plan.data.scope_id != input.scope_id {
            return Ok(ApplyPlanOutcome::PlanOfOtherAccount { error: NotFound });
        }
        match plan.state {
            PlanState::Blocked => {
                return Ok(ApplyPlanOutcome::SimulationViolations {
                    error: SimulationViolations,
                })
            }
            PlanState::Ready => {}
            PlanState::Simulating | PlanState::Discarded => {
                return Ok(ApplyPlanOutcome::PlanNotReady {
                    error: PlanNotReady,
                })
            }
        }
        let created_by = Context::caller_principal_id(self).ok_or(UnmetObligation {
            capability: "command behaviour",
            source: "weft.deployments.ApplyPlan",
        })?;
        let release_id = ReleaseId(Uuid(mint()));
        let created_at = Context::generate_timestamp(self);
        let plan = plan.data;
        ReleaseStorage::put(
            self,
            ReleaseSnapshot {
                state: ReleaseState::Planned,
                data: ReleaseData {
                    release_id: release_id.clone(),
                    scope_id: plan.scope_id,
                    formation_id: plan.formation_id,
                    environment_id: plan.environment_id.clone(),
                    formation_revision_id: plan.formation_revision_id,
                    plan_id: input.plan_id.clone(),
                    strategy: input.strategy,
                    rollback_of: None,
                    created_by,
                    created_at,
                    live_at: None,
                    failure: None,
                },
            },
        );
        Ok(ApplyPlanOutcome::Created {
            release_created: ReleaseCreated {
                release_id,
                plan_id: input.plan_id,
                environment_id: plan.environment_id,
                strategy: input.strategy,
            },
        })
    }
}
