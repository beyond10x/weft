//! `weft.formations.AddComponent` (`PLAN.md` § Obligations, `ess/4-applications/formations.yaml`), in
//! declaration order: `not-callers-account` (a caller acting in no account counts as another
//! account's), `no-such-formation`, `formation-of-other-account`, `retired`, `name-taken` (a
//! component of the formation with the name), `added` otherwise: not on the border, its internal
//! name `<name>.<formation's internal domain>` (`orders.shop.internal`).

use weft_types::behaviour::{ComponentStorage, Context, FormationStorage};
use weft_types::formations::obligations::AddComponentBehavior;
use weft_types::formations::{
    AddComponent, AddComponentOutcome, ComponentAdded, ComponentData, ComponentNameTaken, ComponentSnapshot,
    ComponentState, FormationState, FormationStateConflict, InternalName, NoSuchFormation,
};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::ComponentId;

use crate::context::mint;
use crate::Ports;

impl AddComponentBehavior for Ports {
    fn add_component(&mut self, input: AddComponent) -> Result<AddComponentOutcome, UnmetObligation> {
        if Context::caller_account_id(self).as_ref() != Some(&input.account_id) {
            return Ok(AddComponentOutcome::NotCallersAccount { error: NoSuchFormation });
        }
        let Some(formation) = FormationStorage::get(self, &input.formation_id) else {
            return Ok(AddComponentOutcome::NoSuchFormation { error: NoSuchFormation });
        };
        if formation.data.account_id != input.account_id {
            return Ok(AddComponentOutcome::FormationOfOtherAccount { error: NoSuchFormation });
        }
        if formation.state == FormationState::Retired {
            return Ok(AddComponentOutcome::Retired { error: FormationStateConflict });
        }
        if ComponentStorage::list(self)
            .iter()
            .any(|row| row.data.formation_id == input.formation_id && row.data.name == input.name)
        {
            return Ok(AddComponentOutcome::NameTaken { error: ComponentNameTaken });
        }
        let component_id = ComponentId(Uuid(mint()));
        let internal_name = InternalName(format!("{}.{}", input.name.0, formation.data.internal_domain));
        ComponentStorage::put(
            self,
            ComponentSnapshot {
                state: ComponentState::Declared,
                data: ComponentData {
                    component_id: component_id.clone(),
                    formation_id: input.formation_id.clone(),
                    name: input.name.clone(),
                    internal_name: internal_name.clone(),
                    on_border: false,
                },
            },
        );
        Ok(AddComponentOutcome::Added {
            component_added: ComponentAdded {
                component_id,
                formation_id: input.formation_id,
                name: input.name,
                internal_name,
            },
        })
    }
}
