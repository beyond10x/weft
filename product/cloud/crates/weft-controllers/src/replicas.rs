//! R1 level (`ess/components.yaml`, "Level and time steps"):
//! `weft.workloads.ServicesBelowScale` -> `weft.workloads.RequestReplica` as Scheduler.
//!
//! For each row of the gap view, one `RequestReplica` of the service with the replica index
//! the step names: the lowest index below `replicas` that `weft.workloads.InstanceList` shows no
//! live instance of the service for (`Pending`, `Scheduled`, `Starting`, `Running`, `Paused`).
//! `RequestReplica` raises the service's count, so the next tick sees the service at scale or
//! requests its next replica; the instance itself is made by the binding `r1-create-replica`.

use serde_json::json;
use weft_impl::quota::live;
use weft_impl::System;
use weft_types::actor::Actor;

use crate::{send, Sent};

/// The command the step sends.
pub const REQUEST_REPLICA: &str = "weft.workloads.RequestReplica";

/// One `RequestReplica` per row of `ServicesBelowScale`: the commands sent. A view the system
/// cannot read sends nothing.
pub fn tick(system: &mut System) -> Vec<Sent> {
    let Ok(rows) = system.weft_control.services_below_scale() else {
        return Vec::new();
    };
    let mut sent = Vec::new();
    for row in rows {
        let Ok(instances) = system.weft_control.instance_list(row.workload_id.clone()) else {
            continue;
        };
        let taken = |index: i64| {
            instances.iter().any(|instance| {
                live(instance.state)
                    && instance.service_id.as_ref() == Some(&row.service_id)
                    && instance.replica_index == Some(index)
            })
        };
        let Some(index) = (0..row.replicas).find(|index| !taken(*index)) else {
            continue;
        };
        let input = json!({"service_id": row.service_id.0 .0, "replica_index": index});
        sent.push(send(system, REQUEST_REPLICA, Actor::Scheduler, &input));
    }
    sent
}
