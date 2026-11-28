//! `weft.architecture.CreateEnvironment` (`PLAN.md` § Obligations,
//! `ess/cloud/9-composition/architecture.yaml`), in declaration order: `not-callers-scope` (a caller
//! acting in no scope counts as another scope's), `no-such-formation`,
//! `formation-of-other-scope`, `retired`, `name-taken` (an environment of the formation with
//! the name, in any state), `created` otherwise: `Active`, made by hand (`Manual`), in the
//! formation's scope, with no quota slice and no preview source.

use weft_types::architecture::obligations::CreateEnvironmentBehavior;
use weft_types::architecture::{
    CreateEnvironment, CreateEnvironmentOutcome, EnvironmentCreated, EnvironmentData,
    EnvironmentNameTaken, EnvironmentOrigin, EnvironmentSnapshot, EnvironmentState, FormationState,
    FormationStateConflict, NoSuchFormation,
};
use weft_types::behaviour::{Context, EnvironmentStorage, FormationStorage};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::EnvironmentId;

use crate::context::mint;
use crate::Ports;

impl CreateEnvironmentBehavior for Ports {
    fn create_environment(
        &mut self,
        input: CreateEnvironment,
    ) -> Result<CreateEnvironmentOutcome, UnmetObligation> {
        if Context::caller_scope_id(self).as_ref() != Some(&input.scope_id) {
            return Ok(CreateEnvironmentOutcome::NotCallersScope {
                error: NoSuchFormation,
            });
        }
        let Some(formation) = FormationStorage::get(self, &input.formation_id) else {
            return Ok(CreateEnvironmentOutcome::NoSuchFormation {
                error: NoSuchFormation,
            });
        };
        if formation.data.scope_id != input.scope_id {
            return Ok(CreateEnvironmentOutcome::FormationOfOtherScope {
                error: NoSuchFormation,
            });
        }
        if formation.state == FormationState::Retired {
            return Ok(CreateEnvironmentOutcome::Retired {
                error: FormationStateConflict,
            });
        }
        if EnvironmentStorage::list(self)
            .iter()
            .any(|row| row.data.formation_id == input.formation_id && row.data.name == input.name)
        {
            return Ok(CreateEnvironmentOutcome::NameTaken {
                error: EnvironmentNameTaken,
            });
        }
        let environment_id = EnvironmentId(Uuid(mint()));
        EnvironmentStorage::put(
            self,
            EnvironmentSnapshot {
                state: EnvironmentState::Active,
                data: EnvironmentData {
                    environment_id: environment_id.clone(),
                    formation_id: input.formation_id.clone(),
                    scope_id: formation.data.scope_id,
                    name: input.name.clone(),
                    quota_slice: Vec::new(),
                    origin: EnvironmentOrigin::Manual,
                    protected: input.protected,
                    source_repository: None,
                    pull_request: None,
                    expires_at: None,
                },
            },
        );
        Ok(CreateEnvironmentOutcome::Created {
            environment_created: EnvironmentCreated {
                environment_id,
                formation_id: input.formation_id,
                name: input.name,
            },
        })
    }
}
