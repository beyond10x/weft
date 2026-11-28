//! `weft.deployments.AddReleaseStep` (`ess/6-delivery/deployments.yaml`), in declaration order:
//!
//! - the input guards `invalid-position` (below 1), `rollout-needs-replicas` (a `Rollout` step
//!   with no count) and `invalid-replicas` (a count below 0);
//! - `no-such-release`, `release-not-planned` (the release is in any state but `Planned`);
//! - `position-taken` (a step of the release already sits at the position);
//! - `added` otherwise: a `Pending` step of the release with the position, kind, component and
//!   replica count sent.

use weft_types::behaviour::{ReleaseStepStorage, ReleaseStorage};
use weft_types::deployments::obligations::AddReleaseStepBehavior;
use weft_types::deployments::{
    AddReleaseStep, AddReleaseStepOutcome, InvalidReplicas, InvalidStepPosition, NotFound,
    ReleaseNotPlanned, ReleaseState, ReleaseStepAdded, ReleaseStepData, ReleaseStepSnapshot,
    ReleaseStepState, RolloutNeedsReplicas, StepKind, StepPositionTaken,
};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::ReleaseStepId;

use crate::context::mint;
use crate::Ports;

impl AddReleaseStepBehavior for Ports {
    fn add_release_step(
        &mut self,
        input: AddReleaseStep,
    ) -> Result<AddReleaseStepOutcome, UnmetObligation> {
        use AddReleaseStepOutcome as Outcome;
        if input.position < 1 {
            return Ok(Outcome::InvalidPosition {
                error: InvalidStepPosition,
            });
        }
        if input.kind == StepKind::Rollout && input.replicas.is_none() {
            return Ok(Outcome::RolloutNeedsReplicas {
                error: RolloutNeedsReplicas,
            });
        }
        if input.replicas.is_some_and(|replicas| replicas < 0) {
            return Ok(Outcome::InvalidReplicas {
                error: InvalidReplicas,
            });
        }
        let Some(release) = ReleaseStorage::get(self, &input.release_id) else {
            return Ok(Outcome::NoSuchRelease { error: NotFound });
        };
        if release.state != ReleaseState::Planned {
            return Ok(Outcome::ReleaseNotPlanned {
                error: ReleaseNotPlanned,
            });
        }
        if ReleaseStepStorage::list(self).iter().any(|step| {
            step.data.release_id == input.release_id && step.data.position == input.position
        }) {
            return Ok(Outcome::PositionTaken {
                error: StepPositionTaken,
            });
        }
        let step_id = ReleaseStepId(Uuid(mint()));
        ReleaseStepStorage::put(
            self,
            ReleaseStepSnapshot {
                state: ReleaseStepState::Pending,
                data: ReleaseStepData {
                    step_id: step_id.clone(),
                    release_id: input.release_id.clone(),
                    position: input.position,
                    kind: input.kind,
                    component: input.component,
                    job_id: None,
                    workload_id: None,
                    replicas: input.replicas,
                    ready_replicas: None,
                    failure: None,
                    started_at: None,
                    ended_at: None,
                },
            },
        );
        Ok(Outcome::Added {
            release_step_added: ReleaseStepAdded {
                step_id,
                release_id: input.release_id,
                position: input.position,
                kind: input.kind,
            },
        })
    }
}
