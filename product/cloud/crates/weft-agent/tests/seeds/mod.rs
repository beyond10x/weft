//! The seeded state of `follow` (`story:skeleton-node-run-assignment`), written through the
//! storage port before the control host starts: this node, `Ready` with an open session and its
//! join token used; a second node, `Ready` with an open session; and for each node one instance
//! in `Scheduled` on that node with an `Active` assignment, its workload and the artifact of its
//! snippet. The process of this node's instance is `sh -c "echo weft"`, the other node's
//! `sh -c "echo other"`, so an exec of the other instance shows in the report by its output.
//! Neither process carries a `Ready` check.

use std::collections::BTreeMap;

use time::OffsetDateTime;
use weft_impl::context::{mint, timestamp};
use weft_impl::Row;
use weft_types::artifacts::{ArtifactData, ArtifactKind, ArtifactSnapshot, ArtifactState};
use weft_types::infrastructure::{NodeData, NodeRole, NodeSnapshot, NodeState};
use weft_types::nodes::{
    AssignmentData, AssignmentSnapshot, AssignmentState, DesiredState, NodeSessionData,
    NodeSessionSnapshot, NodeSessionState,
};
use weft_types::primitives::Uuid;
use weft_types::shared::{
    ArtifactDigest, AssignmentId, CloudId, ComponentName, EnvironmentId, FormationId,
    FormationRevisionId, InstanceId, IsolationFacts, IsolationLevel, NodeId, NodeSessionId,
    PlacementDecisionId, PriorityClassName, ProcessName, ResourceAmount, ResourceClassName,
    RuntimeId, ScopeId, ServiceId, WorkloadId, ZoneName,
};
use weft_types::workloads::{
    DesiredRun, InstanceData, InstanceHealth, InstanceSnapshot, InstanceState, PackageKind,
    Process, ProcessRole, RestartMode, RestartPolicy, ServiceSettings, ShapeKind, ShapeSettings,
    WorkloadData, WorkloadSnapshot, WorkloadState,
};

/// The snippet of this node's instance.
pub const THIS_SNIPPET: &str = "echo weft";

/// The snippet of the other node's instance.
pub const OTHER_SNIPPET: &str = "echo other";

/// The digest of `echo weft` (`printf %s 'echo weft' | sha256sum`).
const THIS_DIGEST: &str = "sha256:9ca076ac859babf0584d4e08e6314b91ce8ade279d574b9e6c0e30c09fbac700";

/// The digest of `echo other` (`printf %s 'echo other' | sha256sum`).
const OTHER_DIGEST: &str =
    "sha256:bf0ae755fc13dbecf2476616ab3fae4b7bd7660bf433332c1c15ab99af8d3383";

/// One node of the seed and what is assigned to it.
#[derive(Debug, Clone)]
pub struct Placed {
    pub node_id: String,
    pub instance_id: String,
}

/// The seeded state: its rows, this node's join token, and the identities a test reads.
#[derive(Debug, Clone)]
pub struct Seed {
    pub rows: Vec<Row>,
    /// The join token of this node; the state file keeps its digest beside the node.
    pub join_token: String,
    /// This node: the one the agent runs.
    pub this: Placed,
    /// The second node.
    pub other: Placed,
}

/// Two `Ready` nodes, each with an open session and one `Scheduled` instance assigned to it.
pub fn two_nodes() -> Seed {
    let cloud_id = CloudId(Uuid(mint()));
    let scope_id = ScopeId(Uuid(mint()));
    let mut rows = Vec::new();
    let this = placed(&mut rows, &cloud_id, &scope_id, THIS_SNIPPET, THIS_DIGEST);
    let other = placed(&mut rows, &cloud_id, &scope_id, OTHER_SNIPPET, OTHER_DIGEST);
    Seed {
        rows,
        join_token: mint(),
        this,
        other,
    }
}

/// The seed of [`two_nodes`] with this node's instance row changed by `change`.
pub fn two_nodes_changed(change: impl FnOnce(&mut InstanceSnapshot)) -> Seed {
    let mut seed = two_nodes();
    let instance = seed
        .rows
        .iter_mut()
        .find_map(|row| match row {
            Row::Instance(instance) if instance.data.instance_id.0 .0 == seed.this.instance_id => {
                Some(instance)
            }
            _ => None,
        })
        .expect("this node's instance is seeded");
    change(instance);
    seed
}

/// A `Main` process running `sh -c <snippet>`, named `name`.
pub fn process(name: &str, snippet: &str) -> Process {
    Process {
        name: ProcessName(name.to_owned()),
        role: ProcessRole::Main,
        artifact_digest: None,
        command: vec!["sh".to_owned(), "-c".to_owned(), snippet.to_owned()],
        environment: BTreeMap::new(),
        checks: Vec::new(),
    }
}

/// A new service identifier.
pub fn service_id() -> ServiceId {
    ServiceId(Uuid(mint()))
}

fn id<T>(wrap: impl FnOnce(Uuid) -> T, text: &str) -> T {
    wrap(Uuid(text.to_owned()))
}

/// A `Ready` node with an open session, and one instance running `sh -c <snippet>` assigned to
/// it: the rows pushed to `rows`.
fn placed(
    rows: &mut Vec<Row>,
    cloud_id: &CloudId,
    scope_id: &ScopeId,
    snippet: &str,
    digest: &str,
) -> Placed {
    let now = timestamp(OffsetDateTime::now_utc());
    let placed = Placed {
        node_id: mint(),
        instance_id: mint(),
    };
    let node_id = id(NodeId, &placed.node_id);
    let instance_id = id(InstanceId, &placed.instance_id);
    let workload_id = WorkloadId(Uuid(mint()));
    let session_id = NodeSessionId(Uuid(mint()));
    let artifact_digest = ArtifactDigest(digest.to_owned());

    rows.push(Row::Node(NodeSnapshot {
        state: NodeState::Ready,
        data: NodeData {
            node_id: node_id.clone(),
            cloud_id: cloud_id.clone(),
            zone: ZoneName("z1".to_owned()),
            node_pool_id: None,
            role: NodeRole::Worker,
            join_token_expires_at: now.clone(),
            join_token_used: true,
            capacity: Some(vec![ResourceAmount {
                resource_class: ResourceClassName("cpu".to_owned()),
                amount: 1,
            }]),
            isolation_facts: Some(IsolationFacts {
                kvm_present: false,
                cgroups_delegated: true,
                egress_denied: true,
                gpu_passthrough: false,
                levels: vec![IsolationLevel::Container],
            }),
            observed_at: Some(now.clone()),
        },
    }));
    rows.push(Row::NodeSession(NodeSessionSnapshot {
        state: NodeSessionState::Open,
        data: NodeSessionData {
            session_id: session_id.clone(),
            node_id: node_id.clone(),
            agent_version: "0.1.0".to_owned(),
            opened_at: now.clone(),
            last_seen_at: now.clone(),
            ended_at: None,
            end_reason: None,
        },
    }));
    rows.push(Row::Artifact(ArtifactSnapshot {
        state: ArtifactState::Registered,
        data: ArtifactData {
            artifact_digest: artifact_digest.clone(),
            scope_id: scope_id.clone(),
            kind: ArtifactKind::Snippet,
            size_bytes: i64::try_from(snippet.len()).expect("a short snippet"),
            signature: "unsigned".to_owned(),
            signer_key_id: "none".to_owned(),
            built_from: None,
        },
    }));
    let processes = vec![Process {
        name: ProcessName("main".to_owned()),
        role: ProcessRole::Main,
        artifact_digest: None,
        command: vec!["sh".to_owned(), "-c".to_owned(), snippet.to_owned()],
        environment: BTreeMap::new(),
        checks: Vec::new(),
    }];
    let restart_policy = RestartPolicy {
        mode: RestartMode::Never,
        backoff_initial_seconds: 1,
        backoff_max_seconds: 60,
        backoff_limit: 0,
        backoff_window_seconds: 600,
    };
    rows.push(Row::Workload(WorkloadSnapshot {
        state: WorkloadState::Defined,
        data: WorkloadData {
            workload_id: workload_id.clone(),
            scope_id: scope_id.clone(),
            formation_id: FormationId(Uuid(mint())),
            formation_revision_id: FormationRevisionId(Uuid(mint())),
            environment_id: EnvironmentId(Uuid(mint())),
            component: ComponentName("web".to_owned()),
            package_kind: PackageKind::Snippet,
            artifact_digest: artifact_digest.clone(),
            runtime_id: RuntimeId(Uuid(mint())),
            isolation: IsolationLevel::Container,
            shape: ShapeKind::Service,
            settings: ShapeSettings::Service(ServiceSettings { stateful: false }),
            claims: Vec::new(),
            processes: processes.clone(),
            restart_policy: restart_policy.clone(),
            priority: PriorityClassName("default".to_owned()),
        },
    }));
    rows.push(Row::Instance(InstanceSnapshot {
        state: InstanceState::Scheduled,
        data: InstanceData {
            instance_id: instance_id.clone(),
            workload_id,
            scope_id: scope_id.clone(),
            artifact_digest,
            minimum_isolation: IsolationLevel::Container,
            isolation: Some(IsolationLevel::Container),
            node_id: Some(node_id.clone()),
            job_id: None,
            task_index: None,
            invocation_id: None,
            box_id: None,
            session_id: None,
            replica_index: None,
            service_id: None,
            replica_counted_out: false,
            warm_pool_id: None,
            restored_from: None,
            desired: DesiredRun::Run,
            ready: false,
            health: InstanceHealth::Unknown,
            processes,
            restart_policy,
            restarts: 0,
            restarts_in_window: 0,
            backoff_seconds: None,
            exit_code: None,
            failure_reason: None,
            eviction_reason: None,
        },
    }));
    rows.push(Row::Assignment(AssignmentSnapshot {
        state: AssignmentState::Active,
        data: AssignmentData {
            assignment_id: AssignmentId(Uuid(mint())),
            instance_id,
            node_id,
            session_id,
            placement_decision_id: PlacementDecisionId(Uuid(mint())),
            min_isolation: IsolationLevel::Container,
            desired: DesiredState::Run,
            snapshot_id: None,
            assigned_at: now.clone(),
            lease_renewed_at: now,
            ended_at: None,
        },
    }));
    placed
}
