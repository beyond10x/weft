//! The Scheduler's decision step (`story:skeleton-controllers`): `weft.scheduling.PendingRequests`,
//! with candidates from `weft.infrastructure.Nodes` -> `weft.scheduling.PlaceInstance` as
//! Scheduler.
//!
//! For each pending request, the candidates in order: each isolation level the request admits,
//! from its minimum up to `MicroVm` (the weakest first, so a stronger level is not taken where a
//! weaker one serves), and for each level every `Ready` node, in the view's order, whose
//! isolation facts offer it. The input carries the request's tenant, account and cluster, the
//! node's zone and pool, no bindings, no topology (the default cluster has none) and
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
//!   from one tick doubling up to 32; a request with no candidate sends nothing and does not
//!   wait.

use serde_json::{json, Value};
use weft_impl::context::timestamp;
use weft_impl::System;
use weft_system::SystemEvent;
use weft_types::actor::Actor;
use weft_types::infrastructure::NodeState;
use weft_types::shared::IsolationLevel;

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
    "other-cluster",
    "quota-exceeded",
];

/// The isolation levels from the weakest to the strongest.
const LEVELS: [IsolationLevel; 4] = [
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
        IsolationLevel::ProcessSandbox => "ProcessSandbox",
        IsolationLevel::Container => "Container",
        IsolationLevel::UserKernel => "UserKernel",
        IsolationLevel::MicroVm => "MicroVm",
    }
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
                "tenant_id": row.tenant_id.0 .0,
                "account_id": row.account_id.0 .0,
                "cluster_id": row.cluster_id.0 .0,
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
