//! The view `weft.workloads.NodeInstanceRuns` (`ess/4-applications/workloads.yaml`,
//! `story:slot-node-process`): every instance on the node, with its workload's package kind, host
//! user, prestart steps, stop and restart, read through the instance's `workload_id` (one hop).
//! An instance whose workload is not held has those fields absent.

use weft_types::behaviour::{InstanceStorage, WorkloadStorage};
use weft_types::obligation::UnmetObligation;
use weft_types::shared::NodeId;
use weft_types::workloads::obligations::NodeInstanceRunsQuery;
use weft_types::workloads::NodeInstanceRuns;

use crate::Ports;

impl NodeInstanceRunsQuery for Ports {
    fn node_instance_runs(
        &self,
        node_id: NodeId,
    ) -> Result<Vec<NodeInstanceRuns>, UnmetObligation> {
        Ok(InstanceStorage::list(self)
            .into_iter()
            .filter(|instance| instance.data.node_id.as_ref() == Some(&node_id))
            .map(|instance| {
                let workload =
                    WorkloadStorage::get(self, &instance.data.workload_id).map(|row| row.data);
                NodeInstanceRuns {
                    instance_id: instance.data.instance_id,
                    workload_id: instance.data.workload_id,
                    package_kind: workload.as_ref().map(|workload| workload.package_kind),
                    run_as: workload
                        .as_ref()
                        .and_then(|workload| workload.run_as.clone()),
                    prestart: workload
                        .as_ref()
                        .and_then(|workload| workload.prestart.clone()),
                    stop: workload.as_ref().and_then(|workload| workload.stop.clone()),
                    restart: workload.and_then(|workload| workload.restart),
                    state: instance.state,
                }
            })
            .collect())
    }
}
