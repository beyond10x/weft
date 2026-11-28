//! `weft.workloads.CreateInstance` (`PLAN.md` § Obligations, `ess/cloud/5-workloads/workloads.yaml`),
//! in the selection precedence of its contract:
//!
//! 1. every existence check before every predicate, in declaration order: `no-workload`, then,
//!    for each owner the input names, `no-job`, `no-invocation`, `no-box`, `no-session`,
//!    `no-warm-pool`, `no-service`;
//! 2. the predicates in declaration order: `workload-retired`; each named owner of another
//!    workload (`foreign-job`, `foreign-invocation`, `foreign-box`, `foreign-session`,
//!    `foreign-warm-pool`, `foreign-service`); `replica-live`, only for a replica's request
//!    (service and index given): a live instance (`Pending`, `Scheduled`, `Starting`,
//!    `Running`, `Paused`) of the same service and index;
//! 3. `created` otherwise: a `Pending` instance with the workload's scope, artifact, isolation
//!    as its minimum, processes and restart policy, and `InstanceCreated` with the workload's
//!    formation, claims and priority and the formation's zones.
//!
//! The blocks take `CreateInstance` out of the outcome group `quota`: an instance is created
//! without a quota check, and `weft.scheduling.PlaceInstance` (`quota-exceeded`, weft's run-time
//! admission) refuses to place one past its scope's quota.

use weft_types::behaviour::{
    BoxStorage, FormationStorage, InstanceStorage, InvocationStorage, JobStorage, ServiceStorage,
    SessionStorage, WarmPoolStorage, WorkloadStorage,
};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::{InstanceId, WorkloadId};
use weft_types::workloads::obligations::CreateInstanceBehavior;
use weft_types::workloads::{
    CreateInstance, CreateInstanceOutcome, DesiredRun, ForeignOwner, InstanceCreated, InstanceData,
    InstanceHealth, InstanceSnapshot, InstanceState, NotFound, ReplicaLive, ServiceNotFound,
    WorkloadIsRetired, WorkloadNotFound, WorkloadState,
};

use crate::context::mint;
use crate::quota;
use crate::Ports;

impl CreateInstanceBehavior for Ports {
    fn create_instance(
        &mut self,
        input: CreateInstance,
    ) -> Result<CreateInstanceOutcome, UnmetObligation> {
        let Some(workload) = WorkloadStorage::get(self, &input.workload_id) else {
            return Ok(CreateInstanceOutcome::NoWorkload {
                error: WorkloadNotFound,
            });
        };
        let job = match &input.job_id {
            Some(id) => match JobStorage::get(self, id) {
                Some(row) => Some(row.data.workload_id),
                None => return Ok(CreateInstanceOutcome::NoJob { error: NotFound }),
            },
            None => None,
        };
        let invocation = match &input.invocation_id {
            Some(id) => match InvocationStorage::get(self, id) {
                Some(row) => Some(row.data.workload_id),
                None => return Ok(CreateInstanceOutcome::NoInvocation { error: NotFound }),
            },
            None => None,
        };
        let boxed = match &input.box_id {
            Some(id) => match BoxStorage::get(self, id) {
                Some(row) => Some(row.data.workload_id),
                None => return Ok(CreateInstanceOutcome::NoBox { error: NotFound }),
            },
            None => None,
        };
        let session = match &input.session_id {
            Some(id) => match SessionStorage::get(self, id) {
                Some(row) => Some(row.data.workload_id),
                None => return Ok(CreateInstanceOutcome::NoSession { error: NotFound }),
            },
            None => None,
        };
        let warm_pool = match &input.warm_pool_id {
            Some(id) => match WarmPoolStorage::get(self, id) {
                Some(row) => Some(row.data.workload_id),
                None => return Ok(CreateInstanceOutcome::NoWarmPool { error: NotFound }),
            },
            None => None,
        };
        let service = match &input.service_id {
            Some(id) => match ServiceStorage::get(self, id) {
                Some(row) => Some(row.data.workload_id),
                None => {
                    return Ok(CreateInstanceOutcome::NoService {
                        error: ServiceNotFound,
                    })
                }
            },
            None => None,
        };

        if workload.state == WorkloadState::Retired {
            return Ok(CreateInstanceOutcome::WorkloadRetired {
                error: WorkloadIsRetired,
            });
        }
        let foreign =
            |owner: &Option<WorkloadId>| owner.as_ref().is_some_and(|of| of != &input.workload_id);
        if foreign(&job) {
            return Ok(CreateInstanceOutcome::ForeignJob {
                error: ForeignOwner,
            });
        }
        if foreign(&invocation) {
            return Ok(CreateInstanceOutcome::ForeignInvocation {
                error: ForeignOwner,
            });
        }
        if foreign(&boxed) {
            return Ok(CreateInstanceOutcome::ForeignBox {
                error: ForeignOwner,
            });
        }
        if foreign(&session) {
            return Ok(CreateInstanceOutcome::ForeignSession {
                error: ForeignOwner,
            });
        }
        if foreign(&warm_pool) {
            return Ok(CreateInstanceOutcome::ForeignWarmPool {
                error: ForeignOwner,
            });
        }
        if foreign(&service) {
            return Ok(CreateInstanceOutcome::ForeignService {
                error: ForeignOwner,
            });
        }
        if let (Some(service_id), Some(index)) = (&input.service_id, input.replica_index) {
            if InstanceStorage::list(self).iter().any(|row| {
                quota::live(row.state)
                    && row.data.service_id.as_ref() == Some(service_id)
                    && row.data.replica_index == Some(index)
            }) {
                return Ok(CreateInstanceOutcome::ReplicaLive { error: ReplicaLive });
            }
        }

        let workload = workload.data;

        let zones = FormationStorage::get(self, &workload.formation_id)
            .map(|formation| formation.data.zones)
            .unwrap_or_default();
        let instance_id = InstanceId(Uuid(mint()));
        let created = InstanceCreated {
            instance_id: instance_id.clone(),
            workload_id: input.workload_id.clone(),
            minimum_isolation: workload.isolation,
            scope_id: workload.scope_id.clone(),
            formation_id: workload.formation_id.clone(),
            claims: workload.claims.clone(),
            service_id: input.service_id.clone(),
            replica_index: input.replica_index,
            job_id: input.job_id.clone(),
            zones,
            priority: workload.priority.clone(),
        };
        InstanceStorage::put(
            self,
            InstanceSnapshot {
                state: InstanceState::Pending,
                data: InstanceData {
                    instance_id,
                    workload_id: input.workload_id,
                    scope_id: workload.scope_id,
                    artifact_digest: workload.artifact_digest,
                    minimum_isolation: workload.isolation,
                    isolation: None,
                    node_id: None,
                    job_id: input.job_id,
                    task_index: input.task_index,
                    invocation_id: input.invocation_id,
                    box_id: input.box_id,
                    session_id: input.session_id,
                    replica_index: input.replica_index,
                    service_id: input.service_id,
                    replica_counted_out: false,
                    warm_pool_id: input.warm_pool_id,
                    restored_from: None,
                    desired: DesiredRun::Run,
                    ready: false,
                    health: InstanceHealth::Unknown,
                    processes: workload.processes,
                    restart_policy: workload.restart_policy,
                    restarts: 0,
                    restarts_in_window: 0,
                    backoff_seconds: None,
                    exit_code: None,
                    failure_reason: None,
                    eviction_reason: None,
                },
            },
        );
        Ok(CreateInstanceOutcome::Created {
            instance_created: created,
        })
    }
}
