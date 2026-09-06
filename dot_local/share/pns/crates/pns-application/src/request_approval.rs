//! One approval request, end to end: hand the prompt to the phone, tell the
//! operator, and wait for an answer.
//!
//! THE ORDER IS THE BEHAVIOR, and it is the whole reason this is a use case.
//! The forward starts FIRST so the phone is already ringing while the rest
//! runs; the phone leg is suppressed only where that forward really began; the
//! nag is armed BEFORE the notification, so a prompt answered instantly still
//! has a record to clear; and the wait comes LAST, because everything above it
//! must happen whether or not anybody ever answers.

use crate::ports::delivery::ApprovalForwarder;
use crate::ports::notification::{PhoneSuppression, RaiseNotification};
use crate::ports::records::NagSchedule;
use pns_domain::notification::EventArgs;

/// The ports one approval request runs over.
pub struct RequestApproval<'a> {
    pub forwarder: &'a dyn ApprovalForwarder,
    pub phone: &'a dyn PhoneSuppression,
    pub nag: &'a dyn NagSchedule,
    pub notifier: &'a dyn RaiseNotification,
}

impl RequestApproval<'_> {
    /// Run the request and answer with the harness contract's exit code.
    ///
    /// `subcommand` is `None` where this agent forwards nothing at all, which
    /// is not a failure: the notification below still fires, and the hook
    /// answers 0. Every way of not forwarding collapses here on purpose,
    /// because no caller acts differently on them.
    pub fn run(
        &self,
        event: &EventArgs,
        session_id: &str,
        subcommand: Option<&str>,
        payload_json: &str,
    ) -> i32 {
        let forwarded =
            subcommand.and_then(|subcommand| self.forwarder.forward(subcommand, payload_json));

        // ONLY ON A REAL SPAWN. The card moshi is raising is one the surface
        // model cannot know about, so the notification below must not raise a
        // second; a forward that never started suppresses nothing, or a
        // machine with no moshi would lose its phone card outright.
        if forwarded.is_some() {
            self.phone.suppress();
        }

        // BEFORE THE NOTIFICATION, never after. The record this arms is what a
        // later answer clears, and a prompt answered between the notification
        // and the arming would leave a record nothing clears.
        self.nag.arm(session_id, event);
        self.notifier.raise(event);

        // LAST, because everything above happens whether or not anybody
        // answers. No forward is no waiting and exit 0, which the harness
        // reads as no opinion.
        forwarded.map_or(0, |forwarded| self.forwarder.answer(forwarded))
    }
}

#[cfg(test)]
mod tests;
