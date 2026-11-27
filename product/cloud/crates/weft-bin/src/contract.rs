//! The contract the control host serves. Both generated surfaces declare `GET /openapi.json` and
//! `GET /docs`; the host answers both surfaces on one port (decision A3), so it answers those
//! paths itself, with documents that cover every route it serves:
//!
//! - `/openapi.json`: the union of the two OpenAPI documents (paths, schemas, entities, tags);
//!   each surface's own document stays at `/openapi/weft-api.json` and
//!   `/openapi/weft-control.json`;
//! - `/docs`: both surfaces' documentation, one after the other; each alone at
//!   `/docs/weft-api` and `/docs/weft-control`.

use serde_json::{json, Map, Value};
use weft_server::http::{self, Request, Response};
use weft_server::{weft_api, weft_control};

/// The documents the host answers for itself.
pub struct Contract {
    union: String,
    docs: String,
}

impl Contract {
    /// The union of both surfaces' documents.
    ///
    /// # Errors
    ///
    /// A document that is not JSON, or a schema, entity or path the two declare differently.
    pub fn new() -> Result<Self, String> {
        let api: Value = serde_json::from_str(weft_api::OPENAPI)
            .map_err(|error| format!("weft-api contract: {error}"))?;
        let control: Value = serde_json::from_str(weft_control::OPENAPI)
            .map_err(|error| format!("weft-control contract: {error}"))?;
        Ok(Self {
            union: union(&api, &control)?.to_string(),
            docs: format!("{}\n\n{}", weft_api::DOCS, weft_control::DOCS),
        })
    }

    /// The answer to `request` where its path is one the host answers itself.
    pub fn answer(&self, request: &Request) -> Option<Response> {
        let (content_type, body): (&'static str, &str) = match request.path.as_str() {
            "/openapi.json" => (http::JSON, &self.union),
            "/openapi/weft-api.json" => (http::JSON, weft_api::OPENAPI),
            "/openapi/weft-control.json" => (http::JSON, weft_control::OPENAPI),
            "/docs" => (http::MARKDOWN, &self.docs),
            "/docs/weft-api" => (http::MARKDOWN, weft_api::DOCS),
            "/docs/weft-control" => (http::MARKDOWN, weft_control::DOCS),
            _ => return None,
        };
        if request.method != "GET" {
            return Some(http::method_not_allowed("GET"));
        }
        Some(Response::new(200, content_type, body))
    }
}

/// One OpenAPI document holding both: every path, schema and entity of each (a name both declare
/// must be declared alike), the tags of both, and an `info` naming the two it joins.
fn union(api: &Value, control: &Value) -> Result<Value, String> {
    let mut joined = api.clone();
    joined["info"] = json!({
        "title": "weft control host",
        "version": api["info"]["version"],
        "summary": "The union of the contracts of weft-api and weft-control, which the control host serves on one port.",
        "description": "Every path of `weft-api` and of `weft-control`. Each component's own document is served at `/openapi/weft-api.json` and `/openapi/weft-control.json`; their `info` is under `x-weft-components`.",
        "x-weft-components": [api["info"], control["info"]],
    });
    merge(&mut joined, control, "paths")?;
    merge(&mut joined["components"], &control["components"], "schemas")?;
    merge(&mut joined, control, "x-ess-entities")?;
    let mut tags = api["tags"].as_array().cloned().unwrap_or_default();
    for tag in control["tags"].as_array().into_iter().flatten() {
        if !tags.iter().any(|held| held["name"] == tag["name"]) {
            tags.push(tag.clone());
        }
    }
    joined["tags"] = Value::Array(tags);
    Ok(joined)
}

/// Adds every member of `from[key]` to `into[key]`; a member both hold must be equal.
fn merge(into: &mut Value, from: &Value, key: &str) -> Result<(), String> {
    let Some(added) = from[key].as_object() else {
        return Ok(());
    };
    if !into[key].is_object() {
        into[key] = Value::Object(Map::new());
    }
    let held = into[key].as_object_mut().expect("an object");
    for (name, value) in added {
        match held.get(name) {
            Some(other) if other != value => {
                return Err(format!(
                    "weft-api and weft-control declare {key} `{name}` differently"
                ))
            }
            Some(_) => {}
            None => {
                held.insert(name.clone(), value.clone());
            }
        }
    }
    Ok(())
}
