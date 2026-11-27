//! `weft.formations.CreateFormation` (`PLAN.md` § Obligations, `ess/4-applications/formations.yaml`),
//! in declaration order:
//!
//! - `no-such-account`, decided externally before any row ("the account does not exist or is
//!   closed"), from the `weft.accounts.Account` row of the same store;
//! - `not-callers-account` (a caller acting in no account counts as another account's);
//! - `no-such-cluster`, `cluster-deleted`, `cluster-of-other-account` (a cluster an account holds
//!   that is not this one; the default cluster, held by no account, takes every account);
//! - `name-taken` (a formation of the account with the name, in any state);
//! - `created` otherwise: `Active`, its internal domain `<name>.internal`, no quota slice.

use weft_types::accounts::AccountState;
use weft_types::behaviour::{AccountStorage, ClusterStorage, Context, FormationStorage};
use weft_types::cluster::ClusterState;
use weft_types::formations::obligations::CreateFormationBehavior;
use weft_types::formations::{
    CreateFormation, CreateFormationOutcome, FormationCreated, FormationData, FormationNameTaken, FormationSnapshot,
    FormationState, NoSuchAccount, NoSuchCluster,
};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::FormationId;

use crate::context::mint;
use crate::Ports;

impl CreateFormationBehavior for Ports {
    fn create_formation(&mut self, input: CreateFormation) -> Result<CreateFormationOutcome, UnmetObligation> {
        if AccountStorage::get(self, &input.account_id).is_none_or(|account| account.state == AccountState::Closed) {
            return Ok(CreateFormationOutcome::NoSuchAccount { error: NoSuchAccount });
        }
        if Context::caller_account_id(self).as_ref() != Some(&input.account_id) {
            return Ok(CreateFormationOutcome::NotCallersAccount { error: NoSuchAccount });
        }
        let Some(cluster) = ClusterStorage::get(self, &input.cluster_id) else {
            return Ok(CreateFormationOutcome::NoSuchCluster { error: NoSuchCluster });
        };
        if cluster.state == ClusterState::Deleted {
            return Ok(CreateFormationOutcome::ClusterDeleted { error: NoSuchCluster });
        }
        if cluster.data.account_id.as_ref().is_some_and(|holder| holder != &input.account_id) {
            return Ok(CreateFormationOutcome::ClusterOfOtherAccount { error: NoSuchCluster });
        }
        if FormationStorage::list(self)
            .iter()
            .any(|row| row.data.account_id == input.account_id && row.data.name == input.name)
        {
            return Ok(CreateFormationOutcome::NameTaken { error: FormationNameTaken });
        }
        let formation_id = FormationId(Uuid(mint()));
        FormationStorage::put(
            self,
            FormationSnapshot {
                state: FormationState::Active,
                data: FormationData {
                    formation_id: formation_id.clone(),
                    account_id: input.account_id.clone(),
                    name: input.name.clone(),
                    internal_domain: format!("{}.internal", input.name.0),
                    quota_slice: Vec::new(),
                    zones: input.zones,
                    cluster_id: input.cluster_id,
                },
            },
        );
        Ok(CreateFormationOutcome::Created {
            formation_created: FormationCreated {
                formation_id,
                account_id: input.account_id,
                name: input.name,
            },
        })
    }
}
