//! `weft.nodes.Assign` (`ess/5-runtime/nodes.yaml`), in the selection precedence of its contract
//! (ess/37, the one rule):
//!
//! 1. the input refusal `assigned-in-future` (`assigned_at` after `now`; an instant that is not
//!    RFC 3339 cannot be placed before `now` and is refused the same way);
//! 2. the rows the input names, in declaration order: `no-such-placement`, `no-such-node`;
//! 3. the present-related refusals and the row-set guards, in declaration order:
//!    `placement-mismatch` (the decision's node or instance is not the input's),
//!    `placement-not-bound`, `no-open-session`, `already-assigned` (an `Active` assignment of the
//!    instance), `node-not-ready`;
//! 4. the external branch `isolation-unavailable`: the levels of the node's observed isolation
//!    facts hold neither the decision's isolation nor a stronger level, or the node has none;
//! 5. `assigned`, the branch the contract takes otherwise: an `Active` assignment on the node's
//!    open session at the decision's isolation, desired `Run`, `assigned_at` and
//!    `lease_renewed_at` the input's stamped instant through the conversion
//!    `StampedAt -> Timestamp` ([`crate::conversions`]), and `InstanceAssigned`.

use weft_types::behaviour::{
    AssignmentStorage, NodeSessionStorage, NodeStorage, PlacementDecisionStorage,
};
use weft_types::infrastructure::NodeState;
use weft_types::nodes::obligations::AssignBehavior;
use weft_types::nodes::{
    Assign, AssignOutcome, AssignedInFuture, AssignmentData, AssignmentSnapshot, AssignmentState,
    DesiredState, InstanceAlreadyAssigned, InstanceAssigned, IsolationUnavailable, NoOpenSession,
    NodeNotFound, NodeNotReady, NodeSessionState, PlacementMismatch, PlacementNotBound,
    PlacementNotFound,
};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::scheduling::PlacementDecisionState;
use weft_types::shared::{AssignmentId, IsolationLevel};

use crate::context::{instant, mint};
use crate::{conversions, Ports};

/// The strength of `level`: `None` (no sandbox, cloudspec's weakest level) is the weakest, below
/// `ProcessSandbox`, and `MicroVm` the strongest.
fn rank(level: IsolationLevel) -> u8 {
    match level {
        IsolationLevel::None => 0,
        IsolationLevel::ProcessSandbox => 1,
        IsolationLevel::Container => 2,
        IsolationLevel::UserKernel => 3,
        IsolationLevel::MicroVm => 4,
    }
}

impl AssignBehavior for Ports {
    fn assign(&mut self, input: Assign) -> Result<AssignOutcome, UnmetObligation> {
        let now = self.now();
        if instant(&input.assigned_at.0).is_none_or(|assigned| assigned > now) {
            return Ok(AssignOutcome::AssignedInFuture {
                error: AssignedInFuture,
            });
        }

        let Some(decision) = PlacementDecisionStorage::get(self, &input.placement_decision_id)
        else {
            return Ok(AssignOutcome::NoSuchPlacement {
                error: PlacementNotFound,
            });
        };
        let Some(node) = NodeStorage::get(self, &input.node_id) else {
            return Ok(AssignOutcome::NoSuchNode {
                error: NodeNotFound,
            });
        };

        if decision.data.node_id != input.node_id || decision.data.instance_id != input.instance_id
        {
            return Ok(AssignOutcome::PlacementMismatch {
                error: PlacementMismatch,
            });
        }
        if decision.state != PlacementDecisionState::Bound {
            return Ok(AssignOutcome::PlacementNotBound {
                error: PlacementNotBound,
            });
        }
        let Some(session) = NodeSessionStorage::list(self).into_iter().find(|session| {
            session.data.node_id == input.node_id && session.state == NodeSessionState::Open
        }) else {
            return Ok(AssignOutcome::NoOpenSession {
                error: NoOpenSession,
            });
        };
        if AssignmentStorage::list(self).iter().any(|assignment| {
            assignment.data.instance_id == input.instance_id
                && assignment.state == AssignmentState::Active
        }) {
            return Ok(AssignOutcome::AlreadyAssigned {
                error: InstanceAlreadyAssigned,
            });
        }
        if node.state != NodeState::Ready {
            return Ok(AssignOutcome::NodeNotReady {
                error: NodeNotReady,
            });
        }

        let isolation = decision.data.isolation;
        if node.data.isolation_facts.as_ref().is_none_or(|facts| {
            !facts
                .levels
                .iter()
                .any(|level| rank(*level) >= rank(isolation))
        }) {
            return Ok(AssignOutcome::IsolationUnavailable {
                error: IsolationUnavailable,
            });
        }

        let assigned_at = conversions::stamped(input.assigned_at)?;
        let assignment_id = AssignmentId(Uuid(mint()));
        AssignmentStorage::put(
            self,
            AssignmentSnapshot {
                state: AssignmentState::Active,
                data: AssignmentData {
                    assignment_id: assignment_id.clone(),
                    instance_id: input.instance_id.clone(),
                    node_id: input.node_id.clone(),
                    session_id: session.data.session_id.clone(),
                    placement_decision_id: input.placement_decision_id.clone(),
                    min_isolation: isolation,
                    desired: DesiredState::Run,
                    snapshot_id: None,
                    assigned_at: assigned_at.clone(),
                    lease_renewed_at: assigned_at,
                    ended_at: None,
                },
            },
        );
        Ok(AssignOutcome::Assigned {
            instance_assigned: InstanceAssigned {
                assignment_id,
                instance_id: input.instance_id,
                node_id: input.node_id,
                session_id: session.data.session_id,
                min_isolation: isolation,
                placement_decision_id: input.placement_decision_id,
            },
        })
    }
}
