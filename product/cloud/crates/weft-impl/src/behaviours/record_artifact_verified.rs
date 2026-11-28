//! `weft.workloads.RecordArtifactVerified` (`ess/4-applications/workloads.yaml`), sent by the
//! node agent before it starts an instance's processes, in the selection precedence of its
//! contract (ess/37, the one rule):
//!
//! 1. the row the input names: `no-such-instance`;
//! 2. the present-related refusals, in declaration order: `wrong-node` (the instance is on no node,
//!    or on another), `digest-mismatch` (the instance's `artifact_digest` is not the input's),
//!    `no-artifact` (no `weft.artifacts.Artifact` row of the input's digest), `revoked` (that
//!    row reads `Revoked`);
//! 3. `starting`, the move `start` from `Scheduled`: the instance reads `Starting` with health
//!    `Starting`, and `InstanceStarting` and `InstanceHealthStarting` are published;
//! 4. `wrong-state` from any other state.

use weft_types::artifacts::ArtifactState;
use weft_types::behaviour::{ArtifactStorage, InstanceStorage};
use weft_types::obligation::UnmetObligation;
use weft_types::workloads::obligations::RecordArtifactVerifiedBehavior;
use weft_types::workloads::{
    ArtifactDigestMismatch, ArtifactIsRevoked, ArtifactNotFound, InstanceHealth,
    InstanceHealthStarting, InstanceStarting, InstanceState, NotFound, RecordArtifactVerified,
    RecordArtifactVerifiedOutcome, StateConflict, WrongNode,
};

use crate::Ports;

impl RecordArtifactVerifiedBehavior for Ports {
    fn record_artifact_verified(
        &mut self,
        input: RecordArtifactVerified,
    ) -> Result<RecordArtifactVerifiedOutcome, UnmetObligation> {
        let Some(mut instance) = InstanceStorage::get(self, &input.instance_id) else {
            return Ok(RecordArtifactVerifiedOutcome::NoSuchInstance { error: NotFound });
        };
        if instance.data.node_id.as_ref() != Some(&input.node_id) {
            return Ok(RecordArtifactVerifiedOutcome::WrongNode { error: WrongNode });
        }
        if instance.data.artifact_digest != input.artifact_digest {
            return Ok(RecordArtifactVerifiedOutcome::DigestMismatch {
                error: ArtifactDigestMismatch,
            });
        }
        let Some(artifact) = ArtifactStorage::get(self, &input.artifact_digest) else {
            return Ok(RecordArtifactVerifiedOutcome::NoArtifact {
                error: ArtifactNotFound,
            });
        };
        if artifact.state == ArtifactState::Revoked {
            return Ok(RecordArtifactVerifiedOutcome::Revoked {
                error: ArtifactIsRevoked,
            });
        }
        if instance.state != InstanceState::Scheduled {
            return Ok(RecordArtifactVerifiedOutcome::WrongState {
                error: StateConflict,
            });
        }

        instance.state = InstanceState::Starting;
        instance.data.health = InstanceHealth::Starting;
        let workload_id = instance.data.workload_id.clone();
        InstanceStorage::put(self, instance);
        Ok(RecordArtifactVerifiedOutcome::Starting {
            instance_starting: InstanceStarting {
                instance_id: input.instance_id.clone(),
                artifact_digest: input.artifact_digest,
            },
            instance_health_starting: InstanceHealthStarting {
                instance_id: input.instance_id,
                workload_id,
            },
        })
    }
}
