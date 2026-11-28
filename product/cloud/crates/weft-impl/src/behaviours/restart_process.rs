//! `weft.node.RestartProcess` (`ess/5-runtime/node.yaml`), sent by the node agent after it started
//! a process again (`story:slot-node-process`: after an exit, under the restart policy and its
//! delay), in the selection precedence of its contract (ess/37, the one rule):
//!
//! 1. the input refusal `invalid-count` (`restarts_in_window` below 1);
//! 2. the row the input names: `no-such-process`;
//! 3. the refusals on that row, in declaration order: `not-own-node`, `stopped`,
//!    `restart-never` (its restart policy's mode is `Never`), `count-above-total`
//!    (`restarts_in_window` above its restarts plus this one), `backoff-out-of-range` (the wait
//!    below the policy's initial backoff or above its cap), `alive-not-due` (reason
//!    `AliveCheckFailed` from `Running`: weft-control has not recorded the check failed);
//! 4. `backing-off` when `restarts_in_window` is above the policy's `backoff_limit`, else
//!    `restarted`: the move `restart` to `Running`, one restart more, the input's count, wait and
//!    reason, and `ProcessRestarted` (with `ProcessBackingOff` in `backing-off`).

use weft_types::behaviour::LocalProcessStorage;
use weft_types::node::obligations::RestartProcessBehavior;
use weft_types::node::BackoffOutOfRange;
use weft_types::node::{
    InvalidCount, LocalProcessNotFound, LocalProcessState, NotOwnNode, ProcessBackingOff,
    ProcessRestarted, ProcessStopped, RestartNotAllowed, RestartNotDue, RestartProcess,
    RestartProcessOutcome,
};
use weft_types::obligation::UnmetObligation;
use weft_types::workloads::{RestartMode, RestartReason};

use crate::Ports;

impl RestartProcessBehavior for Ports {
    fn restart_process(
        &mut self,
        input: RestartProcess,
    ) -> Result<RestartProcessOutcome, UnmetObligation> {
        use RestartProcessOutcome as Outcome;
        if input.restarts_in_window < 1 {
            return Ok(Outcome::InvalidCount {
                error: InvalidCount,
            });
        }
        let Some(mut process) = LocalProcessStorage::get(self, &input.local_process_id) else {
            return Ok(Outcome::NoSuchProcess {
                error: LocalProcessNotFound,
            });
        };
        let policy = &process.data.restart_policy;
        if process.data.node_id != input.node_id {
            return Ok(Outcome::NotOwnNode { error: NotOwnNode });
        }
        if process.state == LocalProcessState::Stopped {
            return Ok(Outcome::Stopped {
                error: ProcessStopped,
            });
        }
        if policy.mode == RestartMode::Never {
            return Ok(Outcome::RestartNever {
                error: RestartNotAllowed,
            });
        }
        if input.restarts_in_window > process.data.restarts.saturating_add(1) {
            return Ok(Outcome::CountAboveTotal {
                error: InvalidCount,
            });
        }
        if input.backoff_seconds < policy.backoff_initial_seconds
            || input.backoff_seconds > policy.backoff_max_seconds
        {
            return Ok(Outcome::BackoffOutOfRange {
                error: BackoffOutOfRange,
            });
        }
        if input.reason == RestartReason::AliveCheckFailed
            && process.state == LocalProcessState::Running
        {
            return Ok(Outcome::AliveNotDue {
                error: RestartNotDue,
            });
        }
        let backing_off = input.restarts_in_window > policy.backoff_limit;

        process.state = LocalProcessState::Running;
        process.data.restarts = process.data.restarts.saturating_add(1);
        process.data.restarts_in_window = input.restarts_in_window;
        process.data.backoff_seconds = input.backoff_seconds;
        process.data.last_reason = Some(input.reason);
        if let Some(broken) = process.data.broken_invariant() {
            return Err(UnmetObligation {
                capability: "entity invariant",
                source: broken,
            });
        }
        let restarted = ProcessRestarted {
            local_process_id: input.local_process_id.clone(),
            node_id: input.node_id.clone(),
            instance_id: process.data.instance_id.clone(),
            process: process.data.process.clone(),
            reason: input.reason,
            restarts: process.data.restarts,
            restarts_in_window: input.restarts_in_window,
            backoff_seconds: input.backoff_seconds,
        };
        let outcome = if backing_off {
            Outcome::BackingOff {
                process_backing_off: ProcessBackingOff {
                    local_process_id: input.local_process_id,
                    node_id: input.node_id,
                    instance_id: process.data.instance_id.clone(),
                    process: process.data.process.clone(),
                    restarts_in_window: input.restarts_in_window,
                    backoff_seconds: input.backoff_seconds,
                },
                process_restarted: restarted,
            }
        } else {
            Outcome::Restarted {
                process_restarted: restarted,
            }
        };
        LocalProcessStorage::put(self, process);
        Ok(outcome)
    }
}
