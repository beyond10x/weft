//! The cloudspec submission endpoint of the control host, served on the listener of the HTTP
//! surfaces under `/cloudspec/`.
//!
//! `POST /cloudspec/submissions?formation_id=<id>` takes the bytes of a
//! `cloudspec-weft-submission/1` file (cloudspec's `generate weft`) as its body, sent by the
//! caller as every request is ([`weft_bin::authenticate`]):
//!
//! - [`weft_cloudspec::Submission::read`] checks the file offline. A refused file is answered
//!   `422` `{"refusal": "<variant>"}`, logged as a request line with no operation, and nothing
//!   is written.
//! - An accepted file is recorded in one unit ([`Host::unit`]): `weft.deployments.SubmitDeploymentSource`
//!   runs in the caller's scope with kind `Cloudspec`, format version
//!   `cloudspec-weft-submission/1`, the file's source digest and its two member digests, no
//!   translator, no values and no `embedded_in`; where it answers `submitted`, the file's bytes
//!   are kept under its source digest ([`weft_impl::documents`]) in the same transaction. A
//!   refusal of the command keeps no bytes. The endpoint answers what the command answers.
//!
//! `POST /cloudspec/plans/<plan_id>/apply` applies the plan of a cloudspec source and rolls out
//! its snippet parts ([`cloudspec_apply`]).
//!
//! A request the endpoint refuses itself (`400` no `formation_id` or no strategy, `403` a caller
//! acting in no scope, `404` not a path of the endpoint, `405` not `POST`, `422` a refused file
//! or a plan with no source) is logged as a request line too, with no operation, and writes
//! nothing.

use serde_json::json;
use weft_bin::Host;

use crate::cloudspec_apply;
use weft_cloudspec::Submission;
use weft_impl::context::CallerAttributes;
use weft_server::http::{self, Request, Response};
use weft_server::weft_control;
use weft_types::deployments::SourceDigest;

/// The path prefix of the endpoint.
pub const PREFIX: &str = "/cloudspec/";

/// The path of a submission.
const SUBMISSIONS: &str = "/cloudspec/submissions";

/// The command that records a submission.
const SUBMIT: &str = "weft.deployments.SubmitDeploymentSource";

/// The answer to a request under [`PREFIX`], as the caller [`weft_bin::authenticate`] finds with
/// `trust_callers`.
pub fn answer(host: &Host, request: &Request, trust_callers: bool) -> Response {
    let (caller, attributes) = weft_bin::authenticate(request, trust_callers);
    let actor = caller.as_ref().map(|caller| caller.actor.name());
    let refused = match request.path.as_str() {
        SUBMISSIONS if request.method == "POST" => {
            match submit(host, request, caller.as_ref(), attributes) {
                Ok(answer) => return answer,
                Err(refused) => refused,
            }
        }
        SUBMISSIONS => http::method_not_allowed("POST"),
        other => match cloudspec_apply::plan_of(other) {
            Some(plan_id) if request.method == "POST" => {
                match cloudspec_apply::apply(host, request, plan_id, caller.as_ref(), attributes) {
                    Ok(answer) => return answer,
                    Err(refused) => refused,
                }
            }
            Some(_) => http::method_not_allowed("POST"),
            None => Response::refusal(
                404,
                &format!("`{other}` is not a path of the cloudspec endpoint"),
            ),
        },
    };
    host.log().request(None, actor, refused.status);
    refused
}

/// The command's answer to an accepted file, or the endpoint's own refusal.
fn submit(
    host: &Host,
    request: &Request,
    caller: Option<&weft_types::actor::Caller>,
    attributes: CallerAttributes,
) -> Result<Response, Response> {
    let formation_id = query_value(&request.query, "formation_id").ok_or_else(|| {
        Response::refusal(400, "a submission names its formation: ?formation_id=<id>")
    })?;
    let submission = Submission::read(&request.body).map_err(|refusal| {
        Response::new(
            422,
            http::JSON,
            json!({"refusal": refusal.name()}).to_string(),
        )
    })?;
    let scope_id = attributes
        .scope_id
        .clone()
        .ok_or_else(|| Response::refusal(403, "the caller acts in no scope"))?;
    let input = json!({
        "scope_id": scope_id,
        "formation_id": formation_id,
        "kind": "Cloudspec",
        "format_version": weft_cloudspec::FORMAT,
        "source_digest": submission.source_digest(),
        "helm_values": [],
        "architecture_digest": submission.architecture_digest(),
        "binding_digest": submission.binding_digest(),
    });
    let command = Request {
        method: "POST".to_owned(),
        path: "/weft/commands/SubmitDeploymentSource".to_owned(),
        body: input.to_string().into_bytes(),
        ..Request::default()
    };
    let ports = host.ports();
    Ok(host.unit(|unit| {
        let answer = unit.run(
            Some(SUBMIT),
            caller.map(|caller| caller.actor.name()),
            |system| {
                ports.set_caller(attributes);
                let answer = weft_control::dispatch_with_events(system, caller, &command);
                ports.set_caller(CallerAttributes::default());
                answer
            },
        );
        if submitted(&answer) {
            weft_impl::documents::put(
                ports,
                &SourceDigest(submission.source_digest().to_owned()),
                &request.body,
            );
        }
        answer
    }))
}

/// Whether `answer` is the command's `submitted`.
fn submitted(answer: &Response) -> bool {
    serde_json::from_str::<serde_json::Value>(&answer.body)
        .is_ok_and(|body| body["outcome"] == json!("submitted"))
}

/// The value of `key` in the query string `query`, percent-decoded; `None` when absent, empty or
/// not UTF-8.
fn query_value(query: &str, key: &str) -> Option<String> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(name, _)| *name == key)
        .and_then(|(_, value)| decode(value))
        .filter(|value| !value.is_empty())
}

/// `text` with every `%XX` replaced by its byte and `+` by a blank.
fn decode(text: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut rest = text.as_bytes();
    while let Some((&first, tail)) = rest.split_first() {
        match first {
            b'%' => {
                let hex = tail.get(..2)?;
                let value = u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?;
                bytes.push(value);
                rest = &tail[2..];
            }
            b'+' => {
                bytes.push(b' ');
                rest = tail;
            }
            other => {
                bytes.push(other);
                rest = tail;
            }
        }
    }
    String::from_utf8(bytes).ok()
}
