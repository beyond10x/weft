//! `weft.deployments.PlanFromSource` (`ess/cloud/11-translation/deployments.yaml`), in the
//! selection precedence of its contract:
//!
//! 1. `not-callers-scope` (a caller acting in no scope counts as another scope's), which reads
//!    no row;
//! 2. the existence checks in declaration order: `no-such-source`, `no-such-environment`;
//! 3. the predicates on the source, in declaration order: `source-of-other-scope`,
//!    `source-not-cloudspec`, `source-withdrawn`; then `environment-foreign` (the environment is
//!    not one of the source's formation);
//! 4. `planned`: one command records a `Recorded` `FormationRevision` of the source's formation
//!    and a `Simulating` `Plan` of it that names the source and no translation.
//!
//! The specification leaves the revision to the implementation (UNMAPPED beside the command):
//! its `number` is one more than the formation's revisions so far, as `CommitRevision` numbers
//! them, and its `document_digest` is the source's architecture digest. That digest is checked
//! first against the file kept under the source digest ([`crate::documents`]), read with
//! [`weft_cloudspec::Submission::read`]: a missing file, one that does not read, or one whose
//! source or architecture digest differs from the row leaves the revision without the document
//! it names, and the command answers its unmet obligation (`501`, nothing written). So does a
//! caller with no principal, whose plan would have no `created_by`.

use weft_types::architecture::{
    FormationRevisionData, FormationRevisionSnapshot, FormationRevisionState,
};
use weft_types::behaviour::{
    Context, DeploymentSourceStorage, EnvironmentStorage, FormationRevisionStorage, PlanStorage,
};
use weft_types::deployments::obligations::PlanFromSourceBehavior;
use weft_types::deployments::{
    DeploymentSourceState, NotFound, PlanCreated, PlanData, PlanFromSource, PlanFromSourceOutcome,
    PlanSnapshot, PlanState, SourceIsWithdrawn, SourceKind, SourceNotCloudspec,
};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::{ArtifactDigest, FormationRevisionId, PlanId};

use crate::context::{mint, timestamp};
use crate::{documents, Ports};

const UNMET: UnmetObligation = UnmetObligation {
    capability: "command behaviour",
    source: "weft.deployments.PlanFromSource",
};

impl PlanFromSourceBehavior for Ports {
    fn plan_from_source(
        &mut self,
        input: PlanFromSource,
    ) -> Result<PlanFromSourceOutcome, UnmetObligation> {
        use PlanFromSourceOutcome as Outcome;
        if Context::caller_scope_id(self).as_ref() != Some(&input.scope_id) {
            return Ok(Outcome::NotCallersScope { error: NotFound });
        }
        let Some(source) = DeploymentSourceStorage::get(self, &input.source_id) else {
            return Ok(Outcome::NoSuchSource { error: NotFound });
        };
        let Some(environment) = EnvironmentStorage::get(self, &input.environment_id) else {
            return Ok(Outcome::NoSuchEnvironment { error: NotFound });
        };
        if source.data.scope_id != input.scope_id {
            return Ok(Outcome::SourceOfOtherScope { error: NotFound });
        }
        if source.data.kind != SourceKind::Cloudspec {
            return Ok(Outcome::SourceNotCloudspec {
                error: SourceNotCloudspec,
            });
        }
        if source.state == DeploymentSourceState::Withdrawn {
            return Ok(Outcome::SourceWithdrawn {
                error: SourceIsWithdrawn,
            });
        }
        if environment.data.formation_id != source.data.formation_id {
            return Ok(Outcome::EnvironmentForeign { error: NotFound });
        }

        let architecture_digest = source.data.architecture_digest.as_ref().ok_or(UNMET)?;
        let bytes = documents::get(self, &source.data.source_digest).ok_or(UNMET)?;
        let submission = weft_cloudspec::Submission::read(&bytes).map_err(|_| UNMET)?;
        if submission.source_digest() != source.data.source_digest.0
            || submission.architecture_digest() != architecture_digest.0
        {
            return Err(UNMET);
        }
        let created_by = Context::caller_principal_id(self).ok_or(UNMET)?;

        let formation_id = source.data.formation_id;
        let revisions = FormationRevisionStorage::list(self)
            .iter()
            .filter(|row| row.data.formation_id == formation_id)
            .count();
        let number = i64::try_from(revisions).map_or(i64::MAX, |count| count.saturating_add(1));
        let revision_id = FormationRevisionId(Uuid(mint()));
        FormationRevisionStorage::put(
            self,
            FormationRevisionSnapshot {
                state: FormationRevisionState::Recorded,
                data: FormationRevisionData {
                    revision_id: revision_id.clone(),
                    formation_id: formation_id.clone(),
                    number,
                    document_digest: ArtifactDigest(architecture_digest.0.clone()),
                },
            },
        );
        let plan_id = PlanId(Uuid(mint()));
        PlanStorage::put(
            self,
            PlanSnapshot {
                state: PlanState::Simulating,
                data: PlanData {
                    plan_id: plan_id.clone(),
                    scope_id: source.data.scope_id,
                    formation_id,
                    environment_id: input.environment_id.clone(),
                    formation_revision_id: revision_id.clone(),
                    translation_id: None,
                    source_id: Some(input.source_id),
                    live_release_id: None,
                    diff: None,
                    simulation: None,
                    created_by,
                    created_at: timestamp(self.now()),
                },
            },
        );
        Ok(Outcome::Planned {
            plan_created: PlanCreated {
                plan_id,
                environment_id: input.environment_id,
                formation_revision_id: revision_id,
            },
        })
    }
}
