//! Where the operator's body is, and what the lamps do about it.
//!
//! THREE QUESTIONS, THREE MODULES, and the split between them is the one the
//! modules' own docs draw: `status` says what a READING means, `room` says
//! WHERE THE BODY IS, and `narrowing` says what the lamps do about it.
//!
//! POLICY ONLY: no bridge, no file, no clock. The composition root takes one
//! snapshot of the world and hands it in.

mod narrowing;
mod room;
mod status;

pub use narrowing::{Narrowing, narrow};
pub use room::{Full, Snapshot, chosen};
pub use status::{
    Edge, PresenceStatus, RawPresence, Unreadable, classify, idle_secs_from_ns, unreadable_said,
};
