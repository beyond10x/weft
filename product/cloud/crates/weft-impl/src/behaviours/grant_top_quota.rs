//! `weft.accounts.GrantTopQuota` (`PLAN.md` § Obligations, `ess/2-tenancy/accounts.yaml`), in
//! declaration order: `negative-limit`, `no-such-account`, `sub-account`, `account-closed`,
//! `quota-exists` (a quota of the account for the resource class), `granted` otherwise, the quota
//! taking the account's tenant and its whole limit as remaining.

use weft_types::accounts::obligations::GrantTopQuotaBehavior;
use weft_types::accounts::{
    AccountNotFound, AccountNotOpen, AccountState, GrantTopQuota, GrantTopQuotaOutcome,
    InvalidQuotaAmount, NotTopLevelAccount, QuotaData, QuotaExists, QuotaGranted, QuotaSnapshot,
    QuotaState,
};
use weft_types::behaviour::{AccountStorage, QuotaStorage};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::QuotaId;

use crate::context::mint;
use crate::Ports;

impl GrantTopQuotaBehavior for Ports {
    fn grant_top_quota(
        &mut self,
        input: GrantTopQuota,
    ) -> Result<GrantTopQuotaOutcome, UnmetObligation> {
        if input.limit < 0 {
            return Ok(GrantTopQuotaOutcome::NegativeLimit {
                error: InvalidQuotaAmount,
            });
        }
        let Some(account) = AccountStorage::get(self, &input.account_id) else {
            return Ok(GrantTopQuotaOutcome::NoSuchAccount {
                error: AccountNotFound,
            });
        };
        if account.data.parent_account_id.is_some() {
            return Ok(GrantTopQuotaOutcome::SubAccount {
                error: NotTopLevelAccount,
            });
        }
        if account.state == AccountState::Closed {
            return Ok(GrantTopQuotaOutcome::AccountClosed {
                error: AccountNotOpen,
            });
        }
        if QuotaStorage::list(self).iter().any(|row| {
            row.data.account_id == input.account_id
                && row.data.resource_class == input.resource_class
        }) {
            return Ok(GrantTopQuotaOutcome::QuotaExists { error: QuotaExists });
        }
        let quota_id = QuotaId(Uuid(mint()));
        QuotaStorage::put(
            self,
            QuotaSnapshot {
                state: QuotaState::Set,
                data: QuotaData {
                    quota_id: quota_id.clone(),
                    account_id: input.account_id.clone(),
                    tenant_id: account.data.tenant_id,
                    parent_quota_id: None,
                    resource_class: input.resource_class.clone(),
                    limit: input.limit,
                    remaining: input.limit,
                },
            },
        );
        Ok(GrantTopQuotaOutcome::Granted {
            quota_granted: QuotaGranted {
                quota_id,
                account_id: input.account_id,
                resource_class: input.resource_class,
                limit: input.limit,
            },
        })
    }
}
