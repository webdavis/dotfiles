//! Native channel plugins: the compiled-in delivery half of a registration.
//!
//! DISPATCH PRECEDENCE, decided here once: with `PNS_CHANNELS_DIR` explicitly
//! set, executables win for every name, which is the test seam and the
//! operator's escape hatch. With it unset, a native plugin wins and the
//! executable fallback serves only names that have no native implementation
//! yet. That rule is what lets each channel slice go native without touching
//! the dispatch bats: the stubs keep winning wherever the suite points the
//! engine at a stub directory.

pub mod banner;

use crate::routing::Mode;

/// One rendered event, the structured form of the channel contract's JSON
/// object. The pane is the SANITIZED one.
#[derive(Debug, Default, PartialEq)]
pub struct Event {
    pub agent: String,
    pub state: String,
    pub project: String,
    pub branch: String,
    pub detail: String,
    pub title: String,
    pub message: String,
    pub preview: String,
    pub pane: String,
}

/// A native channel: it decides HOW to deliver and whether it can, never
/// WHETHER it should fire, and it must never fail the caller.
pub trait Channel {
    fn deliver(&self, event: &Event, mode: Mode);
}

/// True when native plugins take precedence for dispatch: only when the
/// channels directory was NOT explicitly overridden.
pub fn native_first(channels_dir_overridden: bool) -> bool {
    let _ = channels_dir_overridden;
    todo!("R2e: an explicit channels dir means executables win")
}
