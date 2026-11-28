//! Where the workloads of a placed cloudspec scope run (`story:slot-node-pool-placement`).
//!
//! The apply route resolves the `placement` of a part's scope (`weft.scope.Placement`: a pool by
//! name or a host by name) and writes it into the `node_selector` of the workload's
//! `DaemonSettings` under the keys below, as the identity of the pool or node. A part with
//! `per: <scope>` also names that scope's placement under the `per-` keys. A selector with none of
//! these keys selects nothing here: the workload gets no instance from the level step.
//!
//! - The scope nodes of a workload ([`Placed::scope`]): the `Ready` nodes of its `per` scope, or
//!   of its own scope when it has no `per`. The level step of `weft-controllers` gives each scope
//!   node one instance (`weft.workloads.Instance.scope_node_id`), and never a second.
//! - The nodes an instance may run on ([`Placed::runs_on`]): its scope node, for a workload with
//!   no `per`; the nodes of its own scope otherwise. The Scheduler's step offers no other node,
//!   so an instance of a node is never placed on another (decision D-S2 A).
//! - A node's name ([`node_name`]) is its identity: `weft.infrastructure.Node` has no name field.
//!   A `host` names a node by it, and each `${instance}` of an instance's processes
//!   ([`INSTANCE_WORD`], [`with_instance`]) is replaced by its scope node's name.

use std::collections::BTreeMap;

use weft_types::shared::{NodeId, NodePoolId};
use weft_types::workloads::{Process, ShapeSettings};

/// The selector key of a scope placed on a pool: the pool's identity.
pub const POOL: &str = "weft.placement/pool";
/// The selector key of a scope placed on a host: the node's identity.
pub const HOST: &str = "weft.placement/host";
/// [`POOL`] of the scope a part is `per`.
pub const PER_POOL: &str = "weft.placement/per-pool";
/// [`HOST`] of the scope a part is `per`.
pub const PER_HOST: &str = "weft.placement/per-host";

/// The word of a process's command or environment that names its scope instance.
pub const INSTANCE_WORD: &str = "${instance}";

/// The nodes of one scope's placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nodes {
    /// Every node of the pool.
    Pool(NodePoolId),
    /// The one node.
    Host(NodeId),
}

impl Nodes {
    /// Whether the node `node_id`, of the pool `node_pool_id`, is one of these nodes.
    pub fn holds(&self, node_id: &NodeId, node_pool_id: Option<&NodePoolId>) -> bool {
        match self {
            Nodes::Pool(pool) => node_pool_id == Some(pool),
            Nodes::Host(id) => node_id == id,
        }
    }

    /// The selector entry of these nodes, under the `per-` key where `per`.
    pub fn entry(&self, per: bool) -> (&'static str, String) {
        match (self, per) {
            (Nodes::Pool(pool), false) => (POOL, pool.0 .0.clone()),
            (Nodes::Pool(pool), true) => (PER_POOL, pool.0 .0.clone()),
            (Nodes::Host(node), false) => (HOST, node.0 .0.clone()),
            (Nodes::Host(node), true) => (PER_HOST, node.0 .0.clone()),
        }
    }
}

/// The placement a workload's selector records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    /// The placement of its own scope, if any.
    pub own: Option<Nodes>,
    /// The placement of the scope it is `per`, if any.
    pub per: Option<Nodes>,
}

impl Placed {
    /// The placement `settings` record: `None` for a shape other than `Daemon` and for a selector
    /// with none of the keys.
    pub fn of(settings: &ShapeSettings) -> Option<Self> {
        let ShapeSettings::Daemon(daemon) = settings else {
            return None;
        };
        let selector = &daemon.node_selector;
        let nodes = |pool: &str, host: &str| {
            selector
                .get(pool)
                .map(|id| Nodes::Pool(NodePoolId(weft_types::primitives::Uuid(id.clone()))))
                .or_else(|| {
                    selector
                        .get(host)
                        .map(|id| Nodes::Host(NodeId(weft_types::primitives::Uuid(id.clone()))))
                })
        };
        let placed = Self {
            own: nodes(POOL, HOST),
            per: nodes(PER_POOL, PER_HOST),
        };
        (placed.own.is_some() || placed.per.is_some()).then_some(placed)
    }

    /// The nodes whose each gets one instance: those of the `per` scope, else of the own scope.
    pub fn scope(&self) -> Option<&Nodes> {
        self.per.as_ref().or(self.own.as_ref())
    }

    /// Whether an instance of the scope node `scope_node` may run on the node `node_id` of the
    /// pool `node_pool_id`: on its scope node with no `per`; with `per`, on a node of the own
    /// scope (any node when the own scope has no placement). An instance with no scope node may
    /// run on a node of the own scope.
    pub fn runs_on(
        &self,
        scope_node: Option<&NodeId>,
        node_id: &NodeId,
        node_pool_id: Option<&NodePoolId>,
    ) -> bool {
        match (scope_node, &self.per) {
            (Some(scope_node), None) => node_id == scope_node,
            _ => self
                .own
                .as_ref()
                .is_none_or(|own| own.holds(node_id, node_pool_id)),
        }
    }

    /// The pool every node an instance runs on belongs to: the own scope's pool, if any.
    pub fn pool(&self) -> Option<&NodePoolId> {
        match &self.own {
            Some(Nodes::Pool(pool)) => Some(pool),
            _ => None,
        }
    }
}

/// The selector of a workload whose own scope is placed on `own` and which is `per` the scope
/// placed on `per`.
pub fn selector(own: Option<&Nodes>, per: Option<&Nodes>) -> BTreeMap<String, String> {
    own.map(|nodes| nodes.entry(false))
        .into_iter()
        .chain(per.map(|nodes| nodes.entry(true)))
        .map(|(key, id)| (key.to_owned(), id))
        .collect()
}

/// The name of `node`: its identity (`weft.infrastructure.Node` has no name).
pub fn node_name(node: &NodeId) -> String {
    node.0 .0.clone()
}

/// `processes` with each [`INSTANCE_WORD`] of their commands and environment values replaced by
/// `name`.
pub fn with_instance(processes: Vec<Process>, name: &str) -> Vec<Process> {
    processes
        .into_iter()
        .map(|mut process| {
            for word in &mut process.command {
                *word = word.replace(INSTANCE_WORD, name);
            }
            for value in process.environment.values_mut() {
                *value = value.replace(INSTANCE_WORD, name);
            }
            process
        })
        .collect()
}
