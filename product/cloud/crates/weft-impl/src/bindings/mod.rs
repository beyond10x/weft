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
//! Each attempt is one [`Answer`], which the control host logs as a binding line
//! ([`crate::log::Log::binding`]) followed by the lines of the events it published.
//!
//! Entries, each with the ESS story whose fix removes it (with its line in
//! `tools/synth-input.sh`, in one change):
//!
//! - [`r1_create_replica`], ESS `story:rust-binding-wraps-a-plain-copy-into-an-optional-member`
//!   (E3);
//! - [`r1_submit_scheduling_request`], ESS
//!   `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//!   (E2).

use std::collections::VecDeque;
use std::fmt::Debug;

use weft_system::SystemEvent;

use crate::log::BindingResult;
use crate::System;

pub mod r1_create_replica;
pub mod r1_submit_scheduling_request;

/// One binding delivered by hand.
pub struct Entry {
    /// The binding's `id` in `ess/components.yaml`.
    pub id: &'static str,
    /// The refusals of its `on_failure.drop` list.
    pub drop: &'static [&'static str],
    /// The binding's delivery of `event`: `None` where the binding does not react to it (another
    /// event, or its `where:` filter does not hold), else what the invoked command answered.
    pub invoke: fn(&mut System, &SystemEvent) -> Option<Invoked>,
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
    /// What the invoked command published, in log order; empty unless `Applied`.
    pub events: Vec<SystemEvent>,
}

/// An event a binding holds for another attempt.
#[derive(Debug, Clone)]
struct Held {
    entry: &'static Entry,
    event: SystemEvent,
}

/// The runner: the events its bindings hold for another attempt. Cloned before a unit of work
/// and put back when the unit rolls back, so a held event never outlives the rows it came from.
#[derive(Debug, Clone, Default)]
pub struct Runner {
    held: Vec<Held>,
}

impl Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id)
    }
}

impl Runner {
    /// A runner that holds nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many events its bindings hold for another attempt.
    pub fn held(&self) -> usize {
        self.held.len()
    }

    /// Delivers `events` to every hand-run binding that reacts to them, and what each invocation
    /// published in turn: every attempt, in order.
    pub fn deliver(&mut self, system: &mut System, events: Vec<SystemEvent>) -> Vec<Answer> {
        let mut queue: VecDeque<SystemEvent> = events.into();
        let mut answers = Vec::new();
        while let Some(event) = queue.pop_front() {
            for entry in ENTRIES {
                if let Some(answer) = self.attempt(system, entry, &event) {
                    queue.extend(answer.events.iter().cloned());
                    answers.push(answer);
                }
            }
        }
        answers
    }

    /// Attempts every held event again, in the order it was held, and delivers what each
    /// invocation published: every attempt, in order. An event refused again is held again.
    pub fn retry(&mut self, system: &mut System) -> Vec<Answer> {
        let mut answers = Vec::new();
        for held in std::mem::take(&mut self.held) {
            if let Some(answer) = self.attempt(system, held.entry, &held.event) {
                let published = answer.events.clone();
                answers.push(answer);
                answers.extend(self.deliver(system, published));
            }
        }
        answers
    }

    /// One attempt of `entry` on `event`, with its policy applied.
    fn attempt(
        &mut self,
        system: &mut System,
        entry: &'static Entry,
        event: &SystemEvent,
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
        if result == BindingResult::Retried {
            self.held.push(Held {
                entry,
                event: event.clone(),
            });
        }
        Some(Answer {
            binding: entry.id,
            refusal,
            result,
            events,
        })
    }
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

/// Runs `call` on the system's `weft-control` port, then pumps the system: what the call
/// published, in log order, with the call's answer. As the generated routes do, an event
/// already on the log before the call is not the call's.
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
