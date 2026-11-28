//! `weft.infrastructure.ProvisionPoolNode` (`PLAN.md` § Obligations,
//! `ess/1-infrastructure/infrastructure.yaml`), in declaration order: `expiry-not-future`,
//! `expiry-too-far` (more than 24 h from now), `unknown-pool`, `pool-deleted`, `pool-draining`,
//! `provisioned` otherwise: a `Provisioning` worker node in the pool's cloud and zone.
//!
//! E1, restored by hand: ESS
//! `story:the-rust-target-writes-a-one-time-response-outcome-as-an-obligation-that-carries-its-contract-instead-of-refusing-the-workspace`.
//! The specification marks the join token of `provisioned` a one-time response
//! (`one_time_response: [join_token]`), which `tools/synth-input.sh` removes because the Rust
//! target refuses it. Here the token is minted once, answered once, and kept only as its SHA-256
//! beside the node ([`crate::storage::Store::set_join_token_digest`]); the decider of
//! `JoinNode/token-invalid` compares against that digest. This piece is removed together with
//! the E1 transformation of `tools/synth-input.sh` when ESS ships the story.

use time::Duration;
use weft_types::behaviour::{NodePoolStorage, NodeStorage};
use weft_types::infrastructure::obligations::ProvisionPoolNodeBehavior;
use weft_types::infrastructure::{
    InvalidTokenExpiry, NodeData, NodePoolState, NodeRole, NodeSnapshot, NodeState,
    PoolNodeProvisioned, ProvisionPoolNode, ProvisionPoolNodeOutcome, ProvisionPoolNodeResponse,
    StateConflict, UnknownNodePool,
};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::NodeId;

use crate::context::{decider::join_token_digest, instant, mint};
use crate::Ports;

impl ProvisionPoolNodeBehavior for Ports {
    fn provision_pool_node(
        &mut self,
        input: ProvisionPoolNode,
    ) -> Result<ProvisionPoolNodeOutcome, UnmetObligation> {
        let now = self.now();
        // The decoder admits only RFC 3339; an instant it admits that this clock cannot read is
        // a decision that cannot be made, refused naming the command.
        let expires = instant(&input.join_token_expires_at).ok_or(UnmetObligation {
            capability: "command behaviour",
            source: "weft.infrastructure.ProvisionPoolNode",
        })?;
        if expires <= now {
            return Ok(ProvisionPoolNodeOutcome::ExpiryNotFuture {
                error: InvalidTokenExpiry,
            });
        }
        if expires > now + Duration::hours(24) {
            return Ok(ProvisionPoolNodeOutcome::ExpiryTooFar {
                error: InvalidTokenExpiry,
            });
        }
        let Some(pool) = NodePoolStorage::get(self, &input.node_pool_id) else {
            return Ok(ProvisionPoolNodeOutcome::UnknownPool {
                error: UnknownNodePool,
            });
        };
        match pool.state {
            NodePoolState::Deleted => {
                return Ok(ProvisionPoolNodeOutcome::PoolDeleted {
                    error: UnknownNodePool,
                })
            }
            NodePoolState::Draining => {
                return Ok(ProvisionPoolNodeOutcome::PoolDraining {
                    error: StateConflict,
                })
            }
            NodePoolState::Active => {}
        }
        let node_id = NodeId(Uuid(mint()));
        let join_token = format!("{}{}", mint().replace('-', ""), mint().replace('-', ""));
        self.store()
            .set_join_token_digest(&node_id, &join_token_digest(&join_token));
        NodeStorage::put(
            self,
            NodeSnapshot {
                state: NodeState::Provisioning,
                data: NodeData {
                    node_id: node_id.clone(),
                    cloud_id: pool.data.cloud_id.clone(),
                    zone: pool.data.zone.clone(),
                    node_pool_id: Some(input.node_pool_id.clone()),
                    role: NodeRole::Worker,
                    join_token_expires_at: input.join_token_expires_at.clone(),
                    join_token_used: false,
                    isolation_facts: None,
                    observed_at: None,
                },
            },
        );
        Ok(ProvisionPoolNodeOutcome::Provisioned {
            response: ProvisionPoolNodeResponse {
                node_id: node_id.clone(),
                join_token,
            },
            pool_node_provisioned: PoolNodeProvisioned {
                node_id,
                cloud_id: pool.data.cloud_id,
                zone: pool.data.zone,
                node_pool_id: input.node_pool_id,
                join_token_expires_at: input.join_token_expires_at,
            },
        })
    }
}
