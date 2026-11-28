//! `weft.workloads.CompleteJob` (`ess/cloud/5-workloads/workloads.yaml`), in the selection
//! precedence of its contract (ess/27, the one rule):
//!
//! 1. the addressed row: `no-such-job`;
//! 2. what it holds: `outstanding` (`succeeded_tasks` below `parallelism`), then `wrong-state`
//!    (any state but `Running`, the one `succeed` starts from);
//! 3. the rows around it: `unknown-output` (an output no `weft.artifacts.Artifact` row carries);
//! 4. `succeeded`, the move `succeed`: the Job reads `Succeeded` with the outputs sent, and
//!    `JobSucceeded` names them.
//!
//! The contract's OPEN (an output should be an artifact of the Job's workload's account) is not
//! checked: it names no outcome. Invoked by the hand-run binding `job-complete-on-task` with no
//! output (a snippet writes none, `story:cloudspec-submission-job-exit`).

use weft_types::behaviour::{ArtifactStorage, JobStorage};
use weft_types::obligation::UnmetObligation;
use weft_types::workloads::obligations::CompleteJobBehavior;
use weft_types::workloads::{
    ArtifactNotFound, CompleteJob, CompleteJobOutcome, JobState, JobSucceeded, NotFound,
    StateConflict, TasksOutstanding,
};

use crate::Ports;

impl CompleteJobBehavior for Ports {
    fn complete_job(&mut self, input: CompleteJob) -> Result<CompleteJobOutcome, UnmetObligation> {
        use CompleteJobOutcome as Outcome;
        let Some(mut job) = JobStorage::get(self, &input.job_id) else {
            return Ok(Outcome::NoSuchJob { error: NotFound });
        };
        if job.data.succeeded_tasks < job.data.parallelism {
            return Ok(Outcome::Outstanding {
                error: TasksOutstanding,
            });
        }
        if job.state != JobState::Running {
            return Ok(Outcome::WrongState {
                error: StateConflict,
            });
        }
        if input
            .outputs
            .iter()
            .any(|digest| ArtifactStorage::get(self, digest).is_none())
        {
            return Ok(Outcome::UnknownOutput {
                error: ArtifactNotFound,
            });
        }
        job.state = JobState::Succeeded;
        job.data.outputs = input.outputs.clone();
        if let Some(broken) = job.data.broken_invariant() {
            return Err(UnmetObligation {
                capability: "entity invariant",
                source: broken,
            });
        }
        JobStorage::put(self, job);
        Ok(Outcome::Succeeded {
            job_succeeded: JobSucceeded {
                job_id: input.job_id,
                outputs: input.outputs,
            },
        })
    }
}
