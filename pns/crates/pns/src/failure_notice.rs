//! Raising the banner that says a page did not arrive.
//!
//! THE READ IS THE SOURCE, not the delivery's return value. A leg's id, its
//! retry count and whether pns has given up are all facts the ledger settles
//! when it records the attempt, and reading them back is what keeps this
//! module's banner and `pns failures` saying the same thing about the same
//! failure. Passing them forward from the attempt would be a second copy of a
//! record that already exists, free to disagree with it.
//!
//! IT IS NOT ON THE LEDGER'S SIDE. A failure notice that failed is not itself
//! recorded and not retried: the log, `pns failures` and `pns doctor` all still
//! hold the failure, and a queue of notices about a broken queue is the loop
//! this stays out of.

use pns_adapters::SqliteStore;
use pns_application::StoredFailure;
use pns_domain::failure::{self, Failure, NotificationSurface};

/// How far back one pass looks. A pass records at most a handful of failures,
/// and the `failed_at` filter below is what actually bounds the set; this is
/// only the window the ledger is asked for.
const SCAN: u32 = 20;

/// Announce every failure this pass recorded that warrants it.
///
/// `since` is the moment the pass began, and it is what makes this idempotent
/// across passes: a leg that failed in an earlier pass has an older
/// `failed_at`, so a second call never re-announces it.
pub(crate) fn announce(store: &SqliteStore, since: u64) {
    let Ok(failures) = store.failing_legs(SCAN) else {
        // NOTHING IS SAID ABOUT AN UNREADABLE LEDGER HERE. That is the delivery
        // health alarm's own subject, it already reports it, and a second voice
        // saying it would be two banners for one fault.
        return;
    };
    let pns = crate::command_click::pns_path();
    for stored in to_announce(&failures, since) {
        raise(&crate::command_failures::compose(stored), &pns);
    }
}

/// Which of the ledger's failing legs this pass speaks for.
///
/// SPLIT OUT SO IT CAN BE PROVEN, because the half above it is a notifier spawn
/// and the half below is the whole of the decision. A test-only copy of this
/// filter would be free to disagree with the one that runs.
fn to_announce(failures: &[StoredFailure], since: u64) -> Vec<&StoredFailure> {
    failures
        .iter()
        .filter(|stored| stored.failed_at >= since)
        .filter(|stored| failure::warrants_notification(stored.retries, stored.deadlettered))
        .collect()
}

/// One banner for one failure.
///
/// THE BANNER IS NEVER THE DESTINATION THAT FAILED, which is what makes it safe
/// to raise unconditionally: it is local, it is not hermes and it is not the
/// phone, so it cannot be the surface a 404 just took away.
fn raise(failure: &Failure, pns_path: &str) {
    let args = pns_adapters::notifier_args(
        &failure.title(),
        &failure::notification(failure, NotificationSurface::Banner),
        Some("default"),
        pns_adapters::DEFAULT_TERMINAL_BUNDLE_ID,
        &failure::click_command(pns_path, failure.id),
    );
    // The outcome is DROPPED. A banner that will not post is the delivery
    // health alarm's subject rather than this module's, and there is no second
    // local surface to report it on.
    let _ = pns_application::CommandRunner::run(
        &pns_adapters::SystemCommandRunner,
        "terminal-notifier",
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    );
}

#[cfg(test)]
#[path = "failure_notice/tests.rs"]
mod tests;
