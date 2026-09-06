//! Raising a notification, and suppressing this process's own phone leg.

/// Raise one notification for an event this use case already built.
///
/// A PORT AND NOT A CALL, because the ORDER is what the approval path has to
/// pin: the notification goes after the nag is armed and before anybody waits
/// on the phone. A use case that could not invoke it could not state that.
///
/// It answers nothing. The notification path always exits 0, and no caller
/// acts on how it went.
///
/// Checked against `run_event` as `blocking_event` calls it
/// (`src/main.rs:2343`). Statements: S074.
pub trait RaiseNotification {
    fn raise(&self, event: &pns_domain::notification::EventArgs);
}

/// Suppress this process's own phone leg for the rest of this run.
///
/// ONLY WHERE A FORWARD REALLY BEGAN. The card moshi is raising is one the
/// surface model cannot know about, so the notification below it must not
/// raise a second; but a forward that never started suppresses nothing, or a
/// machine with no moshi would lose its phone card outright.
///
/// OBSERVABLE ON PURPOSE. "Only on a real spawn" is half of what the statement
/// says, and a suppression that left no trace would make that half unpinnable.
///
/// Checked against the `set_var("PNS_SKIP_PHONE", "1")` inside
/// `blocking_event`'s `forwarded.is_some()` arm (`src/main.rs:2341`).
/// Statements: S074.
pub trait PhoneSuppression {
    fn suppress(&self);
}
