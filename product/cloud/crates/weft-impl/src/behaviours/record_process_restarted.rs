//! `weft.workloads.RecordProcessRestarted` (`ess/4-applications/workloads.yaml`), run by binding
//! r9-record-process-restarted when the node agent's `weft.node.RestartProcess` publishes
//! `ProcessRestarted` (`story:node-records-on-control-host`), in the selection precedence of its
//! contract (ess/37, the one rule):
//!
//! 1. the input refusal `invalid-count` (`restarts_in_window` or `restarts` below 1);
//! 2. the row the input names: `no-such-instance`;
//! 3. the refusals on the row, in declaration order: `wrong-node` (the instance is on no node,
//!    or on another), `no-such-process`, `restart-never` (its restart policy is `Never`),
//!    `count-above-total` (more restarts in the window than in all), `backoff-out-of-range`
//!    (the wait outside the policy's backoff bounds);
//! 4. for a `Starting` or `Running` instance, each recording the larger of its count and the
//!    input's (a repeated delivery changes nothing), the count in the window and the wait, and
//!    `ready` false:
//!    - `backing-off`, its health not `BackingOff` and the count in the window past the policy's
//!      `backoff_limit`: health `BackingOff`, `ProcessRestarted` and `InstanceBackingOff`;
//!    - `restarted-backing-off`, its health `BackingOff`: `ProcessRestarted`;
//!    - `restarted` otherwise: health `Starting`, `ProcessRestarted` and
//!      `InstanceHealthStarting`;
//! 5. `other-state` from any other state.

use weft_types::behaviour::InstanceStorage;
use weft_types::obligation::UnmetObligation;
use weft_types::workloads::obligations::RecordProcessRestartedBehavior;
use weft_types::workloads::{
    BackoffOutOfRange, InstanceBackingOff, InstanceHealth, InstanceHealthStarting, InstanceState,
    InvalidCount, NotFound, ProcessNotDeclared, ProcessRestarted, RecordProcessRestarted,
    RecordProcessRestartedOutcome, RestartMode, RestartNotAllowed, StateConflict, WrongNode,
};

use crate::Ports;

impl RecordProcessRestartedBehavior for Ports {
    fn record_process_restarted(
        &mut self,
        input: RecordProcessRestarted,
    ) -> Result<RecordProcessRestartedOutcome, UnmetObligation> {
        if input.restarts_in_window < 1 || input.restarts < 1 {
            return Ok(RecordProcessRestartedOutcome::InvalidCount {
                error: InvalidCount,
            });
        }
        let Some(mut instance) = InstanceStorage::get(self, &input.instance_id) else {
            return Ok(RecordProcessRestartedOutcome::NoSuchInstance { error: NotFound });
        };
        if instance.data.node_id.as_ref() != Some(&input.node_id) {
            return Ok(RecordProcessRestartedOutcome::WrongNode { error: WrongNode });
        }
        if !instance
            .data
            .processes
            .iter()
            .any(|process| process.name == input.process)
        {
            return Ok(RecordProcessRestartedOutcome::NoSuchProcess {
                error: ProcessNotDeclared,
            });
        }
        let policy = &instance.data.restart_policy;
        if policy.mode == RestartMode::Never {
            return Ok(RecordProcessRestartedOutcome::RestartNever {
                error: RestartNotAllowed,
            });
        }
        if input.restarts_in_window > input.restarts {
            return Ok(RecordProcessRestartedOutcome::CountAboveTotal {
                error: InvalidCount,
            });
        }
        if input.backoff_seconds < policy.backoff_initial_seconds
            || input.backoff_seconds > policy.backoff_max_seconds
        {
            return Ok(RecordProcessRestartedOutcome::BackoffOutOfRange {
                error: BackoffOutOfRange,
            });
        }
        if !matches!(
            instance.state,
            InstanceState::Starting | InstanceState::Running
        ) {
            return Ok(RecordProcessRestartedOutcome::OtherState {
                error: StateConflict,
            });
        }

        let backing_off = instance.data.health == InstanceHealth::BackingOff;
        let past_limit = input.restarts_in_window > policy.backoff_limit;
        let restarts = instance.data.restarts.max(input.restarts);
        instance.data.restarts = restarts;
        instance.data.restarts_in_window = input.restarts_in_window;
        instance.data.backoff_seconds = Some(input.backoff_seconds);
        instance.data.ready = false;
        if !backing_off {
            instance.data.health = if past_limit {
                InstanceHealth::BackingOff
            } else {
                InstanceHealth::Starting
            };
        }
        let workload_id = instance.data.workload_id.clone();
        InstanceStorage::put(self, instance);

        let process_restarted = ProcessRestarted {
            instance_id: input.instance_id.clone(),
            process: input.process.clone(),
            reason: input.reason,
            restarts,
            restarts_in_window: input.restarts_in_window,
            backoff_seconds: input.backoff_seconds,
        };
        Ok(if backing_off {
            RecordProcessRestartedOutcome::RestartedBackingOff { process_restarted }
        } else if past_limit {
            RecordProcessRestartedOutcome::BackingOff {
                process_restarted,
                instance_backing_off: InstanceBackingOff {
                    instance_id: input.instance_id,
                    workload_id,
                    process: input.process,
                    restarts_in_window: input.restarts_in_window,
                    backoff_seconds: input.backoff_seconds,
                },
            }
        } else {
            RecordProcessRestartedOutcome::Restarted {
                process_restarted,
                instance_health_starting: InstanceHealthStarting {
                    instance_id: input.instance_id,
                    workload_id,
                },
            }
        })
    }
}
