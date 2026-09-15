use pns_domain::Delivery;

/// The recap posted, on the one route a recap has.
///
/// SYNCHRONOUS INSIDE THIS PROCESS, and REPORTING, which is the mode whose
/// whole purpose is that a failure is visible. Nobody is behind this, and a
/// silently dropped recap is the exact failure the feature exists to prevent.
///
/// ONE ROUTE, SO ONE POST. The recap took the `pns-recap` route first and fell
/// back to the default one when that refused; the route and its Discord
/// channel retired on 2026-09-15, so a recap now goes where every other
/// session event goes and there is nothing left to fall back from. A refusal
/// is REPORTED by the leg's own `ReportOutcome` mode rather than retried: a
/// gateway having a bad minute would otherwise post every recap twice.
pub fn post_return_recap(body: &str, deliver: impl FnOnce(&str, &str) -> Vec<Delivery>) -> i32 {
    deliver(body, "");
    0
}

#[cfg(test)]
mod tests;
