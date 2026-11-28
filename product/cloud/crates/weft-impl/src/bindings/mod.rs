//! The bindings weft delivers by hand. `tools/synth-input.sh` removes the bindings of the
//! skeleton's path that carry a per-refusal policy from the synthesis input, because the
//! generated [`crate::System`] has no seam for one: its delivery is private, a declared refusal
//! goes to a private held list, and its invocations record is dropped by every request. This
//! runner delivers them as `ess/components.yaml` declares them:
//!
//! - it reads the events a request or a step published ([`Runner::deliver`]), and maps each
//!   event of a hand-run binding to its command, its `where:` filter included;
//! - it invokes the command in this process through the generated method of the accepting
//!   component (`WeftControl::create_instance`), without an actor, as the generated delivery
//!   does, and pumps the system so that what the invocation published is on its log;
//! - it applies the binding's declared policy: a refusal in its `drop:` list is dropped; any
//!   other refusal, and an unmet obligation (the transport failure of this process), is held
//!   and attempted again by [`Runner::retry`];
//! - the events an invocation published go through the runner again.
//!
//! Held events live in the state file (table `weft.impl.Held`), written in the transaction of
//! the unit of work that held them, so a held event outlives a restart (`delivery:
//! at_least_once`) and never outlives the rows it came from. Each [`Runner::retry`] is one tick:
//! a held event is attempted again once its wait is over. The wait doubles with each attempt,
//! from one tick up to [`BACKOFF_CAP`] ticks ([`wait_after`]), so a held event that never goes
//! through writes about one log line per [`BACKOFF_CAP`] ticks. A runner that opens makes every
//! held event due at its first tick.
//!
//! Each attempt is one [`Answer`], with its number, which the control host logs as a binding
//! line ([`crate::log::Log::binding`]) followed by the lines of the events it published.
//!
//! Entries, each with the ESS story whose fix removes it (with its line in
//! `tools/synth-input.sh`, in one change):
//!
//! - [`r1_create_replica`], ESS `story:rust-binding-wraps-a-plain-copy-into-an-optional-member`
//!   (E3);
//! - [`r1_submit_scheduling_request`], ESS
//!   `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//!   (E2);
//! - [`r4_bind_placement`], [`r4_assign`], [`r4_schedule_instance`] and
//!   [`r4_record_placement_assigned`], ESS
//!   `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//!   (E2);
//! - [`edge_backend_ready`] and [`edge_backend_restore`], ESS
//!   `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//!   (E2).

use std::collections::VecDeque;
use std::fmt::Debug;

use serde_json::Value;
use weft_system::SystemEvent;

use crate::log::BindingResult;
use crate::{Ports, System};

pub mod edge_backend_ready;
pub mod edge_backend_restore;
pub mod r1_create_replica;
pub mod r1_submit_scheduling_request;
pub mod r4_assign;
pub mod r4_bind_placement;
pub mod r4_record_placement_assigned;
pub mod r4_schedule_instance;

/// The longest wait between two attempts of a held event, in ticks.
pub const BACKOFF_CAP: u32 = 32;

/// One binding delivered by hand.
pub struct Entry {
    /// The binding's `id` in `ess/components.yaml`.
    pub id: &'static str,
    /// The refusals of its `on_failure.drop` list.
    pub drop: &'static [&'static str],
    /// The binding's delivery of `event`: `None` where the binding does not react to it (another
    /// event, or its `where:` filter does not hold), else what the invoked command answered.
    pub invoke: fn(&mut System, &SystemEvent) -> Option<Invoked>,
    /// The event the binding reacts to in the form of the state file
    /// ([`crate::storage::codec`]); `None` for another event.
    pub encode: fn(&SystemEvent) -> Option<Value>,
    /// The event written by `encode`.
    pub decode: fn(&Value) -> Result<SystemEvent, String>,
}

/// What an invoked command answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invoked {
    /// The command answered its accepting outcome; the events it published, in log order.
    Accepted(Vec<SystemEvent>),
    /// The command answered the declared refusal of this name.
    Refused(String),
    /// The command's port answered an unmet obligation: nothing was written.
    Unmet(String),
}

/// The hand-run bindings, in the order of `ess/components.yaml`.
pub const ENTRIES: &[&Entry] = &[
    &r1_create_replica::ENTRY,
    &r1_submit_scheduling_request::ENTRY,
    &r4_bind_placement::ENTRY,
    &r4_assign::ENTRY,
    &r4_schedule_instance::ENTRY,
    &r4_record_placement_assigned::ENTRY,
    &edge_backend_ready::ENTRY,
    &edge_backend_restore::ENTRY,
];

/// One attempt of a binding: the line the control host logs for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The binding's `id`.
    pub binding: &'static str,
    /// The refusal that stopped it; `None` where the command accepted, or its port answered an
    /// unmet obligation.
    pub refusal: Option<String>,
    /// What became of the event.
    pub result: BindingResult,
    /// The attempt's number for its event, from 1.
    pub attempt: u32,
    /// What the invoked command published, in log order; empty unless `Applied`.
    pub events: Vec<SystemEvent>,
}

/// The ticks a held event waits after its attempt number `attempts`: 1, 2, 4, ... up to
/// [`BACKOFF_CAP`].
pub fn wait_after(attempts: u32) -> u32 {
    let doubled = 1u32
        .checked_shl(attempts.saturating_sub(1))
        .unwrap_or(u32::MAX);
    doubled.min(BACKOFF_CAP)
}

/// The runner over the state file behind its ports, where it holds events for another
/// attempt.
pub struct Runner {
    ports: Ports,
    entries: &'static [&'static Entry],
}

impl Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id)
    }
}

impl Runner {
    /// The runner of [`ENTRIES`] over the state file behind `ports`: every event held there is
    /// due at the next tick.
    pub fn open(ports: &Ports) -> Self {
        Self::open_with(ports, ENTRIES)
    }

    /// As [`Runner::open`], with the bindings `entries`.
    pub fn open_with(ports: &Ports, entries: &'static [&'static Entry]) -> Self {
        ports.store().make_held_due();
        Self {
            ports: ports.clone(),
            entries,
        }
    }

    /// How many events its bindings hold for another attempt.
    pub fn held(&self) -> usize {
        self.ports.store().held().len()
    }

    /// The events its bindings hold for another attempt, in the order held. An event of a
    /// binding the runner does not run stays held and is not answered.
    pub fn held_events(&self) -> Vec<SystemEvent> {
        self.ports
            .store()
            .held()
            .iter()
            .filter_map(|held| {
                let entry = self.entry(&held.binding)?;
                Some(decode(entry, &held.event))
            })
            .collect()
    }

    /// Delivers `events` to every hand-run binding that reacts to them, and what each invocation
    /// published in turn: every attempt, in order. An event a binding retries is held.
    pub fn deliver(&self, system: &mut System, events: Vec<SystemEvent>) -> Vec<Answer> {
        let mut queue: VecDeque<SystemEvent> = events.into();
        let mut answers = Vec::new();
        while let Some(event) = queue.pop_front() {
            for entry in self.entries {
                if let Some(answer) = attempt(system, entry, &event, 1) {
                    if answer.result == BindingResult::Retried {
                        let encoded = (entry.encode)(&event).unwrap_or_else(|| {
                            panic!("{entry:?} reacted to an event it cannot encode: {event:?}")
                        });
                        self.ports
                            .store()
                            .hold(entry.id, &encoded.to_string(), 1, wait_after(1));
                    }
                    queue.extend(answer.events.iter().cloned());
                    answers.push(answer);
                }
            }
        }
        answers
    }

    /// One tick of the held events, in the order held: an event whose wait is over is attempted
    /// again, and what its invocation published is delivered; every other event waits one tick
    /// less. An event retried again waits [`wait_after`] its attempts; any other answer lets it
    /// go. Every attempt, in order.
    pub fn retry(&self, system: &mut System) -> Vec<Answer> {
        let store = self.ports.store();
        let mut answers = Vec::new();
        for held in store.held() {
            let Some(entry) = self.entry(&held.binding) else {
                continue;
            };
            if held.wait > 1 {
                store.set_held(held.id, held.attempts, held.wait - 1);
                continue;
            }
            let event = decode(entry, &held.event);
            let number = held.attempts + 1;
            let Some(answer) = attempt(system, entry, &event, number) else {
                store.release(held.id);
                continue;
            };
            if answer.result == BindingResult::Retried {
                store.set_held(held.id, number, wait_after(number));
            } else {
                store.release(held.id);
            }
            let published = answer.events.clone();
            answers.push(answer);
            answers.extend(self.deliver(system, published));
        }
        answers
    }

    fn entry(&self, id: &str) -> Option<&'static Entry> {
        self.entries.iter().copied().find(|entry| entry.id == id)
    }
}

/// The held event `text` of `entry`.
///
/// # Panics
///
/// When it does not decode: the state file holds what no runner wrote.
fn decode(entry: &Entry, text: &str) -> SystemEvent {
    serde_json::from_str::<Value>(text)
        .map_err(|error| error.to_string())
        .and_then(|value| (entry.decode)(&value))
        .unwrap_or_else(|error| {
            panic!("state file: a held event of {entry:?} does not decode: {error}")
        })
}

/// Attempt `number` of `entry` on `event`, with its policy applied.
fn attempt(
    system: &mut System,
    entry: &'static Entry,
    event: &SystemEvent,
    number: u32,
) -> Option<Answer> {
    let invoked = (entry.invoke)(system, event)?;
    let (refusal, result, events) = match invoked {
        Invoked::Accepted(events) => (None, BindingResult::Applied, events),
        Invoked::Refused(refusal) if entry.drop.contains(&refusal.as_str()) => {
            (Some(refusal), BindingResult::Dropped, Vec::new())
        }
        Invoked::Refused(refusal) => (Some(refusal), BindingResult::Retried, Vec::new()),
        Invoked::Unmet(_) => (None, BindingResult::Retried, Vec::new()),
    };
    Some(Answer {
        binding: entry.id,
        refusal,
        result,
        attempt: number,
        events,
    })
}

/// The declared name of `outcome`, a variant of a generated outcome enum: its variant name in
/// kebab case (`ReplicaLive { .. }` is `replica-live`), as the generator names each variant
/// after its outcome.
pub fn outcome_name(outcome: &impl Debug) -> String {
    let debug = format!("{outcome:?}");
    let variant = debug
        .split(|character: char| !character.is_alphanumeric())
        .next()
        .unwrap_or_default();
    let mut name = String::new();
    for (position, character) in variant.chars().enumerate() {
        if character.is_uppercase() {
            if position > 0 {
                name.push('-');
            }
            name.extend(character.to_lowercase());
        } else {
            name.push(character);
        }
    }
    name
}

/// Runs `call` on a component of the system (`weft-control`, `weft-edge`), then pumps the
/// system: what the call published, in log order, with the call's answer. As the generated
/// routes do, an event already on the log before the call is not the call's.
pub(crate) fn invoke<O>(
    system: &mut System,
    call: impl FnOnce(&mut System) -> Result<O, weft_types::obligation::UnmetObligation>,
) -> Result<(O, Vec<SystemEvent>), String> {
    let before = system.published().len();
    let outcome = call(system).map_err(|unmet| unmet.to_string())?;
    let delivered = system.pump();
    let published: Vec<SystemEvent> = system.take_published().into_iter().skip(before).collect();
    let _ = system.take_invocations();
    // A failed delivery after the call took effect leaves its effect and its events standing,
    // as the generated routes answer it (`committed: true`); the call is still answered.
    let _ = delivered;
    Ok((outcome, published))
}
