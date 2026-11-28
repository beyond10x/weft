//! `weft.node.StartProcess` (`ess/5-runtime/node.yaml`), sent by the node agent once a process of
//! an instance it holds runs (`story:slot-node-process`), in the selection precedence of its
//! contract (ess/37, the one rule):
//!
//! 1. the rows the input names, in declaration order: `no-such-assignment`, `no-such-instance`;
//! 2. the present-related refusals and the row-set guard, in declaration order:
//!    `assignment-not-held` (the assignment is of another node or instance, or not `Active`),
//!    `process-not-declared` (the instance has no process of the input's name),
//!    `already-started` (a `LocalProcess` of the instance and process that is not `Stopped`);
//! 3. `started`: a `Running` `LocalProcess` with the instance's restart policy, no restart yet,
//!    started now, and `LocalProcessStarted`.

use weft_types::behaviour::{AssignmentStorage, InstanceStorage, LocalProcessStorage};
use weft_types::node::obligations::StartProcessBehavior;
use weft_types::node::{
    AssignmentNotHeld, InstanceNotFound, LocalProcessData, LocalProcessSnapshot,
    LocalProcessStarted, LocalProcessState, ProcessAlreadyStarted, ProcessNotDeclared,
    StartProcess, StartProcessOutcome,
};
use weft_types::nodes::AssignmentState;
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::LocalProcessId;

use crate::context::{mint, timestamp};
use crate::Ports;

impl StartProcessBehavior for Ports {
    fn start_process(
        &mut self,
        input: StartProcess,
    ) -> Result<StartProcessOutcome, UnmetObligation> {
        let Some(assignment) = AssignmentStorage::get(self, &input.assignment_id) else {
            return Ok(StartProcessOutcome::NoSuchAssignment {
                error: AssignmentNotHeld,
            });
        };
        let Some(instance) = InstanceStorage::get(self, &input.instance_id) else {
            return Ok(StartProcessOutcome::NoSuchInstance {
                error: InstanceNotFound,
            });
        };
        if assignment.data.node_id != input.node_id
            || assignment.data.instance_id != input.instance_id
            || assignment.state != AssignmentState::Active
        {
            return Ok(StartProcessOutcome::AssignmentNotHeld {
                error: AssignmentNotHeld,
            });
        }
        if !instance
            .data
            .processes
            .iter()
            .any(|process| process.name == input.process)
        {
            return Ok(StartProcessOutcome::ProcessNotDeclared {
                error: ProcessNotDeclared,
            });
        }
        if LocalProcessStorage::list(self).iter().any(|local| {
            local.data.instance_id == input.instance_id
                && local.data.process == input.process
                && local.state != LocalProcessState::Stopped
        }) {
            return Ok(StartProcessOutcome::AlreadyStarted {
                error: ProcessAlreadyStarted,
            });
        }

        let local_process_id = LocalProcessId(Uuid(mint()));
        let started_at = timestamp(self.now());
        LocalProcessStorage::put(
            self,
            LocalProcessSnapshot {
                state: LocalProcessState::Running,
                data: LocalProcessData {
                    local_process_id: local_process_id.clone(),
                    node_id: input.node_id.clone(),
                    assignment_id: input.assignment_id.clone(),
                    instance_id: input.instance_id.clone(),
                    process: input.process.clone(),
                    restart_policy: instance.data.restart_policy,
                    restarts: 0,
                    restarts_in_window: 0,
                    backoff_seconds: 0,
                    last_reason: None,
                    started_at,
                },
            },
        );
        Ok(StartProcessOutcome::Started {
            local_process_started: LocalProcessStarted {
                local_process_id,
                node_id: input.node_id,
                assignment_id: input.assignment_id,
                instance_id: input.instance_id,
                process: input.process,
            },
        })
    }
}
