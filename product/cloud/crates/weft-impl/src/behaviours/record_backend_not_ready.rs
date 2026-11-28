//! `weft.edge.RecordBackendNotReady` (`ess/5-runtime/edge.yaml`), invoked by the binding
//! `edge-backend-restarting` on `weft.workloads.InstanceHealthStarting` (and, once they are on
//! the path, `edge-backend-not-ready` and `edge-backend-backing-off`), in the selection
//! precedence of its contract (ess/37, the one rule):
//!
//! 1. the row-set guard `not-ready`: no `weft.edge.RouteBackend` of the instance reads `Ready`
//!    (the instance was never ready, so it has no backend yet, or it is out of traffic already,
//!    or this is a repeated delivery);
//! 2. `taken-out` otherwise: each `Ready` backend of the instance moves to `NotReady`
//!    (`unready`), and `BackendSetChanged` with `ready: false` names the first of them in the
//!    order the storage port lists them. The outcome carries one event (ESS 0.99.2 generates one
//!    member for `each: changed`); the edge keeps one backend per instance (`known` of
//!    `RecordBackendReady`), so the others it would name do not occur.

use weft_types::behaviour::RouteBackendStorage;
use weft_types::edge::obligations::RecordBackendNotReadyBehavior;
use weft_types::edge::{
    BackendNotReady, BackendSetChanged, RecordBackendNotReady, RecordBackendNotReadyOutcome,
    RouteBackendState,
};
use weft_types::obligation::UnmetObligation;

use crate::Ports;

impl RecordBackendNotReadyBehavior for Ports {
    fn record_backend_not_ready(
        &mut self,
        input: RecordBackendNotReady,
    ) -> Result<RecordBackendNotReadyOutcome, UnmetObligation> {
        let in_traffic: Vec<_> = RouteBackendStorage::list(self)
            .into_iter()
            .filter(|backend| {
                backend.data.instance_id == input.instance_id
                    && backend.state == RouteBackendState::Ready
            })
            .collect();
        let Some(first) = in_traffic.first() else {
            return Ok(RecordBackendNotReadyOutcome::NotReady {
                error: BackendNotReady,
            });
        };
        let changed = BackendSetChanged {
            backend_id: first.data.backend_id.clone(),
            instance_id: input.instance_id,
            workload_id: first.data.workload_id.clone(),
            ready: false,
        };
        for mut backend in in_traffic {
            backend.state = RouteBackendState::NotReady;
            RouteBackendStorage::put(self, backend);
        }
        Ok(RecordBackendNotReadyOutcome::TakenOut {
            backend_set_changed: changed,
        })
    }
}
