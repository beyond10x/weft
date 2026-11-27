//! `weft.accounts.CreateTenant` (`PLAN.md` § Obligations, `ess/2-tenancy/accounts.yaml`):
//! `name-taken` when a tenant of that name exists; `created` otherwise.

use weft_types::accounts::obligations::CreateTenantBehavior;
use weft_types::accounts::{
    CreateTenant, CreateTenantOutcome, TenantCreated, TenantData, TenantNameTaken, TenantSnapshot, TenantState,
};
use weft_types::behaviour::TenantStorage;
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::TenantId;

use crate::context::mint;
use crate::Ports;

impl CreateTenantBehavior for Ports {
    fn create_tenant(&mut self, input: CreateTenant) -> Result<CreateTenantOutcome, UnmetObligation> {
        if TenantStorage::list(self)
            .iter()
            .any(|row| row.data.name == input.name)
        {
            return Ok(CreateTenantOutcome::NameTaken {
                error: TenantNameTaken,
            });
        }
        let tenant_id = TenantId(Uuid(mint()));
        TenantStorage::put(
            self,
            TenantSnapshot {
                state: TenantState::Active,
                data: TenantData {
                    tenant_id: tenant_id.clone(),
                    name: input.name.clone(),
                },
            },
        );
        Ok(CreateTenantOutcome::Created {
            tenant_created: TenantCreated {
                tenant_id,
                name: input.name,
            },
        })
    }
}
