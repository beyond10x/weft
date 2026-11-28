//! `weft.accounts.CreateAccount` (`PLAN.md` § Obligations, `ess/2-tenancy/accounts.yaml`), in
//! declaration order: `not-callers-tenant` (a caller without a tenant counts as another tenant's:
//! deny by default), `no-such-tenant`, `tenant-closed`, `name-taken` (an open top-level account
//! of the tenant with the name), `created` otherwise.

use weft_types::accounts::obligations::CreateAccountBehavior;
use weft_types::accounts::{
    AccountCreated, AccountData, AccountNameTaken, AccountSnapshot, AccountState, CreateAccount,
    CreateAccountOutcome, TenantNotActive, TenantNotFound, TenantState,
};
use weft_types::behaviour::{AccountStorage, Context, TenantStorage};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::AccountId;

use crate::context::mint;
use crate::Ports;

impl CreateAccountBehavior for Ports {
    fn create_account(
        &mut self,
        input: CreateAccount,
    ) -> Result<CreateAccountOutcome, UnmetObligation> {
        if Context::caller_tenant_id(self).as_ref() != Some(&input.tenant_id) {
            return Ok(CreateAccountOutcome::NotCallersTenant {
                error: TenantNotFound,
            });
        }
        let Some(tenant) = TenantStorage::get(self, &input.tenant_id) else {
            return Ok(CreateAccountOutcome::NoSuchTenant {
                error: TenantNotFound,
            });
        };
        if tenant.state == TenantState::Closed {
            return Ok(CreateAccountOutcome::TenantClosed {
                error: TenantNotActive,
            });
        }
        if AccountStorage::list(self).iter().any(|row| {
            row.data.tenant_id == input.tenant_id
                && row.data.parent_account_id.is_none()
                && row.data.name == input.name
                && row.state == AccountState::Open
        }) {
            return Ok(CreateAccountOutcome::NameTaken {
                error: AccountNameTaken,
            });
        }
        let account_id = AccountId(Uuid(mint()));
        AccountStorage::put(
            self,
            AccountSnapshot {
                state: AccountState::Open,
                data: AccountData {
                    account_id: account_id.clone(),
                    tenant_id: input.tenant_id.clone(),
                    parent_account_id: None,
                    scope_id: None,
                    name: input.name.clone(),
                },
            },
        );
        Ok(CreateAccountOutcome::Created {
            account_created: AccountCreated {
                account_id,
                tenant_id: input.tenant_id,
                name: input.name,
            },
        })
    }
}
