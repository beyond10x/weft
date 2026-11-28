//! `weft.architecture.CreateFormation` (`PLAN.md` § Obligations,
//! `ess/cloud/9-composition/architecture.yaml`), in declaration order:
//!
//! - `no-such-scope`, decided externally before any row ("the scope does not exist or is
//!   closed"), from the `weft.scope.Scope` row of the same store;
//! - `not-callers-scope` (a caller acting in no scope counts as another scope's);
//! - `name-taken` (a formation of the scope with the name, in any state);
//! - `created` otherwise: `Active`, its internal domain `<name>.internal`, no quota slice. The
//!   formation is in no cluster until `weft.cluster.PinFormation` records it in one.

use weft_types::architecture::obligations::CreateFormationBehavior;
use weft_types::architecture::{
    CreateFormation, CreateFormationOutcome, FormationCreated, FormationData, FormationNameTaken,
    FormationSnapshot, FormationState, NoSuchScope,
};
use weft_types::behaviour::{Context, FormationStorage, ScopeStorage};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::scope::ScopeState;
use weft_types::shared::FormationId;

use crate::context::mint;
use crate::Ports;

impl CreateFormationBehavior for Ports {
    fn create_formation(
        &mut self,
        input: CreateFormation,
    ) -> Result<CreateFormationOutcome, UnmetObligation> {
        if ScopeStorage::get(self, &input.scope_id)
            .is_none_or(|scope| scope.state == ScopeState::Closed)
        {
            return Ok(CreateFormationOutcome::NoSuchScope { error: NoSuchScope });
        }
        if Context::caller_scope_id(self).as_ref() != Some(&input.scope_id) {
            return Ok(CreateFormationOutcome::NotCallersScope { error: NoSuchScope });
        }
        if FormationStorage::list(self)
            .iter()
            .any(|row| row.data.scope_id == input.scope_id && row.data.name == input.name)
        {
            return Ok(CreateFormationOutcome::NameTaken {
                error: FormationNameTaken,
            });
        }
        let formation_id = FormationId(Uuid(mint()));
        FormationStorage::put(
            self,
            FormationSnapshot {
                state: FormationState::Active,
                data: FormationData {
                    formation_id: formation_id.clone(),
                    scope_id: input.scope_id.clone(),
                    name: input.name.clone(),
                    internal_domain: format!("{}.internal", input.name.0),
                    quota_slice: Vec::new(),
                    zones: input.zones,
                },
            },
        );
        Ok(CreateFormationOutcome::Created {
            formation_created: FormationCreated {
                formation_id,
                scope_id: input.scope_id,
                name: input.name,
            },
        })
    }
}
