//! R4 level (`ess/components.yaml`, "Level and time steps"):
//! `weft.scheduling.UnassignedPlacements` -> `weft.nodes.Assign` as Scheduler, with
//! `assigned_at` from the controller clock ([`Context::now`](crate::Context)).
//!
//! The step repairs a lost or dropped delivery of `r4-assign` (a node with no open session when
//! the placement was bound, and a gang member, whose placement no `PlacementBound` names): for
//! each bound, unassigned placement, one `Assign` of its instance to its node. An
//! `already-assigned` answer means the instance has an `Active` assignment. Where that
//! assignment is this placement's (`weft.nodes.Assignments`, `placement_decision_id`), only the
//! placement's record lags: the step then sends `weft.scheduling.RecordPlacementAssigned` for
//! the row. Where it is another placement's, the step records nothing
//! (`RecordPlacementAssigned` records that weft.nodes created the assignment of this
//! placement), counts the attempt failed, and leaves the row to the R3 time step on
//! `weft.scheduling.OverdueUnassignedPlacements`, which releases it 120 s after its decision.
//!
//! - One command per placement in flight: a placement whose `PlacementBound` (`r4-assign`) or
//!   whose `InstanceAssigned` (`r4-record-placement-assigned`) the bindings hold for another
//!   attempt is skipped until it is answered.
//! - A placement whose attempt failed waits before its next attempt ([`crate::waits`]), from one
//!   tick doubling up to 32.

use serde_json::json;
use weft_impl::context::timestamp;
use weft_impl::System;
use weft_system::SystemEvent;
use weft_types::actor::Actor;
use weft_types::nodes::AssignmentState;
use weft_types::shared::PlacementDecisionId;

use crate::waits::Waits;
use crate::{outcome, send, Context, Sent};

/// The command the step sends for each row.
pub const ASSIGN: &str = "weft.nodes.Assign";

/// The command the step sends after `already-assigned`.
pub const RECORD_PLACEMENT_ASSIGNED: &str = "weft.scheduling.RecordPlacementAssigned";

/// The step's name in the table of waits.
pub const STEP: &str = "repair";

/// One attempt per bound, unassigned placement that is due: the commands sent. A view the
/// system cannot read sends nothing.
pub fn tick(system: &mut System, context: &Context<'_>) -> Vec<Sent> {
    let Ok(rows) = system.weft_control.unassigned_placements() else {
        return Vec::new();
    };
    let subjects: Vec<String> = rows
        .iter()
        .map(|row| row.decision_id.0 .0.clone())
        .collect();
    let mut waits = Waits::load(context.ports.store(), STEP, &subjects);
    let mut sent = Vec::new();
    for (row, subject) in rows.iter().zip(&subjects) {
        let in_flight = context.held.iter().any(|event| match event {
            SystemEvent::SchedulingPlacementBound(bound) => bound.decision_id == row.decision_id,
            SystemEvent::NodesInstanceAssigned(assigned) => {
                assigned.placement_decision_id == row.decision_id
            }
            _ => false,
        });
        if in_flight || !waits.due(subject) {
            continue;
        }
        let input = json!({
            "instance_id": row.instance_id.0 .0,
            "node_id": row.node_id.0 .0,
            "placement_decision_id": row.decision_id.0 .0,
            "assigned_at": timestamp(context.now()).0,
        });
        let answer = send(system, ASSIGN, Actor::Scheduler, &input);
        let mut done = outcome(&answer).as_deref() == Some("assigned");
        let assigned_before = outcome(&answer).as_deref() == Some("already-assigned");
        sent.push(answer);
        if assigned_before && assigned_to(system, &row.decision_id) {
            let input = json!({"decision_id": row.decision_id.0 .0});
            let answer = send(system, RECORD_PLACEMENT_ASSIGNED, Actor::Scheduler, &input);
            done = outcome(&answer).as_deref() == Some("recorded");
            sent.push(answer);
        }
        if done {
            waits.succeeded(subject);
        } else {
            waits.failed(subject);
        }
    }
    sent
}

/// Whether an `Active` assignment of the placement `decision` exists. A view the system cannot
/// read answers no.
fn assigned_to(system: &System, decision: &PlacementDecisionId) -> bool {
    system.weft_control.assignments().is_ok_and(|rows| {
        rows.iter().any(|row| {
            row.state == AssignmentState::Active && &row.placement_decision_id == decision
        })
    })
}
