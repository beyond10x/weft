//! `weft.edge.RemoveBackend` (`ess/5-runtime/edge.yaml`), invoked by the hand-run binding
//! `edge-backend-exited` on `weft.workloads.InstanceExited`
//! (`story:cloudspec-submission-job-exit`: the first exit on the path), in the selection
//! precedence of its contract (ess/27, the one rule):
//!
//! 1. the row-set guard `no-backend`: no `weft.edge.RouteBackend` of the instance reads other than
//!    `Gone` (the instance was never ready, so it never was a backend, or this is a repeated
//!    delivery);
//! 2. `removed` otherwise: each such backend moves to `Gone` (`remove`, from `Ready` or
//!    `NotReady`), and `BackendSetChanged` with `ready: false` names the first of them in the
//!    order the storage port lists them. The outcome carries one event (ESS 0.99.2 generates one
//!    member for `each: changed`); the edge keeps one backend per instance (`known` of
//!    `RecordBackendReady`), so the others it would name do not occur.

use weft_types::behaviour::RouteBackendStorage;
use weft_types::edge::obligations::RemoveBackendBehavior;
use weft_types::edge::{
    BackendNotFound, BackendSetChanged, RemoveBackend, RemoveBackendOutcome, RouteBackendState,
};
use weft_types::obligation::UnmetObligation;

use crate::Ports;

impl RemoveBackendBehavior for Ports {
    fn remove_backend(
        &mut self,
        input: RemoveBackend,
    ) -> Result<RemoveBackendOutcome, UnmetObligation> {
        let live: Vec<_> = RouteBackendStorage::list(self)
            .into_iter()
            .filter(|backend| {
                backend.data.instance_id == input.instance_id
                    && backend.state != RouteBackendState::Gone
            })
            .collect();
        let Some(first) = live.first() else {
            return Ok(RemoveBackendOutcome::NoBackend {
                error: BackendNotFound,
            });
        };
        let changed = BackendSetChanged {
            backend_id: first.data.backend_id.clone(),
            instance_id: input.instance_id,
            workload_id: first.data.workload_id.clone(),
            ready: false,
        };
        for mut backend in live {
            backend.state = RouteBackendState::Gone;
            RouteBackendStorage::put(self, backend);
        }
        Ok(RemoveBackendOutcome::Removed {
            backend_set_changed: changed,
        })
    }
}
