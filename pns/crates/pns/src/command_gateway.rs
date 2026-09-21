mod jobs;
pub(crate) mod pregenerate;
mod service;

/// `pns gateway <verb>`: the clock, the two typed commands that feed it, the
/// retry sweep, and the launchd service all four run under.
///
/// A BARE `pns gateway` IS A REFUSAL, per the house rule that an unknown
/// argument never falls through to help with exit 0: a verb this does not
/// serve is a command the operator believes ran.
pub(crate) fn gateway_mode(verb: &str) -> i32 {
    match verb {
        "run" => crate::daemon_runtime::daemon_run(),
        "retry" => crate::daemon_runtime::daemon_retry(),
        "schedule" => jobs::gateway_schedule(),
        "cancel" => jobs::gateway_cancel(),
        // The four service verbs and every unknown word, which that layer
        // refuses with the usage below.
        _ => service::service_mode(verb),
    }
}

pub(crate) const GATEWAY_USAGE: &str = "pns: usage: pns gateway run | \
pns gateway schedule --id <id> [--in <secs>] [--every <secs>] [--until +<secs>] [--until-epoch <epoch>] \
[--unless-marker <name>] -- <subcommand> [args] | \
pns gateway cancel --id <id> | \
pns gateway retry (one sweep of the retry queue, run by the clock) | \
pns gateway start | pns gateway stop | pns gateway restart | pns gateway status";

#[cfg(test)]
mod tests {
    use super::GATEWAY_USAGE;

    /// All eight verbs, in one text, because `pns gateway --help` prints this
    /// constant and nothing else.
    #[test]
    fn the_usage_names_every_verb_the_subcommand_serves() {
        for verb in [
            "run", "retry", "schedule", "cancel", "start", "stop", "restart", "status",
        ] {
            assert!(
                GATEWAY_USAGE.contains(&format!("pns gateway {verb}")),
                "`{verb}` is missing from: {GATEWAY_USAGE}"
            );
        }
    }
}
