//! The node-channel endpoint of the control host (decision A5, `protocols/node-channel.yaml`),
//! served on the listener of the HTTP surfaces under `/node/`. The node agent dials it; the node
//! has no inbound port. No frame of the node carries a time: the endpoint stamps each on receipt
//! from the context clock.
//!
//! `POST /node/hello` with `{"node_id": "...", "agent_version": "..."}`: the endpoint sends
//! `weft.nodes.OpenNodeSession` in this process as the ControlPlane, the one actor granted it,
//! with `received_at` stamped on receipt, through [`Host::execute`] (one transaction, logged as
//! the ControlPlane's request). It answers `200` `{"session_id": "..."}` where the session was
//! opened, and otherwise the command's own status and body (a refusal), after which the node
//! sends `hello` again after the protocol's redial delay.
//!
//! Heartbeats, lease renewal and the loss of a silent session come with the story that runs R3.

use serde_json::{json, Value};
use weft_bin::Host;
use weft_impl::context::timestamp;
use weft_server::http::{self, Request, Response};
use weft_server::weft_control;
use weft_types::actor::{Actor, Caller};

/// The path prefix of the node channel.
pub const PREFIX: &str = "/node/";

/// The command a `hello` is recorded as.
const OPEN: &str = "weft.nodes.OpenNodeSession";

/// The answer to a frame of the node channel.
pub fn answer(host: &Host, request: &Request) -> Response {
    match request.path.as_str() {
        "/node/hello" if request.method == "POST" => hello(host, request),
        "/node/hello" => http::method_not_allowed("POST"),
        other => Response::refusal(
            404,
            &format!("`{other}` is not a frame of the node channel"),
        ),
    }
}

fn hello(host: &Host, request: &Request) -> Response {
    let received_at = timestamp(host.ports().now());
    let frame: Value = match serde_json::from_slice(&request.body) {
        Ok(frame) => frame,
        Err(error) => {
            return Response::refusal(400, &format!("a hello frame is a JSON object: {error}"))
        }
    };
    let (Some(node_id), Some(agent_version)) =
        (frame["node_id"].as_str(), frame["agent_version"].as_str())
    else {
        return Response::refusal(400, "a hello frame names `node_id` and `agent_version`");
    };
    let command = Request {
        method: "POST".to_owned(),
        path: "/weft/commands/OpenNodeSession".to_owned(),
        body: json!({"node_id": node_id, "agent_version": agent_version, "received_at": received_at.0})
            .to_string()
            .into_bytes(),
        ..Request::default()
    };
    let caller = Caller {
        actor: Actor::ControlPlane,
    };
    let answer = host.execute(Some(OPEN), Some(caller.actor.name()), |system| {
        weft_control::dispatch_with_events(system, Some(&caller), &command)
    });
    let body: Value = serde_json::from_str(&answer.body).unwrap_or(Value::Null);
    if body["outcome"] == json!("opened") {
        if let Some(session_id) = body["published"][0]["payload"]["session_id"].as_str() {
            return Response::new(
                200,
                http::JSON,
                json!({"session_id": session_id}).to_string(),
            );
        }
    }
    answer
}
