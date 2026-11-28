//! weft-platform: the ports through which weft reaches the place it runs, and their adapters
//! (`docs/platform-ports.md`). [`node`] holds the node-layer ports, [`local`] the adapters of
//! the placement `local`, the only code that names substrate's SDK.

use std::fmt;

pub mod local;
pub mod node;

/// The refusals every port shares (`docs/platform-ports.md` § 3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The place did not answer; transient, the step tries again on its next tick.
    Unavailable { detail: String },
    /// This placement's adapter does not serve the operation.
    NotServed { operation: String },
    /// The input breaks a bound the place states.
    Invalid { field: String, reason: String },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { detail } => write!(f, "unavailable: {detail}"),
            Self::NotServed { operation } => write!(f, "{operation} is not served here"),
            Self::Invalid { field, reason } => write!(f, "invalid {field}: {reason}"),
        }
    }
}

impl std::error::Error for Refusal {}
