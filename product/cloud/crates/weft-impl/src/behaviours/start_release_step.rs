//! `weft.deployments.StartReleaseStep` (`ess/6-delivery/deployments.yaml`), in declaration order:
//!
//! - `no-such-step`; `no-such-release` (the step's release is missing);
//! - `release-not-applying` (the release is in any state but `Applying`);
//! - `rollout-needs-workload` (a `Rollout` step started with no workload named);
//! - `previous-step-incomplete` (a step of the release at a lower position has not succeeded);
//! - `job-outside-release` (a job is named, and no `weft.architecture.ComponentWorkload` of the
//!   release's environment assigns the job's workload: an unknown job is refused too);
//! - `workload-outside-release` (a workload is named, and no assignment of the release's
//!   environment names it);
//! - `started` (`Pending` to `Running`, with the job, the workload and the start time), else
//!   `wrong-state`. `ReleaseStepStarted` carries the release's scope, which
//!   `r1-set-workload-replicas` sends to `weft.workloads.SetWorkloadReplicas`.

use weft_types::behaviour::{
    ComponentWorkloadStorage, Context, JobStorage, ReleaseStepStorage, ReleaseStorage,
};
use weft_types::deployments::obligations::StartReleaseStepBehavior;
use weft_types::deployments::{
    NotFound, OutsideRelease, PreviousStepIncomplete, ReleaseNotApplying, ReleaseState,
    ReleaseStepStarted, ReleaseStepState, RolloutNeedsWorkload, StartReleaseStep,
    StartReleaseStepOutcome, StateConflict, StepKind,
};
use weft_types::obligation::UnmetObligation;

use crate::Ports;

impl StartReleaseStepBehavior for Ports {
    fn start_release_step(
        &mut self,
        input: StartReleaseStep,
    ) -> Result<StartReleaseStepOutcome, UnmetObligation> {
        use StartReleaseStepOutcome as Outcome;
        let Some(mut step) = ReleaseStepStorage::get(self, &input.step_id) else {
            return Ok(Outcome::NoSuchStep { error: NotFound });
        };
        let Some(release) = ReleaseStorage::get(self, &step.data.release_id) else {
            return Ok(Outcome::NoSuchRelease { error: NotFound });
        };
        if release.state != ReleaseState::Applying {
            return Ok(Outcome::ReleaseNotApplying {
                error: ReleaseNotApplying,
            });
        }
        if step.data.kind == StepKind::Rollout && input.workload_id.is_none() {
            return Ok(Outcome::RolloutNeedsWorkload {
                error: RolloutNeedsWorkload,
            });
        }
        if ReleaseStepStorage::list(self).iter().any(|other| {
            other.data.release_id == step.data.release_id
                && other.data.position < step.data.position
                && other.state != ReleaseStepState::Succeeded
        }) {
            return Ok(Outcome::PreviousStepIncomplete {
                error: PreviousStepIncomplete,
            });
        }
        let environment_id = &release.data.environment_id;
        let assignments: Vec<_> = ComponentWorkloadStorage::list(self)
            .into_iter()
            .filter(|assignment| &assignment.data.environment_id == environment_id)
            .collect();
        if let Some(job_id) = &input.job_id {
            let workload = JobStorage::get(self, job_id).map(|job| job.data.workload_id);
            if !assignments
                .iter()
                .any(|assignment| Some(&assignment.data.workload_id) == workload.as_ref())
            {
                return Ok(Outcome::JobOutsideRelease {
                    error: OutsideRelease,
                });
            }
        }
        if let Some(workload_id) = &input.workload_id {
            if !assignments
                .iter()
                .any(|assignment| &assignment.data.workload_id == workload_id)
            {
                return Ok(Outcome::WorkloadOutsideRelease {
                    error: OutsideRelease,
                });
            }
        }
        if step.state != ReleaseStepState::Pending {
            return Ok(Outcome::WrongState {
                error: StateConflict,
            });
        }
        step.state = ReleaseStepState::Running;
        step.data.job_id.clone_from(&input.job_id);
        step.data.workload_id.clone_from(&input.workload_id);
        step.data.started_at = Some(Context::generate_timestamp(self));
        let started = ReleaseStepStarted {
            step_id: input.step_id,
            release_id: step.data.release_id.clone(),
            scope_id: release.data.scope_id,
            position: step.data.position,
            kind: step.data.kind,
            component: step.data.component.clone(),
            job_id: input.job_id,
            workload_id: input.workload_id,
            replicas: step.data.replicas,
        };
        ReleaseStepStorage::put(self, step);
        Ok(Outcome::Started {
            release_step_started: started,
        })
    }
}
