//! `weft.scheduling.PlaceInstance` (`ess/5-runtime/scheduling.yaml`), in the selection
//! precedence of its contract (ess/37, the one rule):
//!
//! 1. the input refusal `decided-in-future` (`decided_at` after `now`; an instant that is not
//!    RFC 3339 cannot be placed before `now` and is refused the same way);
//! 2. the rows the input names, in declaration order: `no-such-request`, `no-such-node`,
//!    `no-such-cluster`;
//! 3. the present-related refusals and the row-set guards, in declaration order: `not-pending`,
//!    `already-reserved`, `other-account` (the request's scope is not the input's),
//!    `other-tenant` (no account of the input's tenant holds the input's scope),
//!    `instance-of-other-scope` (the request's instance is missing or of another scope; answers
//!    `NotFound`), `instance-of-other-formation` (no `weft.architecture.ComponentWorkload`
//!    assigns the workload of the request's instance to the formation; answers `NotFound`),
//!    `other-cluster` (the cluster's `formations` do not list the formation:
//!    a formation is placed only in the cluster `weft.cluster.PinFormation` pinned it to),
//!    `binding-of-other-account`, `binding-on-other-node`, `below-microvm`, `below-user-kernel`,
//!    `below-container`, `below-process-sandbox`, `below-cluster-microvm`,
//!    `below-cluster-user-kernel`, `below-cluster-container`, `below-cluster-process-sandbox`
//!    (`None`, no sandbox, is the weakest level, below `ProcessSandbox`),
//!    `outside-cluster-zones`, `outside-zones`, `shared-kernel`,
//!    `node-not-schedulable`, `node-not-connected`, `node-zone-differs`, `node-in-other-cloud`,
//!    `node-pool-differs`, `node-in-other-clusters-pool`, `node-outside-dedicated-pools`,
//!    `cluster-pool-differs`, `group-not-of-node`, `crosses-closed-link`,
//!    `isolation-not-offered`;
//! 4. the external branches, in declaration order, decided from the rows of the same store:
//!    - `no-capacity`: for a class the request claims, the node's free capacity is less than
//!      the request claims. Free capacity is the node's observed capacity (the newest
//!      `weft.inventory.NodeCapacity` of the node by `observed_at`) minus the claims of the
//!      requests of the node's live decisions (`Reserved`, `Bound`, `Evicting`, the set
//!      `holds` names) and minus the claims of the node's live bindings (`Reserved`,
//!      `Active`) that neither one of those decisions nor this placement's input names, so no
//!      claim is taken twice; a node with no observed capacity has none;
//!    - `quota-exceeded`: the request's claims against what its scope holds besides the
//!      request's own instance ([`crate::quota::exceeded_besides`]); a `Pending` instance holds
//!      quota only while a decision of it holds a node ([`crate::quota::holds_quota`]);
//!    - `device-not-wiped`: a `Wiping` device of the node that a binding of the placement, on
//!      that node, names;
//! 5. `placed`, the branch the contract takes otherwise: a `Reserved` decision with the
//!    request's instance, scope, minimum isolation, gang and priority, the input's formation,
//!    the topology
//!    of the input's group (`topology_id`), and
//!    `InstancePlaced`.

use std::collections::HashMap;

use weft_types::behaviour::{
    AccountStorage, BindingStorage, ClaimStorage, ClusterNodePoolStorage, ClusterStorage,
    ComponentWorkloadStorage, DeviceStorage, InstanceStorage, NodeCapacityStorage,
    NodeSessionStorage, NodeStorage, PlacementDecisionStorage, SchedulingRequestStorage,
    TopologyGroupStorage, TopologyLinkStorage,
};
use weft_types::claims::BindingState;
use weft_types::cluster::ClusterNodePoolState;
use weft_types::infrastructure::NodeState;
use weft_types::inventory::DeviceState;
use weft_types::nodes::NodeSessionState;
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::scheduling::obligations::PlaceInstanceBehavior;
use weft_types::scheduling::{
    CrossesClosedLink, DecidedInFuture, DeviceNotWiped, InstancePlaced, IsolationNotOffered,
    IsolationTooWeak, NoCapacity, NodeNotConnected, NodeNotSchedulable, NodeOutsideCluster,
    NotFound, PlaceInstance, PlaceInstanceOutcome, PlacementDecisionData,
    PlacementDecisionSnapshot, PlacementDecisionState, QuotaExceeded, RequestMismatch,
    RequestReserved, SchedulingRequestState, StateConflict, TenantKernelShared, TopologyMismatch,
    ZoneMismatch,
};
use weft_types::shared::{BindingId, IsolationLevel, NodeId, PlacementDecisionId, ResourceAmount};
use weft_types::topology::{TopologyGroupState, TopologyLinkState};

use crate::context::{instant, mint};
use crate::{quota, Ports};

/// Whether a decision in `state` holds its node (`Reserved`, `Bound`, `Evicting`).
fn holds(state: PlacementDecisionState) -> bool {
    matches!(
        state,
        PlacementDecisionState::Reserved
            | PlacementDecisionState::Bound
            | PlacementDecisionState::Evicting
    )
}

/// The capacity of `node`: the amounts of its newest `weft.inventory.NodeCapacity` row by
/// `observed_at` (story:cloud-blocks-layer-refactor-pin: split out of
/// `weft.infrastructure.Node`), or `None` before its first report.
fn newest_capacity(ports: &Ports, node: &NodeId) -> Option<Vec<ResourceAmount>> {
    NodeCapacityStorage::list(ports)
        .into_iter()
        .filter(|row| &row.data.node_id == node)
        .max_by_key(|row| instant(&row.data.observed_at))
        .map(|row| row.data.capacity)
}

/// Whether an instance at `isolation` shares the node's kernel (every level but `MicroVm`).
fn shares_kernel(isolation: IsolationLevel) -> bool {
    !matches!(isolation, IsolationLevel::MicroVm)
}

impl PlaceInstanceBehavior for Ports {
    fn place_instance(
        &mut self,
        input: PlaceInstance,
    ) -> Result<PlaceInstanceOutcome, UnmetObligation> {
        let now = self.now();
        if instant(&input.decided_at.0).is_none_or(|decided| decided > now) {
            return Ok(PlaceInstanceOutcome::DecidedInFuture {
                error: DecidedInFuture,
            });
        }

        let Some(request) = SchedulingRequestStorage::get(self, &input.request_id) else {
            return Ok(PlaceInstanceOutcome::NoSuchRequest { error: NotFound });
        };
        let Some(node) = NodeStorage::get(self, &input.node_id) else {
            return Ok(PlaceInstanceOutcome::NoSuchNode { error: NotFound });
        };
        let Some(cluster) = ClusterStorage::get(self, &input.cluster_id) else {
            return Ok(PlaceInstanceOutcome::NoSuchCluster { error: NotFound });
        };

        if request.state != SchedulingRequestState::Pending {
            return Ok(PlaceInstanceOutcome::NotPending {
                error: StateConflict,
            });
        }
        let decisions = PlacementDecisionStorage::list(self);
        if decisions
            .iter()
            .any(|decision| decision.data.request_id == input.request_id && holds(decision.state))
        {
            return Ok(PlaceInstanceOutcome::AlreadyReserved {
                error: RequestReserved,
            });
        }
        if request.data.scope_id != input.scope_id {
            return Ok(PlaceInstanceOutcome::OtherAccount {
                error: RequestMismatch,
            });
        }
        // Deny by default: a scope that no account of the input's tenant holds is another
        // tenant's.
        if !AccountStorage::list(self).iter().any(|account| {
            account.data.scope_id.as_ref() == Some(&input.scope_id)
                && account.data.tenant_id == input.tenant_id
        }) {
            return Ok(PlaceInstanceOutcome::OtherTenant {
                error: RequestMismatch,
            });
        }
        // Deny by default: a request whose instance row is missing, or is another scope's, is
        // answered as a missing request.
        let instance = InstanceStorage::get(self, &request.data.instance_id)
            .filter(|instance| instance.data.scope_id == input.scope_id);
        let Some(instance) = instance else {
            return Ok(PlaceInstanceOutcome::InstanceOfOtherScope { error: NotFound });
        };
        // Deny by default: an instance whose workload no assignment of the formation names
        // (`weft.architecture.ComponentWorkload`; the request and the workload name no
        // formation) is answered as a missing request.
        if !ComponentWorkloadStorage::list(self).iter().any(|row| {
            row.data.workload_id == instance.data.workload_id
                && row.data.formation_id == input.formation_id
        }) {
            return Ok(PlaceInstanceOutcome::InstanceOfOtherFormation { error: NotFound });
        }
        // Deny by default: a formation the cluster does not list (one not pinned to it, by
        // `weft.cluster.PinFormation`) is another cluster's.
        if !cluster.data.formations.contains(&input.formation_id) {
            return Ok(PlaceInstanceOutcome::OtherCluster {
                error: RequestMismatch,
            });
        }
        let bindings = BindingStorage::list(self);
        let claims = ClaimStorage::list(self);
        let placed: Vec<_> = bindings
            .iter()
            .filter(|binding| input.bindings.contains(&binding.data.binding_id))
            .collect();
        if placed.iter().any(|binding| {
            !claims.iter().any(|claim| {
                claim.data.claim_id == binding.data.claim_id
                    && claim.data.scope_id == input.scope_id
            })
        }) {
            return Ok(PlaceInstanceOutcome::BindingOfOtherAccount {
                error: RequestMismatch,
            });
        }
        if placed.iter().any(|binding| {
            binding
                .data
                .node_id
                .as_ref()
                .is_some_and(|node_id| node_id != &input.node_id)
        }) {
            return Ok(PlaceInstanceOutcome::BindingOnOtherNode {
                error: RequestMismatch,
            });
        }

        let isolation = input.isolation;
        let weaker_than_microvm = matches!(
            isolation,
            IsolationLevel::UserKernel
                | IsolationLevel::Container
                | IsolationLevel::ProcessSandbox
                | IsolationLevel::None
        );
        let weaker_than_user_kernel = matches!(
            isolation,
            IsolationLevel::Container | IsolationLevel::ProcessSandbox | IsolationLevel::None
        );
        let weaker_than_container = matches!(
            isolation,
            IsolationLevel::ProcessSandbox | IsolationLevel::None
        );
        let weaker_than_process_sandbox = isolation == IsolationLevel::None;
        let minimum = request.data.min_isolation;
        if weaker_than_microvm && minimum == IsolationLevel::MicroVm {
            return Ok(PlaceInstanceOutcome::BelowMicrovm {
                error: IsolationTooWeak,
            });
        }
        if weaker_than_user_kernel && minimum == IsolationLevel::UserKernel {
            return Ok(PlaceInstanceOutcome::BelowUserKernel {
                error: IsolationTooWeak,
            });
        }
        if weaker_than_container && minimum == IsolationLevel::Container {
            return Ok(PlaceInstanceOutcome::BelowContainer {
                error: IsolationTooWeak,
            });
        }
        if weaker_than_process_sandbox && minimum == IsolationLevel::ProcessSandbox {
            return Ok(PlaceInstanceOutcome::BelowProcessSandbox {
                error: IsolationTooWeak,
            });
        }
        let cluster_minimum = cluster.data.min_isolation;
        if weaker_than_microvm && cluster_minimum == IsolationLevel::MicroVm {
            return Ok(PlaceInstanceOutcome::BelowClusterMicrovm {
                error: IsolationTooWeak,
            });
        }
        if weaker_than_user_kernel && cluster_minimum == IsolationLevel::UserKernel {
            return Ok(PlaceInstanceOutcome::BelowClusterUserKernel {
                error: IsolationTooWeak,
            });
        }
        if weaker_than_container && cluster_minimum == IsolationLevel::Container {
            return Ok(PlaceInstanceOutcome::BelowClusterContainer {
                error: IsolationTooWeak,
            });
        }
        if weaker_than_process_sandbox && cluster_minimum == IsolationLevel::ProcessSandbox {
            return Ok(PlaceInstanceOutcome::BelowClusterProcessSandbox {
                error: IsolationTooWeak,
            });
        }
        if !cluster.data.zones.contains(&input.zone) {
            return Ok(PlaceInstanceOutcome::OutsideClusterZones {
                error: ZoneMismatch,
            });
        }
        if !request.data.zones.is_empty() && !request.data.zones.contains(&input.zone) {
            return Ok(PlaceInstanceOutcome::OutsideZones {
                error: ZoneMismatch,
            });
        }
        if shares_kernel(isolation)
            && decisions.iter().any(|decision| {
                decision.data.node_id == input.node_id
                    && decision.data.tenant_id != input.tenant_id
                    && shares_kernel(decision.data.isolation)
                    && holds(decision.state)
            })
        {
            return Ok(PlaceInstanceOutcome::SharedKernel {
                error: TenantKernelShared,
            });
        }
        if node.state != NodeState::Ready {
            return Ok(PlaceInstanceOutcome::NodeNotSchedulable {
                error: NodeNotSchedulable,
            });
        }
        if !NodeSessionStorage::list(self).iter().any(|session| {
            session.data.node_id == input.node_id && session.state == NodeSessionState::Open
        }) {
            return Ok(PlaceInstanceOutcome::NodeNotConnected {
                error: NodeNotConnected,
            });
        }
        if node.data.zone != input.zone {
            return Ok(PlaceInstanceOutcome::NodeZoneDiffers {
                error: ZoneMismatch,
            });
        }
        if cluster.data.cloud_id != node.data.cloud_id {
            return Ok(PlaceInstanceOutcome::NodeInOtherCloud {
                error: NodeOutsideCluster,
            });
        }
        // `(defined(node_pool_id) and node_pool_id != input.node_pool_id) or (not
        // defined(node_pool_id) and defined(input.node_pool_id))`: the node's pool and the input's
        // differ, absent counting as a value of its own.
        if node.data.node_pool_id != input.node_pool_id {
            return Ok(PlaceInstanceOutcome::NodePoolDiffers {
                error: RequestMismatch,
            });
        }
        let pools = ClusterNodePoolStorage::list(self);
        let names_input_pool = |pool: &weft_types::cluster::ClusterNodePoolData| {
            pool.node_pool_id.is_some() && pool.node_pool_id == input.node_pool_id
        };
        if pools
            .iter()
            .any(|pool| pool.data.cluster_id != input.cluster_id && names_input_pool(&pool.data))
        {
            return Ok(PlaceInstanceOutcome::NodeInOtherClustersPool {
                error: NodeOutsideCluster,
            });
        }
        if pools.iter().any(|pool| {
            pool.data.cluster_id == input.cluster_id
                && pool.state != ClusterNodePoolState::Removed
                && !pools.iter().any(|other| {
                    other.data.cluster_id == pool.data.cluster_id && names_input_pool(&other.data)
                })
        }) {
            return Ok(PlaceInstanceOutcome::NodeOutsideDedicatedPools {
                error: NodeOutsideCluster,
            });
        }
        if let Some(cluster_pool) = &input.cluster_node_pool_id {
            if pools.iter().any(|pool| {
                &pool.data.cluster_node_pool_id == cluster_pool
                    && (pool.data.cluster_id != input.cluster_id
                        || pool.data.node_pool_id.is_none()
                        || input.node_pool_id.is_none()
                        || pool.data.node_pool_id != input.node_pool_id)
            }) {
                return Ok(PlaceInstanceOutcome::ClusterPoolDiffers {
                    error: TopologyMismatch,
                });
            }
        }
        if let Some(group) = &input.topology_group_id {
            if TopologyGroupStorage::list(self).iter().any(|row| {
                &row.data.topology_group_id == group
                    && (row.state != TopologyGroupState::Active
                        || input
                            .node_pool_id
                            .as_ref()
                            .is_none_or(|pool| !row.data.node_pools.contains(pool)))
            }) {
                return Ok(PlaceInstanceOutcome::GroupNotOfNode {
                    error: TopologyMismatch,
                });
            }
            if TopologyLinkStorage::list(self).iter().any(|link| {
                link.state == TopologyLinkState::Declared
                    && !link.data.crossable
                    && ((&link.data.from_group_id == group
                        && input.peer_groups.contains(&link.data.to_group_id))
                        || (&link.data.to_group_id == group
                            && input.peer_groups.contains(&link.data.from_group_id)))
            }) {
                return Ok(PlaceInstanceOutcome::CrossesClosedLink {
                    error: CrossesClosedLink,
                });
            }
        }
        if node
            .data
            .isolation_facts
            .as_ref()
            .is_none_or(|facts| !facts.levels.contains(&isolation))
        {
            return Ok(PlaceInstanceOutcome::IsolationNotOffered {
                error: IsolationNotOffered,
            });
        }

        // no-capacity: the node's observed capacity (its newest `weft.inventory.NodeCapacity`)
        // minus the claims of its live decisions and minus the claims of its live bindings that
        // none of those decisions and not this placement names: each claim is taken once.
        let mut free: HashMap<String, i64> = HashMap::new();
        for amount in newest_capacity(self, &input.node_id).iter().flatten() {
            *free.entry(amount.resource_class.0.clone()).or_default() += amount.amount;
        }
        let mut counted: Vec<&BindingId> = input.bindings.iter().collect();
        for decision in &decisions {
            if decision.data.node_id != input.node_id || !holds(decision.state) {
                continue;
            }
            // A decision whose request row is gone is counted through its bindings alone.
            let Some(held) = SchedulingRequestStorage::get(self, &decision.data.request_id) else {
                continue;
            };
            for claim in &held.data.claims {
                *free.entry(claim.class.0.clone()).or_default() -= claim.amount;
            }
            counted.extend(&decision.data.bindings);
        }
        for binding in &bindings {
            let live = matches!(binding.state, BindingState::Reserved | BindingState::Active);
            if !live
                || binding.data.node_id.as_ref() != Some(&input.node_id)
                || counted.contains(&&binding.data.binding_id)
            {
                continue;
            }
            if let Some(claim) = claims
                .iter()
                .find(|claim| claim.data.claim_id == binding.data.claim_id)
            {
                *free.entry(claim.data.class_name.0.clone()).or_default() -= claim.data.amount;
            }
        }
        let mut wanted: HashMap<String, i64> = HashMap::new();
        for claim in &request.data.claims {
            *wanted.entry(claim.class.0.clone()).or_default() += claim.amount;
        }
        if wanted
            .iter()
            .any(|(class, amount)| *amount > free.get(class).copied().unwrap_or(0))
        {
            return Ok(PlaceInstanceOutcome::NoCapacity { error: NoCapacity });
        }
        if quota::exceeded_besides(
            self,
            &request.data.scope_id,
            &request.data.claims,
            &request.data.instance_id,
        ) {
            return Ok(PlaceInstanceOutcome::QuotaExceeded {
                error: QuotaExceeded,
            });
        }
        let wiping: Vec<_> = DeviceStorage::list(self)
            .into_iter()
            .filter(|device| {
                device.data.node_id == input.node_id && device.state == DeviceState::Wiping
            })
            .collect();
        if wiping.iter().any(|device| {
            placed.iter().any(|binding| {
                binding.data.node_id.as_ref() == Some(&device.data.node_id)
                    && binding.data.device_ids.contains(&device.data.device_id)
            })
        }) {
            return Ok(PlaceInstanceOutcome::DeviceNotWiped {
                error: DeviceNotWiped,
            });
        }

        let decision_id = PlacementDecisionId(Uuid(mint()));
        let data = request.data;
        // `topology_id: {related: {via: input.topology_group_id, field: topology_id}}`: the
        // topology of the group, absent with the group (or with its row).
        let topology_id = input.topology_group_id.as_ref().and_then(|group| {
            TopologyGroupStorage::get(self, group).map(|row| row.data.topology_id)
        });
        PlacementDecisionStorage::put(
            self,
            PlacementDecisionSnapshot {
                state: PlacementDecisionState::Reserved,
                data: PlacementDecisionData {
                    decision_id: decision_id.clone(),
                    request_id: input.request_id.clone(),
                    instance_id: data.instance_id,
                    tenant_id: input.tenant_id,
                    scope_id: data.scope_id,
                    node_id: input.node_id.clone(),
                    zone: input.zone,
                    formation_id: input.formation_id,
                    topology_group_id: input.topology_group_id,
                    topology_id,
                    isolation,
                    min_isolation: data.min_isolation,
                    assigned: false,
                    bindings: input.bindings.clone(),
                    gang_id: data.gang_id,
                    priority_value: data.priority_value,
                    preemptible: data.preemptible,
                    grace_seconds: data.grace_seconds,
                    decided_at: input.decided_at,
                    preempted_by: None,
                    preemption_reason: None,
                    evicting_since: None,
                    released_at: None,
                },
            },
        );
        Ok(PlaceInstanceOutcome::Placed {
            instance_placed: InstancePlaced {
                decision_id,
                request_id: input.request_id,
                node_id: input.node_id,
                isolation,
                bindings: input.bindings,
            },
        })
    }
}
