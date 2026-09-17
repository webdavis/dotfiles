//! Saying out loud that the bridge presented the wrong certificate.
//!
//! ONE PERMANENT FAILURE, REPORTED ONCE. A mismatch does not heal by retrying,
//! so it is announced on the first refused handshake a process sees and never
//! again: every lamp write after it fails the same way, and a banner per write
//! would train the operator to dismiss the one banner that matters.
//!
//! THE BANNER IS THE SURFACE, and the phone joins it when the operator is away,
//! which is the rule every delivery failure follows. Nothing routes through the
//! bridge that just refused, for the same reason a hermes failure is never
//! announced through hermes.

use pns_adapters::Mismatch;

/// The heading, short enough for a banner and specific enough to act on.
const TITLE: &str = "hue bridge · certificate refused";

/// Announce the mismatch this process has not spoken for, if there is one.
pub(crate) fn announce_mismatch() {
    let Some(mismatch) = pns_adapters::unreported_mismatch() else {
        return;
    };
    let body = body(&mismatch);
    eprintln!("{body}");
    let args = pns_adapters::notifier_args(
        TITLE,
        &body,
        Some("default"),
        pns_adapters::DEFAULT_TERMINAL_BUNDLE_ID,
        "",
    );
    // The outcome is DROPPED, the way every other local banner drops it: there
    // is no second local surface to report a banner that would not post on.
    let _ = pns_application::CommandRunner::run(
        &pns_adapters::SystemCommandRunner,
        "terminal-notifier",
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    );
}

/// What the report says. BOTH FINGERPRINTS, because the operator's next move is
/// to run the enrollment and compare what it prints against this.
fn body(mismatch: &Mismatch) -> String {
    format!(
        "pns: the bridge at {} presented a certificate pns is not pinned to, so \
every lamp call is refused\nexpected: {}\npresented: {}\nfix: run `pns lights enroll`, \
check the printed common name is the bridge you expect, and put its \
`certificate = \"sha256:...\"` line on the vault entry [plugins.hue] certificate reads",
        mismatch.address, mismatch.expected, mismatch.presented
    )
}

#[cfg(test)]
#[path = "certificate_notice/tests.rs"]
mod tests;
