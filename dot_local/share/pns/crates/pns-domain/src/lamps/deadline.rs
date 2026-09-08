use std::time::Duration;

/// How long ONE of a tick's bridge calls may take.
///
/// A FIFTH OF THE INTERVAL, so the three the resolve makes cannot outlive the
/// child that makes them AND still leave a breath: at the transport's own ten
/// seconds they outlive every interval the config permits, and a wedged bridge
/// would then have tick after tick piling up, each still dialling while the next
/// was spawned. A fifth is what keeps a full cycle of the shortest locked shape
/// inside what is left even when all three calls run to their deadline, which is
/// the whole point of the child staying alive.
///
/// A SECOND AT LEAST, which the division cannot reach anyway inside the config's
/// own bounds; a bridge on the same LAN answers these in milliseconds either
/// way.
pub fn tick_bridge_deadline(refresh_secs: u64) -> Duration {
    Duration::from_secs((refresh_secs / 5).max(1))
}
