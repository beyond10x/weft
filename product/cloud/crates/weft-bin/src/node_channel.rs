//! The node-channel endpoint of the control host (decision A5, `protocols/node-channel.yaml`),
//! served on the listener of the HTTP surfaces under `/node/`. The node agent dials it; the node
//! has no inbound port. No frame of the node carries a time: the endpoint stamps each on receipt
//! from the context clock.
//!
//! `POST /node/hello` with `{"node_id": "...", "agent_version": "..."}` is recorded as commands
//! sent in this process as the ControlPlane, the one actor granted them, with `received_at`
//! stamped on receipt (`ess/5-runtime/nodes.yaml`):
//!
//! - where the node still holds an `Open` session (`replace-open-session`: the agent restarted),
//!   `weft.nodes.SupersedeNodeSession` ends it first;
//! - then `weft.nodes.OpenNodeSession` opens the new one.
//!
//! The commands of one `hello` are one unit ([`Host::unit`]): one transaction, each command's
//! request line in the log. The protocol model takes a `hello` as one step of control
//! (`replace-open-session` and a first dial both go to `vetting`, then `welcome` or `refused`),
//! so the unit commits only whole: a supersede whose `OpenNodeSession` does not open a session
//! is rolled back with it, and the node keeps the session it had. The endpoint answers `200`
//! `{"session_id": "..."}` where a session was opened, and otherwise the refusing command's own
//! status and body; on a refusal the node stops (`session-refused`).
//!
//! A frame the endpoint refuses itself (`400` not a hello frame, `404` not a frame of the
//! channel, `405` not `POST`) is logged as a request line too, with no operation and no actor.
//!
//! Heartbeats, lease renewal and the loss of a silent session come with the story that runs R3.

use serde_json::{json, Value};
use weft_bin::{Host, Unit};
use weft_impl::context::timestamp;
use weft_server::http::{self, Request, Response};
use weft_server::weft_control;
use weft_types::actor::{Actor, Caller};
use weft_types::behaviour::NodeSessionStorage;
use weft_types::nodes::NodeSessionState;
use weft_types::primitives::Uuid;
use weft_types::shared::NodeId;

/// The path prefix of the node channel.
pub const PREFIX: &str = "/node/";

/// The command that opens the session of a `hello`.
const OPEN: &str = "weft.nodes.OpenNodeSession";

/// The command that ends a node's still-`Open` session on its `hello`.
const SUPERSEDE: &str = "weft.nodes.SupersedeNodeSession";

/// The caller of every command of the node channel.
const CONTROL_PLANE: Caller = Caller {
    actor: Actor::ControlPlane,
};

/// The answer to a frame of the node channel.
pub fn answer(host: &Host, request: &Request) -> Response {
    let refused = match request.path.as_str() {
        "/node/hello" if request.method == "POST" => match hello(host, request) {
            Ok(answer) => return answer,
            Err(refused) => refused,
        },
        "/node/hello" => http::method_not_allowed("POST"),
        other => Response::refusal(
            404,
            &format!("`{other}` is not a frame of the node channel"),
        ),
    };
    host.log().request(None, None, refused.status);
    refused
}

/// The answer to a `hello`, or the endpoint's own refusal of a frame that is not one.
fn hello(host: &Host, request: &Request) -> Result<Response, Response> {
    let received_at = timestamp(host.ports().now());
    let frame: Value = serde_json::from_slice(&request.body).map_err(|error| {
        Response::refusal(400, &format!("a hello frame is a JSON object: {error}"))
    })?;
    let (Some(node_id), Some(agent_version)) =
        (frame["node_id"].as_str(), frame["agent_version"].as_str())
    else {
        return Err(Response::refusal(
            400,
            "a hello frame names `node_id` and `agent_version`",
        ));
    };
    Ok(host.unit(|unit| {
        let node = NodeId(Uuid(node_id.to_owned()));
        let open: Vec<String> = NodeSessionStorage::list(host.ports())
            .into_iter()
            .filter(|session| session.data.node_id == node && session.state == NodeSessionState::Open)
            .map(|session| session.data.session_id.0 .0)
            .collect();
        let superseded = !open.is_empty();
        for session_id in open {
            let answer = run(unit, SUPERSEDE, json!({"session_id": session_id, "node_id": node_id}));
            if outcome(&answer).as_deref() != Some("superseded") {
                unit.abort();
                return answer;
            }
        }
        let answer = run(
            unit,
            OPEN,
            json!({"node_id": node_id, "agent_version": agent_version, "received_at": received_at.0}),
        );
        let body: Value = serde_json::from_str(&answer.body).unwrap_or(Value::Null);
        match body["published"][0]["payload"]["session_id"].as_str() {
            Some(session_id) if body["outcome"] == json!("opened") => {
                Response::new(200, http::JSON, json!({"session_id": session_id}).to_string())
            }
            _ => {
                if superseded {
                    unit.abort();
                }
                answer
            }
        }
    }))
}

/// Sends `command` (a qualified name) with `input` as the ControlPlane, in `unit`.
fn run(unit: &mut Unit<'_>, command: &str, input: Value) -> Response {
    let short = command.rsplit('.').next().unwrap_or(command);
    let request = Request {
        method: "POST".to_owned(),
        path: format!("/weft/commands/{short}"),
        body: input.to_string().into_bytes(),
        ..Request::default()
    };
    unit.run(Some(command), Some(CONTROL_PLANE.actor.name()), |system| {
        weft_control::dispatch_with_events(system, Some(&CONTROL_PLANE), &request)
    })
}

fn outcome(answer: &Response) -> Option<String> {
    serde_json::from_str::<Value>(&answer.body)
        .ok()
        .and_then(|body| body["outcome"].as_str().map(str::to_owned))
}
