use std::time::Duration;

/// A deadline override in milliseconds, for tests that must prove expiry
/// without waiting out the production window.
pub fn env_deadline(variable: &str) -> Option<Duration> {
    std::env::var(variable)
        .ok()?
        .parse()
        .ok()
        .map(Duration::from_millis)
}
/// Where the moshi-hook binary is, asked ONE WAY for every caller.
///
/// Two spellings of "where is moshi-hook" is exactly the duplicated rule this
/// crate keeps being bitten by: the day one of them learns a second lookup the
/// other keeps answering the old address, and the two disagree silently. It is
/// also the seam every test drives the binary through, which is what makes a
/// caller stubbable at all.
pub fn moshi_hook_bin() -> String {
    std::env::var("MOSHI_HOOK_BIN").unwrap_or_else(|_| DEFAULT_MOSHI_HOOK_BIN.to_string())
}
/// Homebrew's own prefix, which is where the cask puts it. `MOSHI_HOOK_BIN`
/// overrides it, and that override is how every test points a caller at a stub
/// instead of at the operator's own moshi.
const DEFAULT_MOSHI_HOOK_BIN: &str = "/opt/homebrew/bin/moshi-hook";
