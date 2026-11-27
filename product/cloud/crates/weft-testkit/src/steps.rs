//! The operator and tenant steps of the path, each as its own actor with the input of
//! `sim/experiments/full-setup.yaml` (setup steps `create-cloud` to `grant-quota-acme`), then the
//! workload definitions (the steps of globex's formation `site` in the default cluster, from
//! `create-formation-site` to the service, with a `Snippet` package in place of the image).
//! Each step asserts its accepting outcome and answers the identity the command answered.

use std::io;

use serde_json::{json, Value};
use weft_client::{Answer, Client};

/// The operator of the cloud.
pub const OPERATOR: &str = "weft.actors.ClusterOperator";
/// The owner of a tenant.
pub const TENANT_OWNER: &str = "weft.actors.TenantOwner";
/// The node agent of a node.
pub const NODE_AGENT: &str = "weft.actors.NodeAgent";
/// The administrator of an account.
pub const ACCOUNT_ADMIN: &str = "weft.actors.AccountAdmin";
/// The deployer of an account.
pub const DEPLOYER: &str = "weft.actors.Deployer";

/// The inputs of the steps, as `sim/experiments/full-setup.yaml` writes them.
pub mod input {
    use serde_json::{json, Value};

    /// `create-cloud`.
    pub fn create_cloud() -> Value {
        json!({
            "name": "sim",
            "placement": "Hosts",
            "access_mode": "Federated",
            "policy": {"minimum_isolation": "Container"},
            "control_plane": {"replicas": 3},
        })
    }

    /// `declare-zone-a` and `declare-zone-b`.
    pub fn declare_zone(cloud_id: &str, zone: &str) -> Value {
        json!({"zone": zone, "cloud_id": cloud_id})
    }

    /// `define-priority`.
    pub fn define_priority_class() -> Value {
        json!({"name": "standard", "value": 100, "preempts": false, "preemptible": true, "grace_seconds": 30})
    }

    /// `create-shared-pool`.
    pub fn create_node_pool() -> Value {
        json!({"name": "shared", "zone": "z1", "machine_type": "m1", "image": "weft-node", "min_size": 0, "max_size": 8})
    }

    /// `provision-shared`; the expiry is the caller's, since the file's fixed instant lies in the
    /// past of any real clock.
    pub fn provision_pool_node(node_pool_id: &str, join_token_expires_at: &str) -> Value {
        json!({"node_pool_id": node_pool_id, "join_token_expires_at": join_token_expires_at})
    }

    /// `create-default-cluster`.
    pub fn create_default_cluster(cloud_id: &str) -> Value {
        json!({"cloud_id": cloud_id, "name": "default", "min_isolation": "Container", "zones": ["z1", "z2"]})
    }

    /// `create-tenant-acme` (`name` `acme`).
    pub fn create_tenant(name: &str) -> Value {
        json!({"name": name})
    }

    /// `create-account-acme`.
    pub fn create_account(tenant_id: &str) -> Value {
        json!({"tenant_id": tenant_id, "name": "prod"})
    }

    /// `grant-quota-acme`.
    pub fn grant_top_quota(account_id: &str) -> Value {
        json!({"account_id": account_id, "resource_class": "cpu", "limit": 64})
    }

    /// The digest of the skeleton's snippet artifact.
    pub const ARTIFACT_DIGEST: &str = "sha256:weft-skeleton-snippet";

    /// `create-formation-site`: formation `site` in `cluster_id` (the default cluster).
    pub fn create_formation(account_id: &str, cluster_id: &str) -> Value {
        json!({"account_id": account_id, "name": "site", "zones": ["z1", "z2"], "cluster_id": cluster_id})
    }

    /// `add-component-site`.
    pub fn add_component(formation_id: &str, account_id: &str) -> Value {
        json!({"formation_id": formation_id, "account_id": account_id, "name": "site"})
    }

    /// `commit-site`.
    pub fn commit_revision(formation_id: &str, account_id: &str) -> Value {
        json!({"formation_id": formation_id, "account_id": account_id})
    }

    /// `env-site`.
    pub fn create_environment(formation_id: &str, account_id: &str) -> Value {
        json!({"formation_id": formation_id, "account_id": account_id, "name": "prod", "protected": false})
    }

    /// `artifact-globex`, of kind `Snippet` under [`ARTIFACT_DIGEST`].
    pub fn register_artifact(account_id: &str) -> Value {
        json!({
            "artifact_digest": ARTIFACT_DIGEST,
            "account_id": account_id,
            "kind": "Snippet",
            "size_bytes": 9,
            "signature": "sig",
            "signer_key_id": "k1",
            "built_from": null,
        })
    }

    /// `register-runtime`, running `package_kinds` at `Container` isolation or stronger.
    pub fn register_runtime(package_kinds: &[&str]) -> Value {
        json!({"name": "snippet", "version": "1", "package_kinds": package_kinds, "minimum_isolation": "Container"})
    }

    /// The skeleton's one `Main` process, `sh -c "echo weft"`.
    pub fn process() -> Value {
        json!({"name": "main", "role": "Main", "command": ["sh", "-c", "echo weft"], "environment": {}, "checks": []})
    }

    /// What `DefineWorkload` names: the formation, its revision and environment, and the runtime.
    #[derive(Debug, Clone, Copy)]
    pub struct WorkloadOf<'a> {
        /// The account.
        pub account_id: &'a str,
        /// The formation.
        pub formation_id: &'a str,
        /// Its committed revision.
        pub formation_revision_id: &'a str,
        /// Its environment.
        pub environment_id: &'a str,
        /// The runtime.
        pub runtime_id: &'a str,
    }

    /// `workload-site`: component `site`, a `Snippet` of [`ARTIFACT_DIGEST`] with the one
    /// [`process`], at `Container` isolation, shaped `Service`.
    pub fn define_workload(of: WorkloadOf<'_>) -> Value {
        json!({
            "account_id": of.account_id,
            "formation_id": of.formation_id,
            "formation_revision_id": of.formation_revision_id,
            "environment_id": of.environment_id,
            "component": "site",
            "package_kind": "Snippet",
            "artifact_digest": ARTIFACT_DIGEST,
            "runtime_id": of.runtime_id,
            "isolation": "Container",
            "shape": "Service",
            "settings": {"shape": "service", "value": {"stateful": false}},
            "claims": [{"name": "cpu", "class": "cpu", "amount": 1, "selector": {}}],
            "processes": [process()],
            "restart_policy": {
                "mode": "Always",
                "backoff_initial_seconds": 1,
                "backoff_max_seconds": 60,
                "backoff_limit": 5,
                "backoff_window_seconds": 600,
            },
            "priority": "standard",
        })
    }

    /// `service-site`: one replica, scaled between 1 and 6 on requests.
    pub fn define_service(workload_id: &str, account_id: &str) -> Value {
        json!({
            "workload_id": workload_id,
            "account_id": account_id,
            "replicas": 1,
            "scale_rule": {"min_replicas": 1, "max_replicas": 6, "metric": "Requests", "target": 10},
            "disruption_budget": null,
            "sleep_policy": null,
            "wake_deadline": null,
        })
    }
}

/// What `ProvisionPoolNode` answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provisioned {
    /// The node made.
    pub node_id: String,
    /// The one-time join token.
    pub join_token: String,
    /// The expiry sent.
    pub join_token_expires_at: String,
}

/// The identities the operator and tenant path made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    /// `CreateCloud`.
    pub cloud_id: String,
    /// `CreateNodePool`.
    pub node_pool_id: String,
    /// `ProvisionPoolNode`.
    pub node: Provisioned,
    /// `CreateDefaultCluster`.
    pub cluster_id: String,
    /// `CreateTenant`.
    pub tenant_id: String,
    /// `CreateAccount`.
    pub account_id: String,
    /// `GrantTopQuota`.
    pub quota_id: String,
}

/// Runs the operator and tenant path in the order of the full setup: the cloud, zones `z1` and
/// `z2`, the priority class, the shared pool, one node, the default cluster, tenant `acme`, its
/// account and its quota.
pub fn run_path(client: &Client) -> Path {
    let cloud_id = create_cloud(client);
    declare_zone(client, &cloud_id, "z1");
    declare_zone(client, &cloud_id, "z2");
    define_priority_class(client);
    let node_pool_id = create_node_pool(client);
    let node = provision_pool_node(client, &node_pool_id);
    let cluster_id = create_default_cluster(client, &cloud_id);
    let tenant_id = create_tenant(client, "acme");
    let account_id = create_account(client, &tenant_id);
    let quota_id = grant_top_quota(client, &account_id);
    Path {
        cloud_id,
        node_pool_id,
        node,
        cluster_id,
        tenant_id,
        account_id,
        quota_id,
    }
}

/// The body of an answer with a 2xx status and the outcome `outcome`.
fn accepted(answer: io::Result<Answer>, command: &str, outcome: &str) -> Value {
    let answer = answer
        .unwrap_or_else(|error| panic!("{command}: the control host did not answer: {error}"));
    assert!(
        (200..300).contains(&answer.status) && answer.body["outcome"] == json!(outcome),
        "{command}: expected outcome {outcome}, the control host answered {} {}",
        answer.status,
        answer.body
    );
    answer.body
}

/// The member `field` of the first published event's payload.
fn published(body: &Value, field: &str) -> String {
    body["published"][0]["payload"][field]
        .as_str()
        .unwrap_or_else(|| panic!("no {field} in the published payload of {body}"))
        .to_owned()
}

fn operator(client: &Client, command: &str, input: Value, outcome: &str) -> Value {
    accepted(
        client.command(command, OPERATOR, json!({}), input),
        command,
        outcome,
    )
}

/// `CreateCloud` as ClusterOperator: the cloud's identity.
pub fn create_cloud(client: &Client) -> String {
    let body = operator(
        client,
        "weft.infrastructure.CreateCloud",
        input::create_cloud(),
        "created",
    );
    published(&body, "cloud_id")
}

/// `DeclareZone` as ClusterOperator.
pub fn declare_zone(client: &Client, cloud_id: &str, zone: &str) {
    operator(
        client,
        "weft.infrastructure.DeclareZone",
        input::declare_zone(cloud_id, zone),
        "declared",
    );
}

/// `DefinePriorityClass` as ClusterOperator.
pub fn define_priority_class(client: &Client) {
    operator(
        client,
        "weft.infrastructure.DefinePriorityClass",
        input::define_priority_class(),
        "defined",
    );
}

/// `CreateNodePool` as ClusterOperator: the pool's identity.
pub fn create_node_pool(client: &Client) -> String {
    let body = operator(
        client,
        "weft.infrastructure.CreateNodePool",
        input::create_node_pool(),
        "created",
    );
    published(&body, "node_pool_id")
}

/// `ProvisionPoolNode` as ClusterOperator, the token expiring an hour from now: the node and its
/// join token.
pub fn provision_pool_node(client: &Client, node_pool_id: &str) -> Provisioned {
    let expires = in_an_hour();
    let body = operator(
        client,
        "weft.infrastructure.ProvisionPoolNode",
        input::provision_pool_node(node_pool_id, &expires),
        "provisioned",
    );
    let response = |field: &str| {
        body["response"][field]
            .as_str()
            .unwrap_or_else(|| panic!("no {field} in the response of {body}"))
            .to_owned()
    };
    Provisioned {
        node_id: response("node_id"),
        join_token: response("join_token"),
        join_token_expires_at: expires,
    }
}

/// `JoinNode` as NodeAgent with `join_token`: the answer's body, whatever its outcome.
pub fn join_node(client: &Client, node_id: &str, join_token: &str) -> Value {
    client
        .command(
            "weft.infrastructure.JoinNode",
            NODE_AGENT,
            json!({}),
            json!({"node_id": node_id, "join_token": join_token}),
        )
        .unwrap_or_else(|error| panic!("JoinNode: the control host did not answer: {error}"))
        .body
}

/// `CreateDefaultCluster` as ClusterOperator: the cluster's identity.
pub fn create_default_cluster(client: &Client, cloud_id: &str) -> String {
    let body = operator(
        client,
        "weft.cluster.CreateDefaultCluster",
        input::create_default_cluster(cloud_id),
        "created",
    );
    published(&body, "cluster_id")
}

/// `CreateTenant` as ClusterOperator: the tenant's identity.
pub fn create_tenant(client: &Client, name: &str) -> String {
    let body = operator(
        client,
        "weft.accounts.CreateTenant",
        input::create_tenant(name),
        "created",
    );
    published(&body, "tenant_id")
}

/// `CreateAccount` as TenantOwner of `tenant_id`: the account's identity.
pub fn create_account(client: &Client, tenant_id: &str) -> String {
    let command = "weft.accounts.CreateAccount";
    let answer = client.command(
        command,
        TENANT_OWNER,
        json!({"tenant_id": tenant_id}),
        input::create_account(tenant_id),
    );
    published(&accepted(answer, command, "created"), "account_id")
}

/// `GrantTopQuota` as ClusterOperator: the quota's identity.
pub fn grant_top_quota(client: &Client, account_id: &str) -> String {
    let body = operator(
        client,
        "weft.accounts.GrantTopQuota",
        input::grant_top_quota(account_id),
        "granted",
    );
    published(&body, "quota_id")
}

/// The caller of a request that acts in `account_id` of `tenant_id`.
pub fn account_caller(tenant_id: &str, account_id: &str) -> Value {
    json!({"tenant_id": tenant_id, "account_id": account_id})
}

/// `command` as `actor` with `caller`: the body of its accepting outcome `outcome`.
fn in_account(
    client: &Client,
    actor: &str,
    caller: &Value,
    command: &str,
    input: Value,
    outcome: &str,
) -> Value {
    accepted(
        client.command(command, actor, caller.clone(), input),
        command,
        outcome,
    )
}

/// `CreateFormation` as AccountAdmin: formation `site` of `account_id` in `cluster_id`, its
/// identity.
pub fn create_formation(
    client: &Client,
    caller: &Value,
    account_id: &str,
    cluster_id: &str,
) -> String {
    let command = "weft.formations.CreateFormation";
    let input = input::create_formation(account_id, cluster_id);
    published(
        &in_account(client, ACCOUNT_ADMIN, caller, command, input, "created"),
        "formation_id",
    )
}

/// `AddComponent` as Deployer: component `site` of `formation_id`, its identity.
pub fn add_component(
    client: &Client,
    caller: &Value,
    formation_id: &str,
    account_id: &str,
) -> String {
    let command = "weft.formations.AddComponent";
    let input = input::add_component(formation_id, account_id);
    published(
        &in_account(client, DEPLOYER, caller, command, input, "added"),
        "component_id",
    )
}

/// `CommitRevision` as Deployer: the revision's identity.
pub fn commit_revision(
    client: &Client,
    caller: &Value,
    formation_id: &str,
    account_id: &str,
) -> String {
    let command = "weft.formations.CommitRevision";
    let input = input::commit_revision(formation_id, account_id);
    published(
        &in_account(client, DEPLOYER, caller, command, input, "committed"),
        "revision_id",
    )
}

/// `CreateEnvironment` as AccountAdmin: environment `prod`, not protected, its identity.
pub fn create_environment(
    client: &Client,
    caller: &Value,
    formation_id: &str,
    account_id: &str,
) -> String {
    let command = "weft.formations.CreateEnvironment";
    let input = input::create_environment(formation_id, account_id);
    published(
        &in_account(client, ACCOUNT_ADMIN, caller, command, input, "created"),
        "environment_id",
    )
}

/// `RegisterArtifact` as Deployer: the `Snippet` artifact, its digest.
pub fn register_artifact(client: &Client, caller: &Value, account_id: &str) -> String {
    let command = "weft.artifacts.RegisterArtifact";
    let input = input::register_artifact(account_id);
    published(
        &in_account(client, DEPLOYER, caller, command, input, "registered"),
        "artifact_digest",
    )
}

/// `RegisterRuntime` as ClusterOperator, running `package_kinds`: the runtime's identity.
pub fn register_runtime(client: &Client, package_kinds: &[&str]) -> String {
    let body = operator(
        client,
        "weft.workloads.RegisterRuntime",
        input::register_runtime(package_kinds),
        "registered",
    );
    published(&body, "runtime_id")
}

/// `DefineWorkload` as Deployer with `input`: the workload's identity.
pub fn define_workload(client: &Client, caller: &Value, input: Value) -> String {
    let command = "weft.workloads.DefineWorkload";
    published(
        &in_account(client, DEPLOYER, caller, command, input, "defined"),
        "workload_id",
    )
}

/// `DefineService` as Deployer: one replica of `workload_id`, the service's identity.
pub fn define_service(
    client: &Client,
    caller: &Value,
    workload_id: &str,
    account_id: &str,
) -> String {
    let command = "weft.workloads.DefineService";
    let input = input::define_service(workload_id, account_id);
    published(
        &in_account(client, DEPLOYER, caller, command, input, "defined"),
        "service_id",
    )
}

/// The identities the workload definitions made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definitions {
    /// `CreateFormation`.
    pub formation_id: String,
    /// `AddComponent`.
    pub component_id: String,
    /// `CommitRevision`.
    pub revision_id: String,
    /// `CreateEnvironment`.
    pub environment_id: String,
    /// `RegisterArtifact`.
    pub artifact_digest: String,
    /// `RegisterRuntime`, running `Snippet`.
    pub runtime_id: String,
    /// `DefineWorkload`.
    pub workload_id: String,
    /// `DefineService`.
    pub service_id: String,
}

/// Runs the workload definitions after the operator and tenant `path`, each as its own actor in
/// the path's account: formation `site` in the default cluster, its component `site`, a
/// revision, environment `prod`, the `Snippet` artifact, a runtime of `Snippet`, the workload
/// and its service of one replica.
pub fn run_definitions(client: &Client, path: &Path) -> Definitions {
    let caller = account_caller(&path.tenant_id, &path.account_id);
    let account_id = path.account_id.as_str();
    let formation_id = create_formation(client, &caller, account_id, &path.cluster_id);
    let component_id = add_component(client, &caller, &formation_id, account_id);
    let revision_id = commit_revision(client, &caller, &formation_id, account_id);
    let environment_id = create_environment(client, &caller, &formation_id, account_id);
    let artifact_digest = register_artifact(client, &caller, account_id);
    let runtime_id = register_runtime(client, &["Snippet"]);
    let workload = input::define_workload(input::WorkloadOf {
        account_id,
        formation_id: &formation_id,
        formation_revision_id: &revision_id,
        environment_id: &environment_id,
        runtime_id: &runtime_id,
    });
    let workload_id = define_workload(client, &caller, workload);
    let service_id = define_service(client, &caller, &workload_id, account_id);
    Definitions {
        formation_id,
        component_id,
        revision_id,
        environment_id,
        artifact_digest,
        runtime_id,
        workload_id,
        service_id,
    }
}

/// An hour from now, to the second, in RFC 3339 (`2026-11-27T17:00:00Z`).
pub fn in_an_hour() -> String {
    let at = time::OffsetDateTime::now_utc() + time::Duration::hours(1);
    at.replace_nanosecond(0)
        .expect("zero nanoseconds is valid")
        .format(&time::format_description::well_known::Rfc3339)
        .expect("UTC time formats")
}
