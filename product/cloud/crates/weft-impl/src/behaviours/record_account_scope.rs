//! `weft.accounts.RecordAccountScope` (`PLAN.md` § Obligations, `ess/2-tenancy/accounts.yaml`),
//! in declaration order: `no-such-account`, `no-such-scope` (no `weft.scope.Scope` of the
//! identity, in any state), `scope-taken` (the account holds a scope already, or another
//! account, or this one, holds this scope: one account, one scope, both ways), `recorded`
//! otherwise: the scope is set on the account.

use weft_types::accounts::obligations::RecordAccountScopeBehavior;
use weft_types::accounts::{
    AccountNotFound, AccountScopeRecorded, RecordAccountScope, RecordAccountScopeOutcome,
    ScopeNotFound, ScopeTaken,
};
use weft_types::behaviour::{AccountStorage, ScopeStorage};
use weft_types::obligation::UnmetObligation;

use crate::Ports;

impl RecordAccountScopeBehavior for Ports {
    fn record_account_scope(
        &mut self,
        input: RecordAccountScope,
    ) -> Result<RecordAccountScopeOutcome, UnmetObligation> {
        let Some(mut account) = AccountStorage::get(self, &input.account_id) else {
            return Ok(RecordAccountScopeOutcome::NoSuchAccount {
                error: AccountNotFound,
            });
        };
        if ScopeStorage::get(self, &input.scope_id).is_none() {
            return Ok(RecordAccountScopeOutcome::NoSuchScope {
                error: ScopeNotFound,
            });
        }
        if AccountStorage::list(self).iter().any(|row| {
            (row.data.account_id == input.account_id && row.data.scope_id.is_some())
                || row.data.scope_id.as_ref() == Some(&input.scope_id)
        }) {
            return Ok(RecordAccountScopeOutcome::ScopeTaken { error: ScopeTaken });
        }
        account.data.scope_id = Some(input.scope_id.clone());
        AccountStorage::put(self, account);
        Ok(RecordAccountScopeOutcome::Recorded {
            account_scope_recorded: AccountScopeRecorded {
                account_id: input.account_id,
                scope_id: input.scope_id,
            },
        })
    }
}
