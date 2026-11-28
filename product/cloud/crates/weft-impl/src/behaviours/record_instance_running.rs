//! `weft.workloads.RecordInstanceRunning` (`ess/4-applications/workloads.yaml`), sent by the node
//! agent once the processes of an instance run, in the selection precedence of its contract
//! (ess/37, the one rule):
//!
//! 1. the row the input names: `no-such-instance`;
//! 2. the present-related refusal `wrong-node` (the instance is on no node, or on another);
//! 3. `running-ready` for a `Starting` instance with health `Starting` none of whose processes
//!    declares a `Ready` check: it reads `Running`, health `Ready` and `ready`, and
//!    `InstanceRunning` and `InstanceReady` are published (nothing else would make it ready);
//! 4. `running`, the move `run` from `Starting` otherwise: it reads `Running`, its health as it
//!    was, and `InstanceRunning` is published;
//! 5. `wrong-state` from any other state.

use weft_types::behaviour::InstanceStorage;
use weft_types::obligation::UnmetObligation;
use weft_types::workloads::obligations::RecordInstanceRunningBehavior;
use weft_types::workloads::{
    CheckRole, InstanceHealth, InstanceReady, InstanceRunning, InstanceState, NotFound,
    RecordInstanceRunning, RecordInstanceRunningOutcome, StateConflict, WrongNode,
};

use crate::Ports;

impl RecordInstanceRunningBehavior for Ports {
    fn record_instance_running(
        &mut self,
        input: RecordInstanceRunning,
    ) -> Result<RecordInstanceRunningOutcome, UnmetObligation> {
        let Some(mut instance) = InstanceStorage::get(self, &input.instance_id) else {
            return Ok(RecordInstanceRunningOutcome::NoSuchInstance { error: NotFound });
        };
        if instance.data.node_id.as_ref() != Some(&input.node_id) {
            return Ok(RecordInstanceRunningOutcome::WrongNode { error: WrongNode });
        }
        if instance.state != InstanceState::Starting {
            return Ok(RecordInstanceRunningOutcome::WrongState {
                error: StateConflict,
            });
        }

        let ready_check = instance.data.processes.iter().any(|process| {
            process
                .checks
                .iter()
                .any(|check| check.role == CheckRole::Ready)
        });
        let ready = instance.data.health == InstanceHealth::Starting && !ready_check;
        instance.state = InstanceState::Running;
        if ready {
            instance.data.health = InstanceHealth::Ready;
            instance.data.ready = true;
        }
        let workload_id = instance.data.workload_id.clone();
        InstanceStorage::put(self, instance);
        let instance_running = InstanceRunning {
            instance_id: input.instance_id.clone(),
        };
        Ok(if ready {
            RecordInstanceRunningOutcome::RunningReady {
                instance_running,
                instance_ready: InstanceReady {
                    instance_id: input.instance_id,
                    workload_id,
                },
            }
        } else {
            RecordInstanceRunningOutcome::Running { instance_running }
        })
    }
}
