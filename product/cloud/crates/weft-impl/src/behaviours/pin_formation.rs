//! `weft.cluster.PinFormation` (`PLAN.md` § Obligations, `ess/3-resources/cluster.yaml`), in
//! declaration order:
//!
//! - `not-callers-account` (a caller acting in no scope counts as another scope's);
//! - `no-such-cluster`, `no-such-formation`;
//! - `formation-foreign` (the formation is another scope's), `cluster-foreign` (a cluster held
//!   by another scope, or deleted; the default cluster, held by no scope, takes every scope),
//!   `formation-retired`;
//! - `already-pinned` (a cluster, this one or another, lists the formation: a formation runs in
//!   one cluster);
//! - `pinned` otherwise: the formation is appended to the cluster's `formations`, which
//!   `weft.scheduling.PlaceInstance` reads (`other-cluster`).

use weft_types::architecture::FormationState;
use weft_types::behaviour::{ClusterStorage, Context, FormationStorage};
use weft_types::cluster::obligations::PinFormationBehavior;
use weft_types::cluster::{
    ClusterState, FormationAlreadyPinned, FormationPinned, PinFormation, PinFormationOutcome,
};
use weft_types::obligation::UnmetObligation;
use weft_types::topology::{NotFound, StateConflict};

use crate::Ports;

impl PinFormationBehavior for Ports {
    fn pin_formation(
        &mut self,
        input: PinFormation,
    ) -> Result<PinFormationOutcome, UnmetObligation> {
        if Context::caller_scope_id(self).as_ref() != Some(&input.scope_id) {
            return Ok(PinFormationOutcome::NotCallersAccount { error: NotFound });
        }
        let Some(mut cluster) = ClusterStorage::get(self, &input.cluster_id) else {
            return Ok(PinFormationOutcome::NoSuchCluster { error: NotFound });
        };
        let Some(formation) = FormationStorage::get(self, &input.formation_id) else {
            return Ok(PinFormationOutcome::NoSuchFormation { error: NotFound });
        };
        if formation.data.scope_id != input.scope_id {
            return Ok(PinFormationOutcome::FormationForeign { error: NotFound });
        }
        if cluster
            .data
            .scope_id
            .as_ref()
            .is_some_and(|holder| holder != &input.scope_id)
            || cluster.state == ClusterState::Deleted
        {
            return Ok(PinFormationOutcome::ClusterForeign { error: NotFound });
        }
        if formation.state == FormationState::Retired {
            return Ok(PinFormationOutcome::FormationRetired {
                error: StateConflict,
            });
        }
        if ClusterStorage::list(self)
            .iter()
            .any(|row| row.data.formations.contains(&input.formation_id))
        {
            return Ok(PinFormationOutcome::AlreadyPinned {
                error: FormationAlreadyPinned,
            });
        }
        cluster.data.formations.push(input.formation_id.clone());
        ClusterStorage::put(self, cluster);
        Ok(PinFormationOutcome::Pinned {
            formation_pinned: FormationPinned {
                cluster_id: input.cluster_id,
                formation_id: input.formation_id,
            },
        })
    }
}
