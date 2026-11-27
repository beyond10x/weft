//! `weft.infrastructure.RecordNodeObservation` (`PLAN.md` § Obligations,
//! `ess/1-infrastructure/infrastructure.yaml`), sent by the node agent, in declaration order:
//!
//! - `foreign-node`, decided externally: "the calling node agent's session is not the addressed
//!   node's". The node agent's identity is its mutual-TLS certificate, a later story (decision
//!   A5); until then a request carries no node identity to compare, so this outcome is never
//!   taken here;
//! - `no-such-node` for a node without a row;
//! - `ready` for a `Joining` node: it moves to `Ready` with what was observed;
//! - `observed` for a `Ready`, `Cordoned` or `Draining` node: what was observed is updated;
//! - `not-joined` otherwise (`Provisioning`, `Gone`).

use weft_types::behaviour::NodeStorage;
use weft_types::infrastructure::obligations::RecordNodeObservationBehavior;
use weft_types::infrastructure::{
    NodeNotJoined, NodeObserved, NodeReady, NodeState, NotFound, RecordNodeObservation, RecordNodeObservationOutcome,
};
use weft_types::obligation::UnmetObligation;

use crate::Ports;

impl RecordNodeObservationBehavior for Ports {
    fn record_node_observation(
        &mut self,
        input: RecordNodeObservation,
    ) -> Result<RecordNodeObservationOutcome, UnmetObligation> {
        let Some(mut node) = NodeStorage::get(self, &input.node_id) else {
            return Ok(RecordNodeObservationOutcome::NoSuchNode { error: NotFound });
        };
        let ready = match node.state {
            NodeState::Joining => true,
            NodeState::Ready | NodeState::Cordoned | NodeState::Draining => false,
            NodeState::Provisioning | NodeState::Gone => {
                return Ok(RecordNodeObservationOutcome::NotJoined { error: NodeNotJoined });
            }
        };
        if ready {
            node.state = NodeState::Ready;
        }
        node.data.capacity = Some(input.capacity.clone());
        node.data.isolation_facts = Some(input.isolation_facts.clone());
        node.data.observed_at = Some(input.observed_at.clone());
        NodeStorage::put(self, node);
        Ok(if ready {
            RecordNodeObservationOutcome::Ready {
                node_ready: NodeReady {
                    node_id: input.node_id,
                    capacity: input.capacity,
                    isolation_facts: input.isolation_facts,
                    observed_at: input.observed_at,
                },
            }
        } else {
            RecordNodeObservationOutcome::Observed {
                node_observed: NodeObserved {
                    node_id: input.node_id,
                    capacity: input.capacity,
                    isolation_facts: input.isolation_facts,
                    observed_at: input.observed_at,
                },
            }
        })
    }
}
