//! What synthesis leaves to weft, written by hand: the ports of the generated behaviours and the
//! obligations on the skeleton's path (`docs/implementation.md`, decisions A2-A4).
//!
//! - [`Ports`]: the storage port on SQLite ([`storage`], one table per entity, one database file)
//!   and the context port ([`context`]: the clock, identities minted as UUIDv4, the `external:`
//!   decider, the caller's attributes). One `Ports` is shared by the three components of the
//!   [`System`]; clones share one store and one context.
//! - The obligations of `src/behaviours/`, one file each; every other obligation answers the
//!   generated stub, `UnmetObligation` (`build.rs`).
//! - [`log`]: the control host log, one JSON line per answered request, published event and
//!   binding answer.
//! - [`bindings`]: the bindings of the path delivered by hand, with their per-refusal policies.
//! - [`quota`]: the `over-quota` decision of the outcome group `quota`.
//! - [`conversions`]: the conversions of `ess/components.yaml` the path needs.
//! - [`transformations`]: the binding transformations the generated system owes.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

pub mod bindings;
pub mod context;
pub mod conversions;
pub mod documents;
pub mod log;
pub mod placement;
pub mod quota;
pub mod storage;
pub mod transformations;

/// What `build.rs` writes from the generated code: the storage port, the `Row` enum, the codecs
/// and the stub of every obligation without a file of `src/behaviours/`. Its text follows from
/// the specification, so clippy does not lint it (`tools/check.d/20-rust-lint.sh`); the
/// hand-written behaviours are included outside this module and stay linted.
#[allow(clippy::all)]
mod generated {
    use crate::Ports;

    include!(concat!(env!("OUT_DIR"), "/weft_impl_generated.rs"));
}

pub use generated::{Row, ENTITIES};

include!(concat!(env!("OUT_DIR"), "/weft_impl_behaviours.rs"));

/// The generated behaviours over weft's ports: the behaviour bundle of every component.
pub type Behaviours = weft_types::behaviour::Generated<Ports>;

/// The `weft` system with weft's ports behind every component.
pub type System =
    weft_system::System<Behaviours, Behaviours, Behaviours, transformations::Transformations>;

/// The storage and context ports. Clones share the same store and context.
#[derive(Clone)]
pub struct Ports {
    shared: Arc<Shared>,
}

struct Shared {
    store: storage::Store,
    context: Mutex<context::State>,
}

impl Ports {
    /// Ports over the state file at `state` (created when absent), with the system clock.
    ///
    /// # Errors
    ///
    /// What SQLite refuses while opening the file or creating its tables.
    pub fn open(state: &Path) -> Result<Self, storage::Error> {
        Self::open_with_clock(state, context::Clock::system())
    }

    /// Ports over the state file at `state` with the clock `clock`.
    ///
    /// # Errors
    ///
    /// As [`Ports::open`].
    pub fn open_with_clock(state: &Path, clock: context::Clock) -> Result<Self, storage::Error> {
        Ok(Self {
            shared: Arc::new(Shared {
                store: storage::Store::open(state)?,
                context: Mutex::new(context::State::new(clock)),
            }),
        })
    }

    /// The store behind the storage port.
    pub fn store(&self) -> &storage::Store {
        &self.shared.store
    }

    /// Sets the attributes of the caller of the command about to run; the next call replaces
    /// them.
    pub fn set_caller(&self, caller: context::CallerAttributes) {
        self.context().caller = caller;
    }

    fn context(&self) -> MutexGuard<'_, context::State> {
        self.shared
            .context
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// The system over `ports`: every component's behaviours are the generated ones over a clone of
/// `ports`, and the binding transformations the system owes are [`transformations::Transformations`].
pub fn system(ports: &Ports) -> System {
    let behaviours = || weft_types::behaviour::Generated::new(ports.clone());
    weft_system::System::new(
        weft_api::WeftApi::new(behaviours()),
        weft_control::WeftControl::new(behaviours()),
        weft_edge::WeftEdge::new(behaviours()),
        transformations::Transformations,
    )
}
