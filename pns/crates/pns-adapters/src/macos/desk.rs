use super::registry::ConsoleRegistry;
use crate::process::{PROBE_DEADLINE, bounded_call};
use std::sync::Arc;
use std::time::Duration;

/// The idle probe's own body, free of `&self` so `start` can run it on a
/// spawned thread against a cloned handle rather than borrowing the struct
/// across threads. The trait impl runs the SAME function inline.
pub(crate) fn idle_reading(registry: &Arc<dyn ConsoleRegistry>) -> Option<u64> {
    idle_reading_within(registry, PROBE_DEADLINE)
}

/// The lock probe's own body, same reason as `idle_reading` beside it.
///
/// A SECOND REGISTRY READ, not a second look at the idle probe's answer: the
/// aggregate hangs off the Root node and the idle counter off the
/// `IOHIDSystem` service. It is cheap and it only happens where the idle
/// reading it exists to qualify was taken.
///
/// THE FAIL DIRECTION IS DELIBERATE and the decision states it
/// (`surface::surface`): only `Some(true)` locks, so a reading nobody could
/// take leaves the shipped desk-freshness behavior in place instead of
/// killing the desk banner permanently wherever this property is renamed or
/// dropped.
pub(crate) fn lock_reading(registry: &Arc<dyn ConsoleRegistry>) -> Option<bool> {
    lock_reading_within(registry, PROBE_DEADLINE)
}

/// `idle_reading` with the deadline named, which is what lets a test drive a
/// registry that never answers without waiting the production window out.
pub(crate) fn idle_reading_within(
    registry: &Arc<dyn ConsoleRegistry>,
    deadline: Duration,
) -> Option<u64> {
    let registry = Arc::clone(registry);
    bounded_call(deadline, move || registry.idle_nanoseconds())
        .flatten()
        .map(pns_domain::idle_secs_from_ns)
}

/// `lock_reading` with the deadline named, same reason as the idle twin.
pub(crate) fn lock_reading_within(
    registry: &Arc<dyn ConsoleRegistry>,
    deadline: Duration,
) -> Option<bool> {
    let registry = Arc::clone(registry);
    bounded_call(deadline, move || registry.console_locked()).flatten()
}
