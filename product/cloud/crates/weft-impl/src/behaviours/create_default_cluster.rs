//! `weft.cluster.CreateDefaultCluster` (`PLAN.md` § Obligations, `ess/3-resources/cluster.yaml`),
//! in declaration order: `no-zones`, `unknown-cloud`, `zone-unusable` (a named zone retired or of
//! another cloud; a zone that does not exist is not refused here), `default-exists` (a cluster of
//! the cloud without an account that is not deleted), `created` otherwise, with a shared control
//! plane.

use weft_types::behaviour::{CloudStorage, ClusterStorage, ZoneStorage};
use weft_types::cluster::obligations::CreateDefaultClusterBehavior;
use weft_types::cluster::{
    ClusterData, ClusterSnapshot, ClusterState, ControlPlaneMode, CreateDefaultCluster, CreateDefaultClusterOutcome,
    DefaultClusterCreated, DefaultClusterExists, NoZones, UnknownCloud, UnknownZone,
};
use weft_types::infrastructure::ZoneState;
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::ClusterId;

use crate::context::mint;
use crate::Ports;

impl CreateDefaultClusterBehavior for Ports {
    fn create_default_cluster(
        &mut self,
        input: CreateDefaultCluster,
    ) -> Result<CreateDefaultClusterOutcome, UnmetObligation> {
        if input.zones.is_empty() {
            return Ok(CreateDefaultClusterOutcome::NoZones { error: NoZones });
        }
        if CloudStorage::get(self, &input.cloud_id).is_none() {
            return Ok(CreateDefaultClusterOutcome::UnknownCloud { error: UnknownCloud });
        }
        if ZoneStorage::list(self).iter().any(|row| {
            input.zones.contains(&row.data.zone)
                && (row.state == ZoneState::Retired || row.data.cloud_id != input.cloud_id)
        }) {
            return Ok(CreateDefaultClusterOutcome::ZoneUnusable { error: UnknownZone });
        }
        if ClusterStorage::list(self).iter().any(|row| {
            row.data.cloud_id == input.cloud_id && row.data.account_id.is_none() && row.state != ClusterState::Deleted
        }) {
            return Ok(CreateDefaultClusterOutcome::DefaultExists { error: DefaultClusterExists });
        }
        let cluster_id = ClusterId(Uuid(mint()));
        ClusterStorage::put(
            self,
            ClusterSnapshot {
                state: ClusterState::Active,
                data: ClusterData {
                    cluster_id: cluster_id.clone(),
                    cloud_id: input.cloud_id.clone(),
                    account_id: None,
                    name: input.name.clone(),
                    control_plane: ControlPlaneMode::Shared,
                    min_isolation: input.min_isolation,
                    zones: input.zones.clone(),
                },
            },
        );
        Ok(CreateDefaultClusterOutcome::Created {
            default_cluster_created: DefaultClusterCreated {
                cluster_id,
                cloud_id: input.cloud_id,
                name: input.name,
                min_isolation: input.min_isolation,
                zones: input.zones,
            },
        })
    }
}
