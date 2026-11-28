//! A scope placed on a named pool or host runs its process workloads there
//! (`story:slot-node-pool-placement`): one instance of a pool scope's workload on each `Ready`
//! node of the pool and on no other node, a host scope's workload on that node only, and a
//! workload `per: <pool scope>` once per node of the pool, in its own scope, with `${instance}`
//! of its command written as the node's name at instance creation. A node's name is its
//! identity (`weft_impl::placement::node_name`: `weft.infrastructure.Node` has no name field).
//!
//! The control host's own controller loop runs (`--tick-ms` 200): the level step of
//! `weft-controllers` makes the instances, the binding `r1-submit-scheduling-request` asks for
//! their placement and the Scheduler's step places them. Nodes join, observe and open their
//! session by command; no node agent, no substrate daemon, no network beyond loopback.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use weft_client::{Answer, Client, Expect};
use weft_impl::context::{mint, timestamp};
use weft_impl::Ports;
use weft_testkit::cloudspec::{self, ProcessWorld};
use weft_testkit::steps::{self, CONTROL_PLANE, NODE_AGENT, OPERATOR};
use weft_testkit::ControlHost;
use weft_types::behaviour::{
    ComponentStorage, ComponentWorkloadStorage, ReleaseStepStorage, ReleaseStorage, WorkloadStorage,
};

/// How long the controllers may take to place every instance a step of the test asks for.
const PLACE_LIMIT: Duration = Duration::from_secs(20);

/// The name of the pool of the acceptance.
const POOL: &str = "build-nodes";

/// The commands of the three process parts, by part.
const POOL_DAEMON: &str = "/usr/local/bin/node-daemon";
const HOST_DAEMON: &str = "/usr/local/bin/host-daemon";
const REFRESH: &str = "/usr/local/bin/refresh";

/// A control host with a cloud in zone `z1`, the default cluster of minimum isolation `None`
/// (a process runs with no sandbox), pools `build-nodes` and `hosts`, the tenant path, formation `site`
/// pinned to the cluster, its environment and the process artifact and runtime.
struct World {
    host: ControlHost,
    client: Client,
    process: ProcessWorld,
    pool_id: String,
    hosts_id: String,
}

impl World {
    fn new() -> Self {
        let host = ControlHost::start();
        let client = host.client();
        let cloud_id = steps::create_cloud(&client);
        steps::declare_zone(&client, &cloud_id, "z1");
        steps::define_priority_class(&client);
        let pool = |name: &str| {
            let body = operator(
                &client,
                "weft.infrastructure.CreateNodePool",
                json!({"name": name, "zone": "z1", "machine_type": "m1", "image": "weft-node", "min_size": 0, "max_size": 8}),
                "created",
            );
            published(&body, "node_pool_id")
        };
        let pool_id = pool(POOL);
        let hosts_id = pool("hosts");
        let cluster = operator(
            &client,
            "weft.cluster.CreateDefaultCluster",
            json!({"cloud_id": cloud_id, "name": "default", "min_isolation": "None", "zones": ["z1"]}),
            "created",
        );
        let cluster_id = published(&cluster, "cluster_id");
        let tenant_id = steps::create_tenant(&client, "acme");
        let account_id = steps::create_account(&client, &tenant_id);
        let scope_id = steps::record_account_scope(&client, &account_id);
        steps::grant_top_quota(&client, &account_id);
        let mut caller = steps::account_caller(&tenant_id, &account_id, &scope_id);
        caller["principal_id"] = json!(mint());
        let formation_id = steps::create_formation(&client, &caller, &scope_id);
        steps::pin_formation(&client, &caller, &scope_id, &cluster_id, &formation_id);
        let environment_id = steps::create_environment(&client, &caller, &formation_id, &scope_id);
        cloudspec::register_process_artifact(&client, &caller, &scope_id);
        let runtime_id = cloudspec::register_process_runtime(&client);
        Self {
            host,
            client,
            process: ProcessWorld {
                caller,
                scope_id,
                formation_id,
                environment_id,
                runtime_id,
            },
            pool_id,
            hosts_id,
        }
    }

    /// A node of the pool, joined, observed (isolation `None`), with an open session and 8 cpu:
    /// its identity.
    fn pool_node(&self) -> String {
        let node = steps::provision_pool_node(&self.client, &self.pool_id);
        self.ready(&node.node_id, &node.join_token);
        node.node_id
    }

    /// The node of pool `hosts`, made ready as [`World::pool_node`]: its identity. (A node of no
    /// pool, `ProvisionNode`, has no behaviour yet; a host placement names a node of any pool.)
    fn host_node(&self) -> String {
        let node = steps::provision_pool_node(&self.client, &self.hosts_id);
        self.ready(&node.node_id, &node.join_token);
        node.node_id
    }

    fn ready(&self, node_id: &str, join_token: &str) {
        let joined = steps::join_node(&self.client, node_id, join_token);
        assert_eq!(joined["outcome"], json!("joined"), "{joined}");
        let now = || timestamp(time::OffsetDateTime::now_utc()).0;
        let observed = command(
            &self.client,
            "weft.infrastructure.RecordNodeObservation",
            NODE_AGENT,
            json!({
                "node_id": node_id,
                "isolation_facts": {
                    "kvm_present": false,
                    "cgroups_delegated": true,
                    "egress_denied": true,
                    "gpu_passthrough": false,
                    "levels": ["None", "Container"],
                },
                "observed_at": now(),
            }),
        );
        assert_eq!(observed["outcome"], json!("ready"), "{observed}");
        let opened = command(
            &self.client,
            "weft.nodes.OpenNodeSession",
            CONTROL_PLANE,
            json!({"node_id": node_id, "agent_version": "0.1.0", "received_at": now()}),
        );
        assert_eq!(opened["outcome"], json!("opened"), "{opened}");
        let reported = command(
            &self.client,
            "weft.inventory.ReportNodeCapacity",
            NODE_AGENT,
            json!({
                "node_id": node_id,
                "capacity": [{"resource_class": "cpu", "amount": 8}],
                "observed_at": now(),
            }),
        );
        assert_eq!(reported["outcome"], json!("reported"), "{reported}");
    }

    /// `parts` submitted, planned, made `Ready` and applied.
    fn apply(&self, parts: &[(&str, Value)]) -> Answer {
        self.process
            .apply(&self.client, &cloudspec::submission_with_parts(parts))
    }

    /// Every instance of the workloads the apply answer `applied` names: (part, node, command).
    fn instances(&self, applied: &Value) -> Vec<(String, Option<String>, Vec<String>)> {
        let all = self
            .client
            .view("weft.workloads.Instances", &[])
            .ok_body(200);
        let commands: BTreeMap<String, Vec<String>> = all["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .map(|row| {
                let command = row["processes"][0]["command"]
                    .as_array()
                    .expect("a command")
                    .iter()
                    .map(|word| word.as_str().expect("a word").to_owned())
                    .collect();
                (text(&row["instance_id"]), command)
            })
            .collect();
        let mut found = Vec::new();
        for part in applied["parts"].as_array().expect("parts") {
            let workload_id = text(&part["workload_id"]);
            let list = self
                .client
                .view(
                    "weft.workloads.InstanceList",
                    &[("workload_id", workload_id.as_str())],
                )
                .ok_body(200);
            for row in list["rows"].as_array().expect("rows") {
                let command = commands
                    .get(&text(&row["instance_id"]))
                    .cloned()
                    .unwrap_or_default();
                found.push((
                    text(&part["part"]),
                    row["node_id"].as_str().map(str::to_owned),
                    command,
                ));
            }
        }
        found.sort();
        found
    }

    /// The instances of `applied` once `wanted` of them exist and each is placed on a node.
    fn placed(&self, applied: &Value, wanted: usize) -> Vec<(String, String, Vec<String>)> {
        let started = Instant::now();
        loop {
            let rows = self.instances(applied);
            if rows.len() >= wanted && rows.iter().all(|row| row.1.is_some()) {
                return rows
                    .into_iter()
                    .map(|(part, node, command)| (part, node.unwrap_or_default(), command))
                    .collect();
            }
            assert!(
                started.elapsed() < PLACE_LIMIT,
                "not {wanted} placed instances {PLACE_LIMIT:?} after the step: {rows:?}; log: {:?}",
                self.host.log_after(0)
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

fn command(client: &Client, name: &str, actor: &str, input: Value) -> Value {
    client
        .command(name, actor, json!({}), input)
        .expect("the control host answers")
        .body
}

fn operator(client: &Client, name: &str, input: Value, outcome: &str) -> Value {
    let body = command(client, name, OPERATOR, input);
    assert_eq!(body["outcome"], json!(outcome), "{name}: {body}");
    body
}

fn published(body: &Value, field: &str) -> String {
    text(&body["published"][0]["payload"][field])
}

fn text(value: &Value) -> String {
    value
        .as_str()
        .unwrap_or_else(|| panic!("not a text: {value}"))
        .to_owned()
}

/// A process part of `scope`, running `program` as `deployer`.
fn process(scope: &str, program: &str, args: &[&str]) -> Value {
    let mut command = vec![program];
    command.extend_from_slice(args);
    json!({
        "block": "workloads",
        "kind": "process",
        "in": scope,
        "isolation": "none",
        "run_as": {"user": "deployer"},
        "command": command,
    })
}

/// The architecture of the acceptance: scope `build-node` on pool `pool` with one process,
/// scope `workstation` on host `host` with one process, and one process of `workstation`
/// `per: build-node` whose command names `${instance}`.
fn architecture(pool: &str, host: &str) -> Vec<(&'static str, Value)> {
    let mut refresh = process("workstation", REFRESH, &["${instance}", "60"]);
    refresh["per"] = json!("build-node");
    vec![
        (
            "build-node",
            json!({"block": "scope", "placement": {"pool": pool, "lease": {"max_hours": 12}}}),
        ),
        (
            "workstation",
            json!({"block": "scope", "placement": {"host": host}}),
        ),
        (
            "node-daemon",
            process("build-node", POOL_DAEMON, &["serve"]),
        ),
        (
            "host-daemon",
            process("workstation", HOST_DAEMON, &["serve"]),
        ),
        ("toolchain-refresh", refresh),
    ]
}

/// What each part runs where: per part, the multiset of (node, command) of its instances.
fn by_part(rows: &[(String, String, Vec<String>)]) -> BTreeMap<String, Vec<(String, Vec<String>)>> {
    let mut parts: BTreeMap<String, Vec<(String, Vec<String>)>> = BTreeMap::new();
    for (part, node, command) in rows {
        parts
            .entry(part.clone())
            .or_default()
            .push((node.clone(), command.clone()));
    }
    for rows in parts.values_mut() {
        rows.sort();
    }
    parts
}

/// The expected instances with pool nodes `pool_nodes` and host node `workstation`.
fn expected(
    pool_nodes: &[String],
    workstation: &str,
) -> BTreeMap<String, Vec<(String, Vec<String>)>> {
    let words = |words: &[&str]| words.iter().map(|word| (*word).to_owned()).collect();
    let mut pool: Vec<_> = pool_nodes
        .iter()
        .map(|node| (node.clone(), words(&[POOL_DAEMON, "serve"])))
        .collect();
    pool.sort();
    let mut refresh: Vec<_> = pool_nodes
        .iter()
        .map(|node| (workstation.to_owned(), words(&[REFRESH, node, "60"])))
        .collect();
    refresh.sort();
    BTreeMap::from([
        ("node-daemon".to_owned(), pool),
        (
            "host-daemon".to_owned(),
            vec![(workstation.to_owned(), words(&[HOST_DAEMON, "serve"]))],
        ),
        ("toolchain-refresh".to_owned(), refresh),
    ])
}

/// The story's acceptance: with pool `build-nodes` of two ready nodes and a node `workstation`,
/// the applied architecture runs one instance of the pool process on each pool node and none
/// elsewhere, the host process on `workstation` only, and two `per` instances on `workstation`
/// whose commands carry the two node names; a third pool node gets its own two; and further
/// level passes make no second instance of any (workload, node).
#[test]
fn a_pool_scope_and_a_host_scope_run_one_instance_per_node_and_one_per_pool_node() {
    let w = World::new();
    let mut pool_nodes = vec![w.pool_node(), w.pool_node()];
    let workstation = w.host_node();
    let answer = w.apply(&architecture(POOL, &workstation));
    assert_eq!(answer.status, 202, "{}", answer.body);
    let applied = answer.body;

    let rows = w.placed(&applied, 5);
    assert_eq!(
        by_part(&rows),
        expected(&pool_nodes, &workstation),
        "{rows:?}"
    );

    pool_nodes.push(w.pool_node());
    let rows = w.placed(&applied, 7);
    assert_eq!(
        by_part(&rows),
        expected(&pool_nodes, &workstation),
        "{rows:?}"
    );

    // Ten more ticks of the controller loop (200 ms each) make nothing more.
    std::thread::sleep(Duration::from_millis(2_000));
    let rows = w.instances(&applied);
    assert_eq!(rows.len(), 7, "{rows:?}");
    let distinct: BTreeSet<_> = rows.iter().collect();
    assert_eq!(distinct.len(), 7, "{rows:?}");
}

/// A placement naming no pool, or no node, refuses the apply by a named refusal before any
/// command runs, and so does a `per` naming a scope with no placement (the golden's `jobs`): no
/// release, step, component, workload or assignment is written.
#[test]
fn a_placement_naming_no_pool_or_no_node_is_refused_by_name_and_writes_nothing() {
    let mut w = World::new();
    w.pool_node();
    let workstation = w.host_node();
    let pool = w.apply(&architecture("nowhere", &workstation));
    assert_eq!(
        (
            pool.status,
            pool.body["refusal"].clone(),
            pool.body["name"].clone()
        ),
        (422, json!("UnknownPlacementPool"), json!("nowhere")),
        "{}",
        pool.body
    );
    let host = w.apply(&architecture(POOL, "no-such-node"));
    assert_eq!(
        (
            host.status,
            host.body["refusal"].clone(),
            host.body["name"].clone()
        ),
        (422, json!("UnknownPlacementHost"), json!("no-such-node")),
        "{}",
        host.body
    );
    let mut unplaced = architecture(POOL, &workstation);
    unplaced[4].1["per"] = json!("jobs");
    let per = w.apply(&unplaced);
    assert_eq!(
        (
            per.status,
            per.body["refusal"].clone(),
            per.body["scope"].clone()
        ),
        (422, json!("UnplacedPerScope"), json!("jobs")),
        "{}",
        per.body
    );

    w.host.stop();
    let ports = Ports::open(&w.host.state_path()).expect("the state file opens");
    assert_eq!(ReleaseStorage::list(&ports), Vec::new(), "a release");
    assert_eq!(ReleaseStepStorage::list(&ports), Vec::new(), "a step");
    assert_eq!(ComponentStorage::list(&ports), Vec::new(), "a component");
    assert_eq!(WorkloadStorage::list(&ports), Vec::new(), "a workload");
    assert_eq!(
        ComponentWorkloadStorage::list(&ports),
        Vec::new(),
        "an assignment"
    );
}
