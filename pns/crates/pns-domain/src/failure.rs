//! One delivery failure, and the two ways it is written out.
//!
//! ONE RECORD, TWO RENDERINGS, and neither is a summary of the other. The full
//! form goes where there is no length limit and a monospace face: the terminal,
//! `pns doctor`, `pns failures`, the log, and a fenced block in Discord. The
//! notification form goes to the desktop banner and the phone card, under a
//! budget measured on this machine rather than guessed at.
//!
//! The classification lives in [`crate::retry`] and is not repeated here. What
//! this module owns is the WORDING, which varies by destination, and the fix
//! line, which varies by the surface the reader is standing at.

use crate::retry::{DeliveryOutcome, FailureClass};

mod fix;
mod meaning;
mod render;

pub use fix::{NotificationSurface, Surface};
pub use meaning::{DESTINATION_HERMES, DESTINATION_MOBILE, HERMES_KEY, MOBILE_TOKEN};
pub use render::{full, notification};

/// The budget a notification body has, counting newlines, with the header
/// excluded.
///
/// Measured by bisection on 2026-09-08: 256 arrives whole, 257 is truncated,
/// and two probes of different lengths cut at the same absolute position, which
/// is what identified a number rather than a range. It is a TOTAL budget and
/// not a per-line one, so rewrapping a long line buys nothing back; only
/// removing content does.
pub const NOTIFICATION_MAX_CHARS: usize = 256;

/// What a delivery failure knows about itself. Every field is something a
/// reader needs; nothing here is producer text.
///
/// `detail` is deliberately absent. It holds arbitrary producer prose and never
/// appears in an error, because an error the operator is meant to act on cannot
/// also be a place a producer writes into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// The counting number shown everywhere, short enough to read off a phone
    /// and type at a terminal.
    pub id: u64,
    /// Which destination refused it: [`DESTINATION_HERMES`] or
    /// [`DESTINATION_MOBILE`]. The wording table is chosen by this, because a
    /// 401 from hermes and a 401 from moshi name different secrets.
    pub destination: String,
    /// The route the producer named.
    pub route: String,
    /// Where the destination lives, as the reader would type it: a host and
    /// port for hermes, a URL for moshi. Named once per message.
    pub address: String,
    /// The producer, shown as `sent by` in the full form.
    pub agent: String,
    /// The routing flags pns was given, PAST TENSE: the command that already
    /// ran and failed, never an instruction to run one.
    pub command: String,
    /// What the destination answered.
    pub outcome: DeliveryOutcome,
    /// Attempts spent and allowed, which is what the temporary variant's fix
    /// line counts off.
    pub retries: u64,
    pub max_attempts: u64,
}

impl Failure {
    /// Permanent or temporary, through the one rule in [`crate::retry`]. A
    /// delivered outcome is not a failure and reads as temporary here rather
    /// than being given a repair to perform; nothing constructs a `Failure`
    /// from one.
    pub fn class(&self) -> FailureClass {
        self.outcome.class()
    }

    /// The heading a notification carries, and the reason it varies per
    /// FAILURE rather than per producer.
    ///
    /// The `agent · state · project` triple is used as a notification key, and
    /// four probes sharing one triple produced ONE notification where four
    /// distinct ones produced four. Every posture delivery failure would
    /// otherwise carry the same triple, so a second failure could quietly
    /// displace the first and the operator would never learn there were two.
    /// For a security page that is a lost page, which is the outcome this whole
    /// design exists to prevent. The id is the natural discriminator: unique by
    /// construction, and already in the message.
    pub fn title(&self) -> String {
        crate::render::title(&self.agent, "delivery failed", &format!("#{}", self.id))
    }
}

#[cfg(test)]
mod tests;
