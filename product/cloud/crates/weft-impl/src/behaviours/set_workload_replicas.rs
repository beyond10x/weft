//! `weft.workloads.SetWorkloadReplicas` (`ess/cloud/5-workloads/workloads.yaml`), in declaration
//! order:
//!
//! - the input guards `not-callers-scope` (a caller acting in no scope counts as another
//!   scope's) and `negative-replicas`;
//! - `no-workload`, `workload-of-other-scope`, `workload-retired`, `not-a-service` (a workload of
//!   any shape but `Service`);
//! - over the workload's services: `no-service` (none that is not `Retired`), `below-bounds` and
//!   `above-bounds` (a service whose `ScaleRule` the count leaves), `not-awake` (a service in
//!   any state but `Awake`);
//! - `set` otherwise: every `Awake` service of the workload takes the count. The guards leave at
//!   least one, and `weft.workloads.DefineService` defines at most one per workload; the outcome
//!   carries the `ServiceScaled` of the first.
//!
//! Invoked by the hand-run binding `r1-set-workload-replicas`, which acts in the scope of the
//! `ReleaseStepStarted` it delivers ([`crate::bindings`]).

use weft_types::behaviour::{Context, ServiceStorage, WorkloadStorage};
use weft_types::obligation::UnmetObligation;
use weft_types::workloads::obligations::SetWorkloadReplicasBehavior;
use weft_types::workloads::{
    InvalidAmount, ReplicasOutOfBounds, ServiceNotFound, ServiceScaled, ServiceState,
    SetWorkloadReplicas, SetWorkloadReplicasOutcome, ShapeKind, ShapeMismatch, StateConflict,
    WorkloadIsRetired, WorkloadNotFound, WorkloadState,
};

use crate::Ports;

impl SetWorkloadReplicasBehavior for Ports {
    fn set_workload_replicas(
        &mut self,
        input: SetWorkloadReplicas,
    ) -> Result<SetWorkloadReplicasOutcome, UnmetObligation> {
        use SetWorkloadReplicasOutcome as Outcome;
        if Context::caller_scope_id(self).as_ref() != Some(&input.scope_id) {
            return Ok(Outcome::NotCallersScope {
                error: WorkloadNotFound,
            });
        }
        if input.replicas < 0 {
            return Ok(Outcome::NegativeReplicas {
                error: InvalidAmount,
            });
        }
        let Some(workload) = WorkloadStorage::get(self, &input.workload_id) else {
            return Ok(Outcome::NoWorkload {
                error: WorkloadNotFound,
            });
        };
        if workload.data.scope_id != input.scope_id {
            return Ok(Outcome::WorkloadOfOtherScope {
                error: WorkloadNotFound,
            });
        }
        if workload.state == WorkloadState::Retired {
            return Ok(Outcome::WorkloadRetired {
                error: WorkloadIsRetired,
            });
        }
        if workload.data.shape != ShapeKind::Service {
            return Ok(Outcome::NotAService {
                error: ShapeMismatch,
            });
        }
        let services: Vec<_> = ServiceStorage::list(self)
            .into_iter()
            .filter(|service| service.data.workload_id == input.workload_id)
            .collect();
        if !services
            .iter()
            .any(|service| service.state != ServiceState::Retired)
        {
            return Ok(Outcome::NoService {
                error: ServiceNotFound,
            });
        }
        if services
            .iter()
            .any(|service| service.data.scale_rule.min_replicas > input.replicas)
        {
            return Ok(Outcome::BelowBounds {
                error: ReplicasOutOfBounds,
            });
        }
        if services
            .iter()
            .any(|service| service.data.scale_rule.max_replicas < input.replicas)
        {
            return Ok(Outcome::AboveBounds {
                error: ReplicasOutOfBounds,
            });
        }
        if services
            .iter()
            .any(|service| service.state != ServiceState::Awake)
        {
            return Ok(Outcome::NotAwake {
                error: StateConflict,
            });
        }
        let mut scaled = None;
        for mut service in services {
            let service_id = service.data.service_id.clone();
            service.data.replicas = input.replicas;
            ServiceStorage::put(self, service);
            scaled.get_or_insert(service_id);
        }
        let service_id = scaled.ok_or(UnmetObligation {
            capability: "command behaviour",
            source: "weft.workloads.SetWorkloadReplicas",
        })?;
        Ok(Outcome::Set {
            service_scaled: ServiceScaled {
                service_id,
                replicas: input.replicas,
            },
        })
    }
}
