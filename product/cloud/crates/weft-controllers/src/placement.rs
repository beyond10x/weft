//! The Scheduler's decision step (`story:skeleton-controllers`): `weft.scheduling.PendingRequests`,
//! with candidates from `weft.infrastructure.Nodes` -> `weft.scheduling.PlaceInstance` as
//! Scheduler.
//!
//! For each pending request, the candidates in order: each isolation level the request admits,
//! from its minimum up to `MicroVm` (the weakest first, so a stronger level is not taken where a
//! weaker one serves), and for each level every `Ready` node, in the view's order, whose
//! isolation facts offer it. The input carries the request's scope and formation, the cluster
//! the formation is pinned to and the tenant of the account that holds the scope ([`ties`]),
//! the node's zone and pool, no bindings, no topology (the default cluster has none) and
//! `decided_at` from the controller clock ([`Context::now`](crate::Context)). The step stops at
//! the first `placed`; a refusal of the node or level tries the next candidate, a refusal of the
//! request itself ([`REQUEST_REFUSALS`]) ends the request's attempt.
//!
//! - One placement per request in flight: a request whose `InstancePlaced` the bindings hold
//!   for another attempt (`r4-bind-placement`) is skipped until it is answered; in the control
//!   host an `InstancePlaced` is otherwise bound in the unit of work that published it.
//! - A gang member is skipped: a gang is placed and bound whole (`weft.scheduling.BindGang`),
//!   which this step does not do.
//! - A request whose attempt placed nothing waits before its next attempt ([`crate::waits`]),
//!   from one tick doubling up to 32; a request with no candidate, or without its ties (a
//!   formation not pinned to a cluster yet, a scope no account records), sends nothing and does
//!   not wait.

use serde_json::{json, Value};
use weft_impl::context::timestamp;
use weft_impl::System;
use weft_system::SystemEvent;
use weft_types::accounts::AccountSnapshot;
use weft_types::actor::Actor;
use weft_types::behaviour::{AccountStorage, InstanceStorage};
use weft_types::cluster::{ClusterState, Clusters};
use weft_types::infrastructure::NodeState;
use weft_types::scheduling::{PendingRequests, WorkloadFormations};
use weft_types::shared::{ClusterId, FormationId, IsolationLevel, TenantId};
use weft_types::workloads::InstanceSnapshot;

use crate::waits::Waits;
use crate::{outcome, send, Context, Sent};

/// The command the step sends.
pub const PLACE_INSTANCE: &str = "weft.scheduling.PlaceInstance";

/// The step's name in the table of waits.
pub const STEP: &str = "placement";

/// The refusals of `PlaceInstance` that hold for the request whatever the node and level: the
/// request's attempt ends with them.
pub const REQUEST_REFUSALS: &[&str] = &[
    "decided-in-future",
    "no-such-request",
    "no-such-cluster",
    "not-pending",
    "already-reserved",
    "other-tenant",
    "other-account",
    "instance-of-other-scope",
    "instance-of-other-formation",
    "other-cluster",
    "quota-exceeded",
];

/// The isolation levels from the weakest to the strongest. `None` (no sandbox) is the weakest:
/// only a request of minimum `None` admits it (default deny).
const LEVELS: [IsolationLevel; 5] = [
    IsolationLevel::None,
    IsolationLevel::ProcessSandbox,
    IsolationLevel::Container,
    IsolationLevel::UserKernel,
    IsolationLevel::MicroVm,
];

/// The levels a request of minimum `minimum` admits, from the weakest.
fn admitted(minimum: IsolationLevel) -> &'static [IsolationLevel] {
    let from = LEVELS
        .iter()
        .position(|level| *level == minimum)
        .unwrap_or(LEVELS.len());
    &LEVELS[from..]
}

/// The name of `level` in a command's input.
fn level_name(level: IsolationLevel) -> &'static str {
    match level {
        IsolationLevel::None => "None",
        IsolationLevel::ProcessSandbox => "ProcessSandbox",
        IsolationLevel::Container => "Container",
        IsolationLevel::UserKernel => "UserKernel",
        IsolationLevel::MicroVm => "MicroVm",
    }
}

/// What `PlaceInstance` ties a request to besides its scope.
struct Ties {
    /// The tenant of the account that holds the request's scope.
    tenant_id: TenantId,
    /// The request's formation.
    formation_id: FormationId,
    /// The cluster that runs the formation.
    cluster_id: ClusterId,
}

/// The ties of the pending request `row`: its formation, the one a
/// `weft.architecture.ComponentWorkload` assigns the workload of the request's instance to (the
/// request and the workload name no formation, story:cloud-blocks-layer-refactor-pin), the
/// cluster not deleted whose `formations` list it (`weft.cluster.Clusters`, the pin of
/// `weft.cluster.PinFormation`) and the tenant of the account that holds its scope. `None`
/// where one is missing: an instance or assignment not recorded, a formation not pinned yet, a
/// scope no account records.
fn ties(
    row: &PendingRequests,
    instances: &[InstanceSnapshot],
    assignments: &[WorkloadFormations],
    clusters: &[Clusters],
    accounts: &[AccountSnapshot],
) -> Option<Ties> {
    let workload_id = &instances
        .iter()
        .find(|instance| instance.data.instance_id == row.instance_id)?
        .data
        .workload_id;
    let formation_id = assignments
        .iter()
        .find(|assignment| &assignment.workload_id == workload_id)?
        .formation_id
        .clone();
    let cluster_id = clusters
        .iter()
        .find(|cluster| {
            cluster.state != ClusterState::Deleted && cluster.formations.contains(&formation_id)
        })?
        .cluster_id
        .clone();
    let tenant_id = accounts
        .iter()
        .find(|account| account.data.scope_id.as_ref() == Some(&row.scope_id))?
        .data
        .tenant_id
        .clone();
    Some(Ties {
        tenant_id,
        formation_id,
        cluster_id,
    })
}

/// One attempt per pending request that is due: the commands sent. A view the system cannot
/// read sends nothing.
pub fn tick(system: &mut System, context: &Context<'_>) -> Vec<Sent> {
    let Ok(requests) = system.weft_control.pending_requests() else {
        return Vec::new();
    };
    let Ok(nodes) = system.weft_control.nodes() else {
        return Vec::new();
    };
    let Ok(clusters) = system.weft_control.clusters() else {
        return Vec::new();
    };
    // The account rows, read through the storage port: no view names the scope an account
    // holds (`weft.accounts.AccountTree` has no `scope_id`). The instance rows likewise: no view
    // of weft-control names an instance's workload across scopes.
    let accounts = AccountStorage::list(context.ports);
    let instances = InstanceStorage::list(context.ports);
    let Ok(assignments) = system.weft_control.workload_formations() else {
        return Vec::new();
    };
    let subjects: Vec<String> = requests
        .iter()
        .map(|row| row.request_id.0 .0.clone())
        .collect();
    let mut waits = Waits::load(context.ports.store(), STEP, &subjects);
    let mut sent = Vec::new();
    for (row, subject) in requests.iter().zip(&subjects) {
        if row.gang_id.is_some() {
            continue;
        }
        let in_flight = context.held.iter().any(|event| {
            matches!(event, SystemEvent::SchedulingInstancePlaced(placed)
                if placed.request_id == row.request_id)
        });
        if in_flight || !waits.due(subject) {
            continue;
        }
        let Some(of) = ties(row, &instances, &assignments, &clusters, &accounts) else {
            continue;
        };
        let candidates: Vec<_> = admitted(row.min_isolation)
            .iter()
            .flat_map(|level| {
                nodes
                    .iter()
                    .filter(move |node| {
                        node.state == NodeState::Ready
                            && node
                                .isolation_facts
                                .as_ref()
                                .is_some_and(|facts| facts.levels.contains(level))
                    })
                    .map(move |node| (*level, node))
            })
            .collect();
        if candidates.is_empty() {
            continue;
        }
        let decided_at = timestamp(context.now());
        let mut placed = false;
        for (level, node) in candidates {
            let input = json!({
                "request_id": row.request_id.0 .0,
                "tenant_id": of.tenant_id.0 .0,
                "scope_id": row.scope_id.0 .0,
                "cluster_id": of.cluster_id.0 .0,
                "formation_id": of.formation_id.0 .0,
                "node_id": node.node_id.0 .0,
                "node_pool_id": node.node_pool_id.as_ref().map(|pool| Value::from(pool.0 .0.clone())),
                "zone": node.zone.0,
                "isolation": level_name(level),
                "bindings": [],
                "cluster_node_pool_id": null,
                "topology_group_id": null,
                "peer_groups": [],
                "decided_at": decided_at.0,
            });
            let answer = send(system, PLACE_INSTANCE, Actor::Scheduler, &input);
            let name = outcome(&answer);
            sent.push(answer);
            if name.as_deref() == Some("placed") {
                placed = true;
                break;
            }
            if name
                .as_deref()
                .is_some_and(|name| REQUEST_REFUSALS.contains(&name))
            {
                break;
            }
        }
        if placed {
            waits.succeeded(subject);
        } else {
            waits.failed(subject);
        }
    }
    sent
}
