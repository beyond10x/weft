//! `weft.formations.CommitRevision` (`PLAN.md` § Obligations, `ess/4-applications/formations.yaml`),
//! in declaration order: `not-callers-account` (a caller acting in no account counts as another
//! account's), `no-such-formation`, `formation-of-other-account`, `retired`, `empty` (the
//! formation has no component), `committed` otherwise.
//!
//! The specification leaves two values to the implementation:
//!
//! - `number`: 1 for a formation's first revision, one more than its revisions so far for each
//!   next;
//! - `document_digest`: `sha256:<hex>` over the formation's document in a canonical form, so the
//!   same document gives the same digest. The canonical form (OPEN in the specification) is one
//!   JSON array: the formation's name, then its components (name, internal name, on the
//!   border) and its environments (name, protected), each sorted by name. Links, routes and
//!   config join the document with the stories that implement their commands; no row of them
//!   is written on the skeleton path.

use serde_json::json;
use sha2::{Digest, Sha256};
use weft_types::behaviour::{
    ComponentStorage, Context, EnvironmentStorage, FormationRevisionStorage, FormationStorage,
};
use weft_types::formations::obligations::CommitRevisionBehavior;
use weft_types::formations::{
    CommitRevision, CommitRevisionOutcome, EmptyFormation, FormationRevisionData,
    FormationRevisionSnapshot, FormationRevisionState, FormationSnapshot, FormationState,
    FormationStateConflict, NoSuchFormation, RevisionCommitted,
};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::{ArtifactDigest, FormationRevisionId};

use crate::context::mint;
use crate::Ports;

impl CommitRevisionBehavior for Ports {
    fn commit_revision(
        &mut self,
        input: CommitRevision,
    ) -> Result<CommitRevisionOutcome, UnmetObligation> {
        if Context::caller_account_id(self).as_ref() != Some(&input.account_id) {
            return Ok(CommitRevisionOutcome::NotCallersAccount {
                error: NoSuchFormation,
            });
        }
        let Some(formation) = FormationStorage::get(self, &input.formation_id) else {
            return Ok(CommitRevisionOutcome::NoSuchFormation {
                error: NoSuchFormation,
            });
        };
        if formation.data.account_id != input.account_id {
            return Ok(CommitRevisionOutcome::FormationOfOtherAccount {
                error: NoSuchFormation,
            });
        }
        if formation.state == FormationState::Retired {
            return Ok(CommitRevisionOutcome::Retired {
                error: FormationStateConflict,
            });
        }
        if !ComponentStorage::list(self)
            .iter()
            .any(|row| row.data.formation_id == input.formation_id)
        {
            return Ok(CommitRevisionOutcome::Empty {
                error: EmptyFormation,
            });
        }
        let revisions = FormationRevisionStorage::list(self)
            .iter()
            .filter(|row| row.data.formation_id == input.formation_id)
            .count();
        let number = i64::try_from(revisions).map_or(i64::MAX, |count| count.saturating_add(1));
        let document_digest = document_digest(self, &formation);
        let revision_id = FormationRevisionId(Uuid(mint()));
        FormationRevisionStorage::put(
            self,
            FormationRevisionSnapshot {
                state: FormationRevisionState::Recorded,
                data: FormationRevisionData {
                    revision_id: revision_id.clone(),
                    formation_id: input.formation_id.clone(),
                    number,
                    document_digest: document_digest.clone(),
                },
            },
        );
        Ok(CommitRevisionOutcome::Committed {
            revision_committed: RevisionCommitted {
                revision_id,
                formation_id: input.formation_id,
                number,
                document_digest,
            },
        })
    }
}

/// The digest of `formation`'s document in its canonical form (see the module's text).
fn document_digest(ports: &Ports, formation: &FormationSnapshot) -> ArtifactDigest {
    let id = &formation.data.formation_id;
    let mut components: Vec<_> = ComponentStorage::list(ports)
        .into_iter()
        .filter(|row| &row.data.formation_id == id)
        .map(|row| {
            json!([
                row.data.name.0,
                row.data.internal_name.0,
                row.data.on_border
            ])
        })
        .collect();
    components.sort_by(|a, b| a[0].as_str().cmp(&b[0].as_str()));
    let mut environments: Vec<_> = EnvironmentStorage::list(ports)
        .into_iter()
        .filter(|row| &row.data.formation_id == id)
        .map(|row| json!([row.data.name.0, row.data.protected]))
        .collect();
    environments.sort_by(|a, b| a[0].as_str().cmp(&b[0].as_str()));
    let document = json!([formation.data.name.0, components, environments]).to_string();
    let hex: String = Sha256::digest(document.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    ArtifactDigest(format!("sha256:{hex}"))
}
