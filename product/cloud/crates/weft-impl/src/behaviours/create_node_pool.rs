//! `weft.infrastructure.CreateNodePool` (`PLAN.md` § Obligations,
//! `ess/1-infrastructure/infrastructure.yaml`), in declaration order: `negative-size`,
//! `min-above-max`, `unknown-zone`, `zone-retired`, `already-requested` (a pool made for the
//! same `requested_for`), `created` otherwise, in the zone's cloud with its minimum as desired
//! size.

use weft_types::behaviour::{NodePoolStorage, ZoneStorage};
use weft_types::infrastructure::obligations::CreateNodePoolBehavior;
use weft_types::infrastructure::{
    CreateNodePool, CreateNodePoolOutcome, InvalidPoolSize, NodePoolCreated, NodePoolData,
    NodePoolSnapshot, NodePoolState, PoolAlreadyRequested, UnknownZone, ZoneState,
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
        if input.requested_for.is_some()
            && NodePoolStorage::list(self)
                .iter()
                .any(|row| row.data.requested_for == input.requested_for)
        {
            return Ok(CreateNodePoolOutcome::AlreadyRequested {
                error: PoolAlreadyRequested,
            });
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
                    requested_for: input.requested_for.clone(),
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
                requested_for: input.requested_for,
            },
        })
    }
}
