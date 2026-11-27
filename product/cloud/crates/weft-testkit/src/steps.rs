//! The operator and tenant steps of the path, each as its own actor with the input of
//! `sim/experiments/full-setup.yaml` (setup steps `create-cloud` to `grant-quota-acme`). Each
//! step asserts its accepting outcome and answers the identity the command answered.

use std::io;

use serde_json::{json, Value};
use weft_client::{Answer, Client};

/// The operator of the cloud.
pub const OPERATOR: &str = "weft.actors.ClusterOperator";
/// The owner of a tenant.
pub const TENANT_OWNER: &str = "weft.actors.TenantOwner";
/// The node agent of a node.
pub const NODE_AGENT: &str = "weft.actors.NodeAgent";

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

/// An hour from now, to the second, in RFC 3339 (`2026-11-27T17:00:00Z`).
pub fn in_an_hour() -> String {
    let at = time::OffsetDateTime::now_utc() + time::Duration::hours(1);
    at.replace_nanosecond(0)
        .expect("zero nanoseconds is valid")
        .format(&time::format_description::well_known::Rfc3339)
        .expect("UTC time formats")
}
