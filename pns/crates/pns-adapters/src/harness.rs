//! The harness hooks: what a Claude Code or Codex event carries, and how a
//! turn becomes the event the engine already knows how to route.
//!
//! Everything here is PURE. The payload arrives as text, the transcript
//! arrives as text, and each is turned
//! into a decision without touching the world. The spawns and the files live
//! in adapters, which is what lets the whole turn-to-notification
//! path be tested without a harness, a transcript or a network.

mod message;
mod payload;
mod routing;
mod transcript;

pub use message::flattened;
pub use payload::{HookPayload, parse_payload};
pub use routing::{is_harness_subcommand, moshi_subcommand};
pub use transcript::transcript_reply;

#[cfg(test)]
mod tests;
