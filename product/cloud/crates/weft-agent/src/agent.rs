//! The node agent's run (decision A5, `protocols/node-channel.yaml`): it dials the control host
//! over HTTP with the join token `ProvisionPoolNode` issued; the node has no inbound port.
//!
//! 1. It asks its `NodeFacts` port for its capacity and the confinement the node proves, which it
//!    reports as its isolation facts (`weft_platform::node`).
//! 2. `weft.infrastructure.JoinNode` as the NodeAgent with its node id and join token (`joined`,
//!    or `token-used` when it was joined before and restarts).
//! 3. `weft.infrastructure.RecordNodeObservation` as the NodeAgent with its capacity and isolation
//!    facts, observed now on its own clock (`ready` the first time, `observed` after).
//! 4. A `hello` frame on the node channel (`POST /node/hello`); its answer is a line of the
//!    report file. As the protocol model says (`protocols/node-channel.yaml`): a session opens
//!    (`session-welcomed`); a refusal ends the agent with a failure (`session-refused`, the node
//!    closes); a lost connection, no answer at all, is dialed again after the redial delay,
//!    3000 ms (`lose-transport-dialing`, `redial`).
//!
//! Then it holds its session until it is stopped. Heartbeats, lease renewal and the work list come
//! with later stories.

use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use weft_client::{Addresses, Answer, Client};
use weft_platform::node::{Capacity, Exec, Isolation, NodeFacts};

use crate::report::Report;

/// The node agent's actor.
pub const NODE_AGENT: &str = "weft.actors.NodeAgent";

/// The version this agent reports in its `hello`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The redial delay of the protocol model (`protocols/node-channel.yaml`, `redial`).
pub const REDIAL: Duration = Duration::from_millis(3000);

/// What the agent is started with.
#[derive(Debug, Clone)]
pub struct Options {
    /// Every address the control URL resolves to, in the resolver's order; the agent dials them
    /// in that order (`--control http://<host>:<port>`).
    pub control: Addresses,
    /// The node this agent runs (`--node-id`).
    pub node_id: String,
    /// The join token `ProvisionPoolNode` issued for it (`--join-token`).
    pub join_token: String,
    /// The substrate daemon's Unix socket (`--substrate`).
    pub substrate: PathBuf,
    /// The report file (`--report`).
    pub report: PathBuf,
}

/// The usage line.
pub const USAGE: &str =
    "usage: weft-agent --control http://<host>:<port> --node-id <id> --join-token <token> --substrate <socket> --report <path>";

impl Options {
    /// The options `args` (without the program name) give.
    ///
    /// # Errors
    ///
    /// An unknown argument, a missing value, or a control URL that names no address.
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let (mut control, mut node_id, mut join_token, mut substrate, mut report) =
            (None, None, None, None, None);
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value"));
            match arg.as_str() {
                "--control" => control = Some(address(&value()?)?),
                "--node-id" => node_id = Some(value()?),
                "--join-token" => join_token = Some(value()?),
                "--substrate" => substrate = Some(PathBuf::from(value()?)),
                "--report" => report = Some(PathBuf::from(value()?)),
                other => return Err(format!("unknown argument {other}")),
            }
        }
        let missing = |name: &str| format!("{name} is required");
        Ok(Self {
            control: control.ok_or_else(|| missing("--control"))?,
            node_id: node_id.ok_or_else(|| missing("--node-id"))?,
            join_token: join_token.ok_or_else(|| missing("--join-token"))?,
            substrate: substrate.ok_or_else(|| missing("--substrate"))?,
            report: report.ok_or_else(|| missing("--report"))?,
        })
    }
}

/// Every address the control URL `http://<host>:<port>[/]` resolves to, in order.
fn address(url: &str) -> Result<Addresses, String> {
    let authority = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("--control {url}: not an http:// URL"))?
        .trim_end_matches('/');
    let addresses = authority
        .to_socket_addrs()
        .map_err(|error| format!("--control {url}: {error}"))?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err(format!("--control {url} names no address"));
    }
    Ok(Addresses::from(addresses))
}

/// Runs the agent: join, observe, open the session, then hold it. Returns only on a failure.
/// It reaches the node through its ports only: `node` for what it observes about itself, and
/// `_exec` for the processes of its instances, which it starts with a later story.
///
/// # Errors
///
/// The node facts are unavailable, the join or the observation is refused, the control host
/// cannot be reached for them, or the report file cannot be written.
pub fn run(options: &Options, _exec: &dyn Exec, node: &dyn NodeFacts) -> Result<(), String> {
    let facts = node
        .observe()
        .map_err(|error| format!("node facts: {error}"))?;
    let mut report = Report::open(&options.report)
        .map_err(|error| format!("report {}: {error}", options.report.display()))?;
    let client = Client::new(options.control.clone());

    let joined = command(
        &client,
        "weft.infrastructure.JoinNode",
        json!({"node_id": options.node_id, "join_token": options.join_token}),
    )?;
    expect(&joined, "JoinNode", &["joined", "token-used"])?;

    let observed = command(
        &client,
        "weft.infrastructure.RecordNodeObservation",
        json!({
            "node_id": options.node_id,
            "capacity": capacity(&facts.capacity),
            "isolation_facts": isolation_facts(&facts.isolation),
            "observed_at": now(),
        }),
    )?;
    expect(&observed, "RecordNodeObservation", &["ready", "observed"])?;

    let hello = json!({"node_id": options.node_id, "agent_version": VERSION});
    loop {
        match client.post("/node/hello", &hello) {
            Ok(answer) if answer.status == 200 => {
                write(
                    &mut report,
                    &json!({"hello": "opened", "session_id": answer.body["session_id"]}),
                )?;
                break;
            }
            Ok(answer) => {
                write(
                    &mut report,
                    &json!({"hello": "refused", "status": answer.status}),
                )?;
                return Err(format!("hello refused: {} {}", answer.status, answer.body));
            }
            Err(error) => {
                eprintln!("weft-agent: hello: the connection was lost, dialing again in {REDIAL:?}: {error}");
                std::thread::sleep(REDIAL);
            }
        }
    }
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}

fn write(report: &mut Report, line: &Value) -> Result<(), String> {
    report
        .line(line)
        .map_err(|error| format!("report: {error}"))
}

fn command(client: &Client, command: &str, input: Value) -> Result<Answer, String> {
    client
        .command(command, NODE_AGENT, json!({}), input)
        .map_err(|error| format!("{command}: the control host did not answer: {error}"))
}

fn expect(answer: &Answer, command: &str, outcomes: &[&str]) -> Result<(), String> {
    match answer.body["outcome"].as_str() {
        Some(outcome) if outcomes.contains(&outcome) => Ok(()),
        _ => Err(format!(
            "{command} answered {}: {}",
            answer.status, answer.body
        )),
    }
}

fn now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("an instant formats in RFC 3339")
}

/// The node's capacity as `RecordNodeObservation` takes it (`weft.shared.ResourceAmount`).
fn capacity(capacity: &[Capacity]) -> Value {
    capacity
        .iter()
        .map(|amount| json!({"resource_class": amount.resource_class, "amount": amount.amount}))
        .collect()
}

/// The isolation facts as `RecordNodeObservation` takes them (`weft.shared.IsolationFacts`).
/// Each of its facts is a required Boolean there: a fact the port answers absent is reported
/// `false`, a capability the node does not claim (default deny).
fn isolation_facts(isolation: &Isolation) -> Value {
    let claimed = |fact: Option<bool>| fact.unwrap_or(false);
    let levels: Vec<&str> = isolation.levels.iter().map(|level| level.name()).collect();
    json!({
        "kvm_present": claimed(isolation.kvm_present),
        "cgroups_delegated": claimed(isolation.cgroups_delegated),
        "egress_denied": claimed(isolation.egress_denied),
        "gpu_passthrough": claimed(isolation.gpu_passthrough),
        "levels": levels,
    })
}
