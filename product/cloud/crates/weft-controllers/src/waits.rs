//! The waits of a step's subjects, kept in the state file (table `weft.impl.StepWait`) and
//! written in the transaction of the tick that sent the commands, so a wait outlives a restart
//! and never outlives the tick's rows.
//!
//! A subject whose command failed is attempted again after a wait that doubles, from one tick
//! up to [`weft_impl::bindings::BACKOFF_CAP`] ticks, as a held event is
//! ([`weft_impl::bindings::wait_after`]): a subject that never goes through sends about one
//! command per 32 ticks, so the log grows by one request line per attempt, not per tick. A
//! subject that went through, or left the step's view, has no wait.

use std::collections::HashMap;

use weft_impl::bindings::wait_after;
use weft_impl::storage::Store;

/// The waits of one step, for one tick.
pub struct Waits<'a> {
    store: &'a Store,
    step: &'static str,
    /// Per subject: the attempts that failed in a row and the ticks until the next.
    rows: HashMap<String, (u32, u32)>,
}

impl<'a> Waits<'a> {
    /// The waits of `step` in `store`, given the subjects of its view this tick: the wait of a
    /// subject no longer among them is let go.
    pub fn load(store: &'a Store, step: &'static str, subjects: &[String]) -> Self {
        let mut rows = HashMap::new();
        for wait in store.step_waits(step) {
            if subjects.contains(&wait.subject) {
                rows.insert(wait.subject, (wait.attempts, wait.wait));
            } else {
                store.clear_step_wait(step, &wait.subject);
            }
        }
        Self { store, step, rows }
    }

    /// Whether `subject` is attempted this tick: it has no wait, or its wait ends now. A subject
    /// that waits on waits one tick less.
    pub fn due(&mut self, subject: &str) -> bool {
        let Some((attempts, wait)) = self.rows.get(subject).copied() else {
            return true;
        };
        if wait > 1 {
            self.rows.insert(subject.to_owned(), (attempts, wait - 1));
            self.store
                .set_step_wait(self.step, subject, attempts, wait - 1);
            return false;
        }
        true
    }

    /// `subject`'s attempt failed: it waits [`wait_after`] its failed attempts.
    pub fn failed(&mut self, subject: &str) {
        let attempts = self.rows.get(subject).map_or(0, |(attempts, _)| *attempts) + 1;
        let wait = wait_after(attempts);
        self.rows.insert(subject.to_owned(), (attempts, wait));
        self.store.set_step_wait(self.step, subject, attempts, wait);
    }

    /// `subject`'s attempt went through: it has no wait.
    pub fn succeeded(&mut self, subject: &str) {
        if self.rows.remove(subject).is_some() {
            self.store.clear_step_wait(self.step, subject);
        }
    }
}
