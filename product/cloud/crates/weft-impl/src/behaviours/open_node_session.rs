//! `weft.nodes.OpenNodeSession` (`PLAN.md` § Obligations, `ess/5-runtime/nodes.yaml`), sent by the
//! node-channel endpoint of the control host as the ControlPlane, in declaration order:
//!
//! - `received-in-future` when the receipt instant the endpoint stamped is after now;
//! - `identity-refused`, decided externally: "the client certificate does not name this node as a
//!   joined node of the cluster". Mutual TLS is a later story (decision A5), so no certificate
//!   names a node yet; what this file decides is the other half, "a joined node", from the
//!   cluster's `weft.infrastructure.Node` row: refused where the node has no row or is
//!   `Provisioning` (not joined) or `Gone`;
//! - `already-open` while the node holds an `Open` session;
//! - `opened` otherwise: a new `Open` session, opened now, last seen at the receipt instant.

use weft_types::behaviour::{NodeSessionStorage, NodeStorage};
use weft_types::infrastructure::NodeState;
use weft_types::nodes::obligations::OpenNodeSessionBehavior;
use weft_types::nodes::{
    NodeIdentityRefused, NodeSessionData, NodeSessionOpened, NodeSessionSnapshot, NodeSessionState, OpenNodeSession,
    OpenNodeSessionOutcome, ReceivedInFuture, SessionAlreadyOpen,
};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::NodeSessionId;

use crate::context::{instant, mint, timestamp};
use crate::Ports;

impl OpenNodeSessionBehavior for Ports {
    fn open_node_session(&mut self, input: OpenNodeSession) -> Result<OpenNodeSessionOutcome, UnmetObligation> {
        let now = self.now();
        // The decoder admits only RFC 3339; an instant it admits that this clock cannot read is
        // a decision that cannot be made, refused naming the command.
        let received_at = instant(&input.received_at.0).ok_or(UnmetObligation {
            capability: "command behaviour",
            source: "weft.nodes.OpenNodeSession",
        })?;
        if received_at > now {
            return Ok(OpenNodeSessionOutcome::ReceivedInFuture { error: ReceivedInFuture });
        }
        let joined = NodeStorage::get(self, &input.node_id)
            .is_some_and(|node| !matches!(node.state, NodeState::Provisioning | NodeState::Gone));
        if !joined {
            return Ok(OpenNodeSessionOutcome::IdentityRefused { error: NodeIdentityRefused });
        }
        if NodeSessionStorage::list(self)
            .iter()
            .any(|row| row.data.node_id == input.node_id && row.state == NodeSessionState::Open)
        {
            return Ok(OpenNodeSessionOutcome::AlreadyOpen { error: SessionAlreadyOpen });
        }
        let session_id = NodeSessionId(Uuid(mint()));
        NodeSessionStorage::put(
            self,
            NodeSessionSnapshot {
                state: NodeSessionState::Open,
                data: NodeSessionData {
                    session_id: session_id.clone(),
                    node_id: input.node_id.clone(),
                    agent_version: input.agent_version.clone(),
                    opened_at: timestamp(now),
                    last_seen_at: input.received_at.0,
                    ended_at: None,
                    end_reason: None,
                },
            },
        );
        Ok(OpenNodeSessionOutcome::Opened {
            node_session_opened: NodeSessionOpened {
                session_id,
                node_id: input.node_id,
                agent_version: input.agent_version,
            },
        })
    }
}
