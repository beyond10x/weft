//! `weft.workloads.SubmitJob` (`ess/cloud/5-workloads/workloads.yaml`), in the selection
//! precedence of its contract:
//!
//! 1. the input guards in declaration order: `not-callers-scope` (a caller acting in no scope
//!    counts as another scope's), `bad-counts` (`parallelism` below 1 or `max_retries` below 0);
//! 2. the existence checks in declaration order: `no-workload`, `schedule-missing` (a
//!    `schedule_id` is named and no `weft.workloads.Schedule` carries it);
//! 3. the predicates in declaration order: `workload-of-other-scope`,
//!    `schedule-of-other-workload`, `workload-retired`, `not-a-job` (a workload of any shape but
//!    `Job` and `Schedule`);
//! 4. the external branch `over-quota` of the outcome group `quota`, decided from the rows of
//!    the same store ([`crate::quota::exceeded_alone`]): the claims of `parallelism` instances
//!    of the workload alone would take the scope's quota, or a quota above it, past its limit.
//!    What the scope's live instances hold does not count; `PlaceInstance` admits each task's
//!    instance. It is answered before `submitted`, the branch the contract takes "otherwise";
//! 5. `submitted`: a `Pending` Job of the workload, in the workload's scope, with the counts
//!    sent, no retry used, no task succeeded and no output.
//!
//! Invoked by the hand-run binding `r1-rollout-submit-job`, which acts in the scope of the
//! `ReleaseStepStarted` it delivers ([`crate::bindings`]).

use weft_types::behaviour::{Context, JobStorage, ScheduleStorage, WorkloadStorage};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::JobId;
use weft_types::workloads::obligations::SubmitJobBehavior;
use weft_types::workloads::{
    ForeignOwner, InvalidAmount, JobData, JobSnapshot, JobState, JobSubmitted, NotFound,
    QuotaExceeded, ShapeKind, ShapeMismatch, SubmitJob, SubmitJobOutcome, WorkloadIsRetired,
    WorkloadNotFound, WorkloadState,
};

use crate::context::mint;
use crate::quota;
use crate::Ports;

impl SubmitJobBehavior for Ports {
    fn submit_job(&mut self, input: SubmitJob) -> Result<SubmitJobOutcome, UnmetObligation> {
        use SubmitJobOutcome as Outcome;
        if Context::caller_scope_id(self).as_ref() != Some(&input.scope_id) {
            return Ok(Outcome::NotCallersScope {
                error: WorkloadNotFound,
            });
        }
        if input.parallelism < 1 || input.max_retries < 0 {
            return Ok(Outcome::BadCounts {
                error: InvalidAmount,
            });
        }
        let Some(workload) = WorkloadStorage::get(self, &input.workload_id) else {
            return Ok(Outcome::NoWorkload {
                error: WorkloadNotFound,
            });
        };
        let schedule = match &input.schedule_id {
            Some(id) => match ScheduleStorage::get(self, id) {
                Some(row) => Some(row),
                None => return Ok(Outcome::ScheduleMissing { error: NotFound }),
            },
            None => None,
        };
        if workload.data.scope_id != input.scope_id {
            return Ok(Outcome::WorkloadOfOtherScope {
                error: WorkloadNotFound,
            });
        }
        if schedule
            .as_ref()
            .is_some_and(|row| row.data.workload_id != input.workload_id)
        {
            return Ok(Outcome::ScheduleOfOtherWorkload {
                error: ForeignOwner,
            });
        }
        if workload.state == WorkloadState::Retired {
            return Ok(Outcome::WorkloadRetired {
                error: WorkloadIsRetired,
            });
        }
        if !matches!(workload.data.shape, ShapeKind::Job | ShapeKind::Schedule) {
            return Ok(Outcome::NotAJob {
                error: ShapeMismatch,
            });
        }
        let claims = quota::times(&workload.data.claims, input.parallelism);
        if quota::exceeded_alone(self, &workload.data.scope_id, &claims) {
            return Ok(Outcome::OverQuota {
                error: QuotaExceeded,
            });
        }
        let job_id = JobId(Uuid(mint()));
        let data = JobData {
            job_id: job_id.clone(),
            workload_id: input.workload_id.clone(),
            scope_id: workload.data.scope_id,
            parallelism: input.parallelism,
            gang: input.gang,
            max_retries: input.max_retries,
            retries_used: 0,
            succeeded_tasks: 0,
            outputs: Vec::new(),
            failure: None,
            schedule_id: input.schedule_id,
        };
        if let Some(broken) = data.broken_invariant() {
            return Err(UnmetObligation {
                capability: "entity invariant",
                source: broken,
            });
        }
        JobStorage::put(
            self,
            JobSnapshot {
                state: JobState::Pending,
                data,
            },
        );
        Ok(Outcome::Submitted {
            job_submitted: JobSubmitted {
                job_id,
                workload_id: input.workload_id,
                parallelism: input.parallelism,
                gang: input.gang,
            },
        })
    }
}
