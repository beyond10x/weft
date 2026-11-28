//! `weft.scope.OpenScope` (`PLAN.md` § Obligations, `ess/cloud/3-scope/scope.yaml`), in
//! declaration order: `name-taken` (an `Open` scope of the name; a closed scope frees its name),
//! `opened` otherwise, answering the new scope's identity.
//!
//! The binding `account-opens-scope` invokes it for every `weft.accounts.AccountCreated`, the
//! scope named by the account's identity; `weft.accounts.RecordAccountScope` records the
//! identity answered here on the account.

use weft_types::behaviour::ScopeStorage;
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::scope::obligations::OpenScopeBehavior;
use weft_types::scope::{
    OpenScope, OpenScopeOutcome, OpenScopeResponse, ScopeData, ScopeNameTaken, ScopeOpened,
    ScopeSnapshot, ScopeState,
};
use weft_types::shared::ScopeId;

use crate::context::mint;
use crate::Ports;

impl OpenScopeBehavior for Ports {
    fn open_scope(&mut self, input: OpenScope) -> Result<OpenScopeOutcome, UnmetObligation> {
        if ScopeStorage::list(self)
            .iter()
            .any(|row| row.data.name == input.name && row.state == ScopeState::Open)
        {
            return Ok(OpenScopeOutcome::NameTaken {
                error: ScopeNameTaken,
            });
        }
        let scope_id = ScopeId(Uuid(mint()));
        ScopeStorage::put(
            self,
            ScopeSnapshot {
                state: ScopeState::Open,
                data: ScopeData {
                    scope_id: scope_id.clone(),
                    name: input.name.clone(),
                },
            },
        );
        Ok(OpenScopeOutcome::Opened {
            response: OpenScopeResponse {
                scope_id: scope_id.clone(),
            },
            scope_opened: ScopeOpened {
                scope_id,
                name: input.name,
            },
        })
    }
}
