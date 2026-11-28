//! The binding transformations the generated system owes (`weft_system::obligations`), written
//! by hand: each maps its event to the command input as the binding's `mapping:` in
//! `ess/components.yaml` declares, and fills the one member that crosses a declared conversion
//! ([`crate::conversions`]).
//!
//! - `account-opens-scope` and `sub-account-opens-scope`: the scope an account opens is named by
//!   the account's identity (`weft.shared.AccountId -> String`).
//! - `cluster-create-node-pool`: the infrastructure pool made for a cluster pool is named by the
//!   cluster pool's identity (`weft.shared.ClusterNodePoolId -> String`); the zone, machine
//!   type, image and sizes are the event's.

use weft_system::obligations::{
    AccountOpensScopeTransformation, ClusterCreateNodePoolTransformation,
    SubAccountOpensScopeTransformation,
};
use weft_types::accounts::{AccountCreated, SubAccountCreated};
use weft_types::cluster::ClusterNodePoolRequested;
use weft_types::infrastructure::CreateNodePool;
use weft_types::obligation::{
    UnmetObligation, WeftSharedAccountIdToStringConversion,
    WeftSharedClusterNodePoolIdToStringConversion,
};
use weft_types::scope::OpenScope;

use crate::conversions::Conversions;

/// The binding transformations weft implements.
#[derive(Debug, Clone, Copy, Default)]
pub struct Transformations;

impl AccountOpensScopeTransformation for Transformations {
    fn account_opens_scope_input(
        &self,
        event: &AccountCreated,
    ) -> Result<OpenScope, UnmetObligation> {
        Ok(OpenScope {
            name: WeftSharedAccountIdToStringConversion::convert(
                &Conversions,
                event.account_id.clone(),
            )?,
        })
    }
}

impl SubAccountOpensScopeTransformation for Transformations {
    fn sub_account_opens_scope_input(
        &self,
        event: &SubAccountCreated,
    ) -> Result<OpenScope, UnmetObligation> {
        Ok(OpenScope {
            name: WeftSharedAccountIdToStringConversion::convert(
                &Conversions,
                event.account_id.clone(),
            )?,
        })
    }
}

impl ClusterCreateNodePoolTransformation for Transformations {
    fn cluster_create_node_pool_input(
        &self,
        event: &ClusterNodePoolRequested,
    ) -> Result<CreateNodePool, UnmetObligation> {
        Ok(CreateNodePool {
            name: WeftSharedClusterNodePoolIdToStringConversion::convert(
                &Conversions,
                event.cluster_node_pool_id.clone(),
            )?,
            zone: event.zone.clone(),
            machine_type: event.machine_type.clone(),
            image: event.image.clone(),
            min_size: event.min_size,
            max_size: event.max_size,
        })
    }
}
