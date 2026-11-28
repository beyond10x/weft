//! One instance per node (`story:slot-node-pool-placement`, outcome 2): for each `Defined`
//! workload whose `DaemonSettings` record a placement ([`weft_impl::placement::Placed`]), each
//! `Ready` scope node with no live instance of the workload -> `weft.workloads.CreateInstance`
//! with that `scope_node_id`, as Scheduler.
//!
//! The scope nodes are the nodes of the workload's `per` scope, or of its own scope without
//! `per`: a pool scope's nodes are the `Ready` nodes of the pool, a host scope's node is that
//! node. `CreateInstance` writes each `${instance}` of the instance's processes as the scope
//! node's name, and the binding `r1-submit-scheduling-request` asks for its placement; the
//! Scheduler's step ([`crate::placement`]) offers it only the nodes it may run on.
//!
//! - Never a second: a scope node that holds a live instance of the workload (`Pending`,
//!   `Scheduled`, `Starting`, `Running`, `Paused`) is skipped, and `CreateInstance` refuses a
//!   second one (`scope-node-live`).
//! - A node that is not `Ready` gets no instance. An instance of a node that left the pool is
//!   never made again elsewhere (decision D-S2 A); stopping it belongs to the node-leave
//!   lifecycle, which this step does not run.
//! - A workload whose selector records no placement (the empty selector of a process part in a
//!   scope without one) gets nothing from this step.

use serde_json::json;
use weft_impl::placement::Placed;
use weft_impl::quota::live;
use weft_impl::System;
use weft_types::actor::Actor;
use weft_types::behaviour::{InstanceStorage, NodeStorage, WorkloadStorage};
use weft_types::infrastructure::NodeState;
use weft_types::workloads::WorkloadState;

use crate::{send, Context, Sent};

/// The command the step sends.
pub const CREATE_INSTANCE: &str = "weft.workloads.CreateInstance";

/// One `CreateInstance` per placed workload and `Ready` scope node that holds no live instance of
/// it: the commands sent, in the order of the workloads' and nodes' identities.
pub fn tick(system: &mut System, context: &Context<'_>) -> Vec<Sent> {
    let mut workloads = WorkloadStorage::list(context.ports);
    workloads.sort_by(|a, b| a.data.workload_id.0 .0.cmp(&b.data.workload_id.0 .0));
    let mut nodes = NodeStorage::list(context.ports);
    nodes.retain(|node| node.state == NodeState::Ready);
    nodes.sort_by(|a, b| a.data.node_id.0 .0.cmp(&b.data.node_id.0 .0));
    let instances = InstanceStorage::list(context.ports);
    let mut sent = Vec::new();
    for workload in workloads
        .iter()
        .filter(|row| row.state == WorkloadState::Defined)
    {
        let Some(placed) = Placed::of(&workload.data.settings) else {
            continue;
        };
        let Some(scope) = placed.scope() else {
            continue;
        };
        for node in nodes
            .iter()
            .filter(|node| scope.holds(&node.data.node_id, node.data.node_pool_id.as_ref()))
        {
            let held = instances.iter().any(|instance| {
                live(instance.state)
                    && instance.data.workload_id == workload.data.workload_id
                    && instance.data.scope_node_id.as_ref() == Some(&node.data.node_id)
            });
            if held {
                continue;
            }
            let input = json!({
                "workload_id": workload.data.workload_id.0 .0,
                "job_id": null,
                "task_index": null,
                "invocation_id": null,
                "box_id": null,
                "session_id": null,
                "replica_index": null,
                "warm_pool_id": null,
                "service_id": null,
                "scope_node_id": node.data.node_id.0 .0,
            });
            sent.push(send(system, CREATE_INSTANCE, Actor::Scheduler, &input));
        }
    }
    sent
}
