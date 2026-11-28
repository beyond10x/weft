//! `weft.edge.RecordBackendReady` (`ess/5-runtime/edge.yaml`), invoked by the binding
//! `edge-backend-ready` on `weft.workloads.InstanceReady`, in the selection precedence of its
//! contract (ess/37, the one rule):
//!
//! 1. the related rows, in declaration order: `no-such-instance` (no `weft.workloads.Instance`
//!    row of the input's instance), `no-such-workload` (no `weft.workloads.Workload` row of the
//!    input's workload), `other-workload` (the instance is another workload's);
//! 2. the row-set guard `known`: a `weft.edge.RouteBackend` of the instance exists;
//! 3. `added` otherwise: a new `Ready` backend of the instance, with the workload's formation,
//!    environment and component, and `BackendSetChanged` with `ready: true`.

use weft_types::behaviour::{InstanceStorage, RouteBackendStorage, WorkloadStorage};
use weft_types::edge::obligations::RecordBackendReadyBehavior;
use weft_types::edge::{
    BackendAlreadyKnown, BackendSetChanged, InstanceNotFound, RecordBackendReady,
    RecordBackendReadyOutcome, RouteBackendData, RouteBackendSnapshot, RouteBackendState,
};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::RouteBackendId;

use crate::context::mint;
use crate::Ports;

impl RecordBackendReadyBehavior for Ports {
    fn record_backend_ready(
        &mut self,
        input: RecordBackendReady,
    ) -> Result<RecordBackendReadyOutcome, UnmetObligation> {
        let Some(instance) = InstanceStorage::get(self, &input.instance_id) else {
            return Ok(RecordBackendReadyOutcome::NoSuchInstance {
                error: InstanceNotFound,
            });
        };
        let Some(workload) = WorkloadStorage::get(self, &input.workload_id) else {
            return Ok(RecordBackendReadyOutcome::NoSuchWorkload {
                error: InstanceNotFound,
            });
        };
        if instance.data.workload_id != input.workload_id {
            return Ok(RecordBackendReadyOutcome::OtherWorkload {
                error: InstanceNotFound,
            });
        }
        if RouteBackendStorage::list(self)
            .iter()
            .any(|backend| backend.data.instance_id == input.instance_id)
        {
            return Ok(RecordBackendReadyOutcome::Known {
                error: BackendAlreadyKnown,
            });
        }

        let backend_id = RouteBackendId(Uuid(mint()));
        RouteBackendStorage::put(
            self,
            RouteBackendSnapshot {
                state: RouteBackendState::Ready,
                data: RouteBackendData {
                    backend_id: backend_id.clone(),
                    instance_id: input.instance_id.clone(),
                    workload_id: input.workload_id.clone(),
                    formation_id: workload.data.formation_id,
                    environment_id: workload.data.environment_id,
                    component: workload.data.component,
                },
            },
        );
        Ok(RecordBackendReadyOutcome::Added {
            backend_set_changed: BackendSetChanged {
                backend_id,
                instance_id: input.instance_id,
                workload_id: input.workload_id,
                ready: true,
            },
        })
    }
}
