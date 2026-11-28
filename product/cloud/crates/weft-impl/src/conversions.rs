//! The conversions of `ess/components.yaml` (`conversions:`) the path needs, written by hand: the
//! generated code declares each as an obligation (`weft_types::obligation`) and leaves its
//! computation to the implementation.
//!
//! - `weft.shared.StampedAt -> Timestamp`: an instant weft-control stamped is an instant; the
//!   rows that keep it compare it with `now` like any other (`weft.nodes.Assign` stores its
//!   `assigned_at` in `weft.nodes.Assignment`). The newtype only marks the command input, so the
//!   instant passes as it is.

use weft_types::obligation::{UnmetObligation, WeftSharedStampedAtToTimestampConversion};
use weft_types::primitives::Timestamp;
use weft_types::shared::StampedAt;

/// The conversions weft implements.
#[derive(Debug, Clone, Copy, Default)]
pub struct Conversions;

impl WeftSharedStampedAtToTimestampConversion for Conversions {
    fn convert(&self, value: StampedAt) -> Result<Timestamp, UnmetObligation> {
        Ok(value.0)
    }
}

/// `value` as the instant a row keeps: [`Conversions`] on `weft.shared.StampedAt -> Timestamp`.
pub fn stamped(value: StampedAt) -> Result<Timestamp, UnmetObligation> {
    Conversions.convert(value)
}
