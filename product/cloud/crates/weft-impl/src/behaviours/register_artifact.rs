//! `weft.artifacts.RegisterArtifact` (`PLAN.md` § Obligations, `ess/3-resources/artifacts.yaml`),
//! in declaration order:
//!
//! - `not-callers-account` (a caller acting in no account counts as another account's),
//!   `unsigned`, `negative-size`;
//! - `bad-signature`, decided externally before any row: "the signature does not verify against
//!   a signing key the account trusts". Which keys an account trusts is OPEN in the
//!   specification (no trust record exists yet), so this file cannot decide it and, like every
//!   external branch the decider table does not list (`crate::context::decider`), does not
//!   take it: the signature is kept as sent, and the node checks it again before start;
//! - `built-from-missing` (only when `built_from` names an artifact), `built-from-foreign`;
//! - `exists` (an artifact with the digest, of any account: one digest per cluster, OPEN D8 in
//!   the specification);
//! - `registered` otherwise.

use weft_types::artifacts::obligations::RegisterArtifactBehavior;
use weft_types::artifacts::{
    ArtifactData, ArtifactExists, ArtifactRegistered, ArtifactSnapshot, ArtifactState,
    ArtifactUnsigned, InvalidAmount, NotFound, RegisterArtifact, RegisterArtifactOutcome,
};
use weft_types::behaviour::{ArtifactStorage, Context};
use weft_types::obligation::UnmetObligation;

use crate::Ports;

impl RegisterArtifactBehavior for Ports {
    fn register_artifact(
        &mut self,
        input: RegisterArtifact,
    ) -> Result<RegisterArtifactOutcome, UnmetObligation> {
        if Context::caller_account_id(self).as_ref() != Some(&input.account_id) {
            return Ok(RegisterArtifactOutcome::NotCallersAccount { error: NotFound });
        }
        if input.signature.is_empty() {
            return Ok(RegisterArtifactOutcome::Unsigned {
                error: ArtifactUnsigned,
            });
        }
        if input.size_bytes < 0 {
            return Ok(RegisterArtifactOutcome::NegativeSize {
                error: InvalidAmount,
            });
        }
        if let Some(built_from) = &input.built_from {
            let Some(recipe) = ArtifactStorage::get(self, built_from) else {
                return Ok(RegisterArtifactOutcome::BuiltFromMissing { error: NotFound });
            };
            if recipe.data.account_id != input.account_id {
                return Ok(RegisterArtifactOutcome::BuiltFromForeign { error: NotFound });
            }
        }
        if ArtifactStorage::list(self)
            .iter()
            .any(|row| row.data.artifact_digest == input.artifact_digest)
        {
            return Ok(RegisterArtifactOutcome::Exists {
                error: ArtifactExists,
            });
        }
        ArtifactStorage::put(
            self,
            ArtifactSnapshot {
                state: ArtifactState::Registered,
                data: ArtifactData {
                    artifact_digest: input.artifact_digest.clone(),
                    account_id: input.account_id.clone(),
                    kind: input.kind,
                    size_bytes: input.size_bytes,
                    signature: input.signature,
                    signer_key_id: input.signer_key_id.clone(),
                    built_from: input.built_from,
                },
            },
        );
        Ok(RegisterArtifactOutcome::Registered {
            artifact_registered: ArtifactRegistered {
                artifact_digest: input.artifact_digest,
                account_id: input.account_id,
                kind: input.kind,
                signer_key_id: input.signer_key_id,
            },
        })
    }
}
