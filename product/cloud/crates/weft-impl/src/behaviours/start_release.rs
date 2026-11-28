//! `weft.deployments.StartRelease` (`ess/6-delivery/deployments.yaml`), in declaration order. The
//! command has no synthesized scenario (its comment there), so this follows the contract text:
//!
//! - `no-such-release`;
//! - `already-applied`: a release of the same plan is in any state but `Planned` (the addressed
//!   release among them: one that already started answers this, not `wrong-state`);
//! - `release-in-progress`: a release of the same environment is `Applying`;
//! - `gate-not-passed`: an `Active` deploy gate of the release's environment has no `Passed`
//!   check for the release's plan;
//! - `started` (`Planned` to `Applying`), else `wrong-state`.

use weft_types::behaviour::{DeployGateStorage, ReleaseStorage};
use weft_types::deployments::obligations::StartReleaseBehavior;
use weft_types::deployments::{
    GateNotPassed, NotFound, PlanAlreadyApplied, ReleaseInProgress, ReleaseStarted, ReleaseState,
    StartRelease, StartReleaseOutcome, StateConflict,
};
use weft_types::obligation::UnmetObligation;
use weft_types::pipelines::{DeployGateState, GateCheckSnapshot, GateCheckState};

use crate::Ports;

/// The table of `weft.pipelines.GateCheck`: its storage declares no `list`, as no generated
/// behaviour lists it, so the rows are read from the store.
const GATE_CHECK: &str = "weft.pipelines.GateCheck";

impl StartReleaseBehavior for Ports {
    fn start_release(
        &mut self,
        input: StartRelease,
    ) -> Result<StartReleaseOutcome, UnmetObligation> {
        use StartReleaseOutcome as Outcome;
        let Some(mut release) = ReleaseStorage::get(self, &input.release_id) else {
            return Ok(Outcome::NoSuchRelease { error: NotFound });
        };
        let releases = ReleaseStorage::list(self);
        if releases.iter().any(|row| {
            row.data.plan_id == release.data.plan_id && row.state != ReleaseState::Planned
        }) {
            return Ok(Outcome::AlreadyApplied {
                error: PlanAlreadyApplied,
            });
        }
        if releases.iter().any(|row| {
            row.data.environment_id == release.data.environment_id
                && row.state == ReleaseState::Applying
        }) {
            return Ok(Outcome::ReleaseInProgress {
                error: ReleaseInProgress,
            });
        }
        let checks: Vec<GateCheckSnapshot> = self.store().list(GATE_CHECK);
        if DeployGateStorage::list(self).iter().any(|gate| {
            gate.data.environment_id == release.data.environment_id
                && gate.state == DeployGateState::Active
                && !checks.iter().any(|check| {
                    check.data.deploy_gate_id == gate.data.deploy_gate_id
                        && check.data.plan_id == release.data.plan_id
                        && check.state == GateCheckState::Passed
                })
        }) {
            return Ok(Outcome::GateNotPassed {
                error: GateNotPassed,
            });
        }
        if release.state != ReleaseState::Planned {
            return Ok(Outcome::WrongState {
                error: StateConflict,
            });
        }
        release.state = ReleaseState::Applying;
        ReleaseStorage::put(self, release);
        Ok(Outcome::Started {
            release_started: ReleaseStarted {
                release_id: input.release_id,
            },
        })
    }
}
