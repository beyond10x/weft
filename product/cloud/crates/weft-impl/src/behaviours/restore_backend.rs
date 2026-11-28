//! `weft.edge.RestoreBackend` (`ess/5-runtime/edge.yaml`), invoked by the binding
//! `edge-backend-restore` on `weft.workloads.InstanceReady`, in the selection precedence of its
//! contract (ess/37, the one rule):
//!
//! 1. the row-set guard `not-out-of-traffic`: no `weft.edge.RouteBackend` of the instance reads
//!    `NotReady` (the instance's first readiness, which `edge-backend-ready` answers, or a
//!    repeated delivery);
//! 2. `restored` otherwise: each `NotReady` backend of the instance moves back to `Ready`
//!    (`ready`), and `BackendSetChanged` with `ready: true` names the first of them in the order
//!    the storage port lists them. The outcome carries one event (ESS 0.99.2 generates one
//!    member for `each: changed`); the edge keeps one backend per instance (`known` of
//!    `RecordBackendReady`), so the others it would name do not occur.

use weft_types::behaviour::RouteBackendStorage;
use weft_types::edge::obligations::RestoreBackendBehavior;
use weft_types::edge::{
    BackendNotFound, BackendSetChanged, RestoreBackend, RestoreBackendOutcome, RouteBackendState,
};
use weft_types::obligation::UnmetObligation;

use crate::Ports;

impl RestoreBackendBehavior for Ports {
    fn restore_backend(
        &mut self,
        input: RestoreBackend,
    ) -> Result<RestoreBackendOutcome, UnmetObligation> {
        let out_of_traffic: Vec<_> = RouteBackendStorage::list(self)
            .into_iter()
            .filter(|backend| {
                backend.data.instance_id == input.instance_id
                    && backend.state == RouteBackendState::NotReady
            })
            .collect();
        let Some(first) = out_of_traffic.first() else {
            return Ok(RestoreBackendOutcome::NotOutOfTraffic {
                error: BackendNotFound,
            });
        };
        let changed = BackendSetChanged {
            backend_id: first.data.backend_id.clone(),
            instance_id: input.instance_id,
            workload_id: first.data.workload_id.clone(),
            ready: true,
        };
        for mut backend in out_of_traffic {
            backend.state = RouteBackendState::Ready;
            RouteBackendStorage::put(self, backend);
        }
        Ok(RestoreBackendOutcome::Restored {
            backend_set_changed: changed,
        })
    }
}
