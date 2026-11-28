//! `weft.deployments.SubmitDeploymentSource` (`ess/cloud/11-translation/deployments.yaml`), in
//! the selection precedence of its contract:
//!
//! 1. the input guards in declaration order: `not-callers-scope` (a caller acting in no scope
//!    counts as another scope's), `values-not-helm`, `not-a-spec-source`, then the guards on the
//!    kind, which read no row: `translator-required` (any kind but `Cloudspec` names a
//!    translator), `translator-not-allowed` (a `Cloudspec` source names none),
//!    `digests-required` (a `Cloudspec` source names both digests), `digests-not-cloudspec`;
//! 2. the existence checks in declaration order: `formation-missing`, `no-translator` (read only
//!    where the input names a translator: a `Cloudspec` source names none and reads no row);
//! 3. the predicates on those rows: `formation-foreign`, `translator-not-for-source` (not
//!    `Registered`, or of another kind or format version), `values-environment-foreign` (a
//!    values file names an environment that is not one of the formation's);
//! 4. `submitted`: a `Submitted` source with the values sent, both digests for `Cloudspec`, the
//!    caller's principal as `submitted_by` and the context clock's `submitted_at`. A caller with
//!    no principal leaves the submitter undecidable: the command answers its unmet obligation
//!    (`501`, nothing written) rather than record a submitter it does not know.

use weft_types::behaviour::{
    Context, DeploymentSourceStorage, EnvironmentStorage, FormationStorage, TranslatorStorage,
};
use weft_types::deployments::obligations::SubmitDeploymentSourceBehavior;
use weft_types::deployments::{
    DeploymentSourceData, DeploymentSourceSnapshot, DeploymentSourceState, DigestsOnlyForCloudspec,
    DigestsRequired, NoTranslator, NotASpecSource, NotFound, SourceKind, SourceSubmitted,
    SubmitDeploymentSource, SubmitDeploymentSourceOutcome, TranslatorNotAllowed,
    TranslatorRequired, TranslatorState, ValuesOnlyForHelm,
};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::DeploymentSourceId;

use crate::context::{mint, timestamp};
use crate::Ports;

impl SubmitDeploymentSourceBehavior for Ports {
    fn submit_deployment_source(
        &mut self,
        input: SubmitDeploymentSource,
    ) -> Result<SubmitDeploymentSourceOutcome, UnmetObligation> {
        use SubmitDeploymentSourceOutcome as Outcome;
        if Context::caller_scope_id(self).as_ref() != Some(&input.scope_id) {
            return Ok(Outcome::NotCallersScope { error: NotFound });
        }
        if input.kind != SourceKind::Helm && !input.helm_values.is_empty() {
            return Ok(Outcome::ValuesNotHelm {
                error: ValuesOnlyForHelm,
            });
        }
        if input.embedded_in.is_some()
            && matches!(
                input.kind,
                SourceKind::Weft | SourceKind::Compose | SourceKind::Helm | SourceKind::Kubernetes
            )
        {
            return Ok(Outcome::NotASpecSource {
                error: NotASpecSource,
            });
        }
        let cloudspec = input.kind == SourceKind::Cloudspec;
        if !cloudspec && input.translator_id.is_none() {
            return Ok(Outcome::TranslatorRequired {
                error: TranslatorRequired,
            });
        }
        if cloudspec && input.translator_id.is_some() {
            return Ok(Outcome::TranslatorNotAllowed {
                error: TranslatorNotAllowed,
            });
        }
        if cloudspec && (input.architecture_digest.is_none() || input.binding_digest.is_none()) {
            return Ok(Outcome::DigestsRequired {
                error: DigestsRequired,
            });
        }
        if !cloudspec && (input.architecture_digest.is_some() || input.binding_digest.is_some()) {
            return Ok(Outcome::DigestsNotCloudspec {
                error: DigestsOnlyForCloudspec,
            });
        }

        let Some(formation) = FormationStorage::get(self, &input.formation_id) else {
            return Ok(Outcome::FormationMissing { error: NotFound });
        };
        let translator = match &input.translator_id {
            Some(id) => match TranslatorStorage::get(self, id) {
                Some(row) => Some(row),
                None => {
                    return Ok(Outcome::NoTranslator {
                        error: NoTranslator,
                    })
                }
            },
            None => None,
        };

        if formation.data.scope_id != input.scope_id {
            return Ok(Outcome::FormationForeign { error: NotFound });
        }
        if translator.is_some_and(|row| {
            row.state != TranslatorState::Registered
                || row.data.source_kind != input.kind
                || row.data.source_format_version != input.format_version
        }) {
            return Ok(Outcome::TranslatorNotForSource {
                error: NoTranslator,
            });
        }
        if !input.helm_values.is_empty() {
            let environments = EnvironmentStorage::list(self);
            if input.helm_values.iter().any(|values| {
                !environments.iter().any(|environment| {
                    environment.data.environment_id == values.environment_id
                        && environment.data.formation_id == input.formation_id
                })
            }) {
                return Ok(Outcome::ValuesEnvironmentForeign { error: NotFound });
            }
        }

        let submitted_by = Context::caller_principal_id(self).ok_or(UnmetObligation {
            capability: "command behaviour",
            source: "weft.deployments.SubmitDeploymentSource",
        })?;
        let source_id = DeploymentSourceId(Uuid(mint()));
        let submitted = SourceSubmitted {
            source_id: source_id.clone(),
            scope_id: input.scope_id.clone(),
            formation_id: input.formation_id.clone(),
            kind: input.kind,
            source_digest: input.source_digest.clone(),
        };
        DeploymentSourceStorage::put(
            self,
            DeploymentSourceSnapshot {
                state: DeploymentSourceState::Submitted,
                data: DeploymentSourceData {
                    source_id,
                    scope_id: input.scope_id,
                    formation_id: input.formation_id,
                    kind: input.kind,
                    format_version: input.format_version,
                    source_digest: input.source_digest,
                    helm_values: input.helm_values,
                    embedded_in: input.embedded_in,
                    architecture_digest: input.architecture_digest,
                    binding_digest: input.binding_digest,
                    submitted_by,
                    submitted_at: timestamp(self.now()),
                },
            },
        );
        Ok(Outcome::Submitted {
            source_submitted: submitted,
        })
    }
}
