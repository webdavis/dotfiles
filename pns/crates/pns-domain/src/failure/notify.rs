//! When a failure is worth interrupting the operator for.
//!
//! A LEG THAT KEEPS FAILING IS NOT NEWS TWENTY TIMES. The retry loop reattempts
//! a temporary failure until its attempt or age limit runs out, and a banner per
//! attempt would train the operator to dismiss the one banner this whole design
//! exists to put in front of them. So a leg speaks twice at most, and each time
//! it says something the previous one did not.

/// Whether the failure just recorded warrants a notification.
///
/// TWO MOMENTS, and they are the two the reader can act on differently:
///
/// 1. **The first failure** (`retries == 0`), which is the news that something
///    broke. A permanent refusal dead-letters here, so this is also the only
///    moment a 404 on a mistyped route ever gets.
/// 2. **The dead-letter**, which is the news that pns has given up and the page
///    is lost for good. A temporary failure reaches this after its limits run
///    out; the attempts in between say nothing new.
///
/// A permanent failure satisfies both on the same record and is announced ONCE,
/// because a caller sees it once: it is dead-lettered on the attempt that also
/// spent its first retry.
pub fn warrants_notification(retries: u64, deadlettered: bool) -> bool {
    retries == 0 || deadlettered
}

#[cfg(test)]
#[path = "notify/tests.rs"]
mod tests;
