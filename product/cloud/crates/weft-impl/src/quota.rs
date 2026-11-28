//! The `over-quota` branch of the outcome group `quota` (`ess/cloud/5-workloads/workloads.yaml`):
//! "the step would take the scope past its quota for a resource class", decided from the rows
//! of the same store.
//!
//! The blocks key a step by its scope; weft keeps its quotas per account. A scope's quota is
//! the one of the account that holds the scope (`weft.accounts.Account.scope_id`, recorded by
//! `RecordAccountScope`), with every quota above it. A scope no account holds has no quota and
//! may hold nothing (deny by default).
//!
//! - A quota (`weft.accounts.Quota`) bounds what its account and the accounts that drew from it
//!   hold together: its `limit`. The quotas above an account's are its `parent_quota_id` chain.
//! - What an account holds of a class is the sum of the claims of the workloads of its live
//!   instances (`Pending`, `Scheduled`, `Starting`, `Running`, `Paused`: the states
//!   `replica-live` counts as live). Each live instance counts against its account's quota for
//!   the class and every quota above it.
//! - An account with no quota for a class may hold none of it (`weft.accounts.Quota`: "an
//!   account with no quota row for a class ... Taken: no (default deny)").
//!
//! What each step adds: a step that makes an instance, what it claims against what the scope
//! holds ([`exceeded`]). `DefineWorkload` and `DefineService` make no instance; they declare
//! what their instances will take, and are refused only where that alone passes a limit
//! ([`exceeded_alone`]): one instance of the workload, and `replicas` instances of the service.
//! A definition is not refused because the scope's live instances hold its quota (a new
//! revision is defined while the old one runs). The blocks take `CreateInstance` out of the
//! group: its instance is live from `Pending` on, and `PlaceInstance` (weft's run-time
//! admission) places it only while its claims and what the scope holds besides it stay within
//! the quota ([`exceeded_besides`]).

use std::collections::HashMap;

use weft_types::accounts::QuotaData;
use weft_types::behaviour::{AccountStorage, InstanceStorage, QuotaStorage, WorkloadStorage};
use weft_types::shared::{AccountId, InstanceId, ScopeId};
use weft_types::workloads::{ClaimRequest, InstanceState};

use crate::Ports;

/// Whether an instance in `state` holds its claims.
pub fn live(state: InstanceState) -> bool {
    matches!(
        state,
        InstanceState::Pending
            | InstanceState::Scheduled
            | InstanceState::Starting
            | InstanceState::Running
            | InstanceState::Paused
    )
}

/// Whether adding `claims` to what `scope` holds would take its quota, or a quota above it,
/// past its limit for a class the claims name.
pub fn exceeded(ports: &Ports, scope: &ScopeId, claims: &[ClaimRequest]) -> bool {
    past(ports, scope, claims, Held::All)
}

/// Whether `claims` alone, with nothing held, would take the quota of `scope`, or a quota
/// above it, past its limit for a class the claims name.
pub fn exceeded_alone(ports: &Ports, scope: &ScopeId, claims: &[ClaimRequest]) -> bool {
    past(ports, scope, claims, Held::Nothing)
}

/// Whether `claims`, added to what `scope` holds besides the instance `own`, would take its
/// quota, or a quota above it, past its limit for a class the claims name: the claims of an
/// instance that already counts as held (`weft.scheduling.PlaceInstance` places an instance
/// `CreateInstance` admitted), against the quota the others leave.
pub fn exceeded_besides(
    ports: &Ports,
    scope: &ScopeId,
    claims: &[ClaimRequest],
    own: &InstanceId,
) -> bool {
    past(ports, scope, claims, Held::AllBut(own))
}

/// `claims` times `count`: what `count` instances that each claim `claims` take.
pub fn times(claims: &[ClaimRequest], count: i64) -> Vec<ClaimRequest> {
    claims
        .iter()
        .map(|claim| ClaimRequest {
            amount: claim.amount.saturating_mul(count.max(0)),
            ..claim.clone()
        })
        .collect()
}

/// What of the account's live instances [`past`] counts as held.
#[derive(Clone, Copy)]
enum Held<'a> {
    /// None: the claims alone.
    Nothing,
    /// Every live instance.
    All,
    /// Every live instance but this one.
    AllBut(&'a InstanceId),
}

/// Whether `claims`, added to what `held` counts of what `scope` holds, would take a quota of
/// its chain past its limit.
fn past(ports: &Ports, scope: &ScopeId, claims: &[ClaimRequest], held: Held<'_>) -> bool {
    // Every quota by its identity.
    let quotas: HashMap<String, QuotaData> = QuotaStorage::list(ports)
        .into_iter()
        .map(|quota| (quota.data.quota_id.0 .0.clone(), quota.data))
        .collect();
    // The account that holds each scope.
    let accounts: HashMap<String, AccountId> = AccountStorage::list(ports)
        .into_iter()
        .filter_map(|account| Some((account.data.scope_id?.0 .0, account.data.account_id)))
        .collect();
    // The identities of the quota of the account of `scope` for `class` and of every quota
    // above it.
    let chain = |scope: &ScopeId, class: &str| -> Option<Vec<String>> {
        let account = accounts.get(&scope.0 .0)?;
        let mut at = quotas
            .values()
            .find(|quota| &quota.account_id == account && quota.resource_class.0 == class)?;
        let mut chain = vec![at.quota_id.0 .0.clone()];
        while let Some(parent) = at
            .parent_quota_id
            .as_ref()
            .and_then(|parent| quotas.get(&parent.0 .0))
        {
            if chain.contains(&parent.quota_id.0 .0) {
                break;
            }
            chain.push(parent.quota_id.0 .0.clone());
            at = parent;
        }
        Some(chain)
    };
    let claims_of: HashMap<String, Vec<ClaimRequest>> = WorkloadStorage::list(ports)
        .into_iter()
        .map(|workload| (workload.data.workload_id.0 .0, workload.data.claims))
        .collect();
    let mut holding: HashMap<String, i64> = HashMap::new();
    for instance in InstanceStorage::list(ports) {
        let counted = match held {
            Held::Nothing => false,
            Held::All => true,
            Held::AllBut(own) => &instance.data.instance_id != own,
        };
        if !counted || !live(instance.state) {
            continue;
        }
        let Some(workload_claims) = claims_of.get(&instance.data.workload_id.0 .0) else {
            continue;
        };
        for claim in workload_claims {
            for quota in chain(&instance.data.scope_id, &claim.class.0)
                .into_iter()
                .flatten()
            {
                *holding.entry(quota).or_default() += claim.amount;
            }
        }
    }
    let mut adding: HashMap<&str, i64> = HashMap::new();
    for claim in claims {
        *adding.entry(claim.class.0.as_str()).or_default() += claim.amount;
    }
    adding
        .into_iter()
        .any(|(class, amount)| match chain(scope, class) {
            None => amount > 0,
            Some(chain) => chain.iter().any(|quota| {
                let limit = quotas.get(quota).map_or(0, |quota| quota.limit);
                holding.get(quota).copied().unwrap_or(0) + amount > limit
            }),
        })
}
