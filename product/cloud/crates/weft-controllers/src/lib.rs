//! The level and time steps of `ess/components.yaml` (bindings block, "Level and time steps")
//! that no binding runs: each reads a gap view through the generated query and sends one
//! command per row to `weft-control` as the actor the step names.
//!
//! [`tick`] runs every step once over a [`weft_impl::System`]; the control host (`weft-bin`)
//! runs it every `--tick-ms` under its one system lock and logs what it sent, and tests call it
//! in-process. A command is sent through the route of `weft-control` that serves it
//! ([`weft_server::weft_control::dispatch_with_events`]), so its answer carries the status the
//! contract declares, which the control host log records.
//!
//! Steps, in the order [`tick`] runs them: [`replicas`] (R1 level), [`per_node`] (one instance
//! per node of a placed scope), [`placement`] (the Scheduler's decision step), [`repair`] (R4
//! level). A step that retries a subject waits between its attempts ([`waits`]).

use serde_json::Value;
use weft_impl::{Ports, System};
use weft_server::http::{Request, Response};
use weft_server::weft_control;
use weft_system::SystemEvent;
use weft_types::actor::{Actor, Caller};

pub mod per_node;
pub mod placement;
pub mod repair;
pub mod replicas;
pub mod waits;

/// What a tick's steps read besides the system's views.
pub struct Context<'a> {
    /// The ports of the system: the controller clock and the state file, where the steps keep
    /// their waits in the tick's transaction.
    pub ports: &'a Ports,
    /// The events the hand-run bindings hold for another attempt
    /// ([`weft_impl::bindings::Runner::held_events`]).
    pub held: &'a [SystemEvent],
}

impl Context<'_> {
    /// The controller clock's instant: the clock of the ports, which the control host is opened
    /// with.
    pub fn now(&self) -> time::OffsetDateTime {
        self.ports.now()
    }
}

/// One command a step sent.
#[derive(Debug)]
pub struct Sent {
    /// The command's qualified name.
    pub operation: &'static str,
    /// The actor it was sent as.
    pub actor: Actor,
    /// Its answer.
    pub answer: Response,
    /// What it published, and what the generated bindings that reacted published in turn.
    pub events: Vec<SystemEvent>,
}

/// Runs every step once over `context`: the commands sent, in order.
pub fn tick(system: &mut System, context: &Context<'_>) -> Vec<Sent> {
    let mut sent = replicas::tick(system, context.held);
    sent.extend(per_node::tick(system, context));
    sent.extend(placement::tick(system, context));
    sent.extend(repair::tick(system, context));
    sent
}

/// The outcome `sent` answered: the `outcome` member of its body; `None` for an answer without
/// one (an unmet obligation, a refused caller).
pub fn outcome(sent: &Sent) -> Option<String> {
    serde_json::from_str::<Value>(&sent.answer.body)
        .ok()?
        .get("outcome")?
        .as_str()
        .map(str::to_owned)
}

/// Sends `operation` with `input` to `weft-control` as `actor`.
///
/// # Panics
///
/// When `weft-control` serves no route for `operation`: a step names a command of another
/// component.
pub fn send(system: &mut System, operation: &'static str, actor: Actor, input: &Value) -> Sent {
    let path = path_of(operation)
        .unwrap_or_else(|| panic!("weft-control serves no route for {operation}"));
    let request = Request {
        method: "POST".to_owned(),
        path,
        body: input.to_string().into_bytes(),
        ..Request::default()
    };
    let (answer, events) =
        weft_control::dispatch_with_events(system, Some(&Caller { actor }), &request);
    Sent {
        operation,
        actor,
        answer,
        events,
    }
}

/// The path of the route of `weft-control` that serves `operation`, read from its startup
/// record.
fn path_of(operation: &str) -> Option<String> {
    weft_control::STARTUP.iter().find_map(|line| {
        let record = serde_json::from_str::<Value>(&format!("{line}}}")).ok()?;
        record["paths"]
            .as_array()?
            .iter()
            .find(|path| path["name"] == operation && path["serves"] == "command")
            .and_then(|path| path["path"].as_str())
            .map(str::to_owned)
    })
}
