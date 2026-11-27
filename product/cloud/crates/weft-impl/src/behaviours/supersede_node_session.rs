//! `weft.nodes.SupersedeNodeSession` (`PLAN.md` § Obligations, `ess/5-runtime/nodes.yaml`), sent
//! by the node-channel endpoint of the control host as the ControlPlane when a node dials again
//! while its session is still `Open` (`protocols/node-channel.yaml` `replace-open-session`); the
//! endpoint sends `OpenNodeSession` after it. In declaration order:
//!
//! - `identity-refused`, decided externally: "the client certificate of the redialing connection
//!   does not name node_id as a joined node of the cluster". As in `open_node_session.rs`: no
//!   certificate names a node before mutual TLS (decision A5), and "a joined node" is decided from
//!   the node's row ([`super::open_node_session::joined`]);
//! - `no-such-session` for a session without a row;
//! - `other-node` for a session of another node;
//! - `superseded` for an `Open` session: it moves to `Superseded`, ended now with the reason
//!   `superseded`, and every `Open` exec session carried in it is closed now with the same reason;
//! - `not-open` otherwise.

use weft_types::behaviour::{ExecSessionStorage, NodeSessionStorage};
use weft_types::nodes::obligations::SupersedeNodeSessionBehavior;
use weft_types::nodes::{
    ExecSessionState, NodeIdentityRefused, NodeSessionState, NodeSessionSuperseded, SessionNodeMismatch,
    SessionNotFound, SessionNotOpen, SupersedeNodeSession, SupersedeNodeSessionOutcome,
};
use weft_types::obligation::UnmetObligation;

use super::open_node_session::joined;
use crate::context::timestamp;
use crate::Ports;

/// The reason a superseded session and its exec sessions end with.
const REASON: &str = "superseded";

impl SupersedeNodeSessionBehavior for Ports {
    fn supersede_node_session(
        &mut self,
        input: SupersedeNodeSession,
    ) -> Result<SupersedeNodeSessionOutcome, UnmetObligation> {
        if !joined(self, &input.node_id) {
            return Ok(SupersedeNodeSessionOutcome::IdentityRefused { error: NodeIdentityRefused });
        }
        let Some(mut session) = NodeSessionStorage::get(self, &input.session_id) else {
            return Ok(SupersedeNodeSessionOutcome::NoSuchSession { error: SessionNotFound });
        };
        if session.data.node_id != input.node_id {
            return Ok(SupersedeNodeSessionOutcome::OtherNode { error: SessionNodeMismatch });
        }
        if session.state != NodeSessionState::Open {
            return Ok(SupersedeNodeSessionOutcome::NotOpen { error: SessionNotOpen });
        }
        let now = timestamp(self.now());
        session.state = NodeSessionState::Superseded;
        session.data.ended_at = Some(now.clone());
        session.data.end_reason = Some(REASON.to_owned());
        NodeSessionStorage::put(self, session);
        for mut exec in ExecSessionStorage::list(self) {
            if exec.data.session_id == input.session_id && exec.state == ExecSessionState::Open {
                exec.state = ExecSessionState::Closed;
                exec.data.closed_at = Some(now.clone());
                exec.data.close_reason = Some(REASON.to_owned());
                ExecSessionStorage::put(self, exec);
            }
        }
        Ok(SupersedeNodeSessionOutcome::Superseded {
            node_session_superseded: NodeSessionSuperseded {
                session_id: input.session_id,
                node_id: input.node_id,
            },
        })
    }
}
