//! `weft.infrastructure.CreateNodePool` (`PLAN.md` § Obligations,
//! `ess/cloud/2-infrastructure/infrastructure.yaml`), in declaration order: `negative-size`,
//! `min-above-max`, `unknown-zone`, `zone-retired`, `created` otherwise, in the zone's cloud
//! with its minimum as desired size. The blocks keep no `requested_for`: a second delivery of
//! one `ClusterNodePoolRequested` makes a second pool (OPEN at the binding
//! `cluster-create-node-pool`).

use weft_types::behaviour::{NodePoolStorage, ZoneStorage};
use weft_types::infrastructure::obligations::CreateNodePoolBehavior;
use weft_types::infrastructure::{
    CreateNodePool, CreateNodePoolOutcome, InvalidPoolSize, NodePoolCreated, NodePoolData,
    NodePoolSnapshot, NodePoolState, UnknownZone, ZoneState,
};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::NodePoolId;

use crate::context::mint;
use crate::Ports;

impl CreateNodePoolBehavior for Ports {
    fn create_node_pool(
        &mut self,
        input: CreateNodePool,
    ) -> Result<CreateNodePoolOutcome, UnmetObligation> {
        if input.min_size < 0 {
            return Ok(CreateNodePoolOutcome::NegativeSize {
                error: InvalidPoolSize,
            });
        }
        if input.min_size > input.max_size {
            return Ok(CreateNodePoolOutcome::MinAboveMax {
                error: InvalidPoolSize,
            });
        }
        let Some(zone) = ZoneStorage::get(self, &input.zone) else {
            return Ok(CreateNodePoolOutcome::UnknownZone { error: UnknownZone });
        };
        if zone.state == ZoneState::Retired {
            return Ok(CreateNodePoolOutcome::ZoneRetired { error: UnknownZone });
        }
        let node_pool_id = NodePoolId(Uuid(mint()));
        let cloud_id = zone.data.cloud_id;
        NodePoolStorage::put(
            self,
            NodePoolSnapshot {
                state: NodePoolState::Active,
                data: NodePoolData {
                    node_pool_id: node_pool_id.clone(),
                    cloud_id: cloud_id.clone(),
                    name: input.name.clone(),
                    zone: input.zone.clone(),
                    machine_type: input.machine_type.clone(),
                    image: input.image.clone(),
                    min_size: input.min_size,
                    max_size: input.max_size,
                    desired_size: input.min_size,
                },
            },
        );
        Ok(CreateNodePoolOutcome::Created {
            node_pool_created: NodePoolCreated {
                node_pool_id,
                cloud_id,
                name: input.name,
                zone: input.zone,
                machine_type: input.machine_type,
                image: input.image,
                min_size: input.min_size,
                max_size: input.max_size,
            },
        })
    }
}
