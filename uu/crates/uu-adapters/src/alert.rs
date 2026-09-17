//! The alerts path: a failed lane, said out loud through the pns engine.
//!
//! ALERTS ARE ARGV, NOT STDIN. pns's client interface is flags, the same ones
//! the shell notifier and the weekly bash jobs use; piping an event at it
//! yields an empty event and exit 0. uu is a pns CLIENT here and nothing more:
//! it never decides presence, escalation or which destination fires, because
//! that is the engine's whole job.
//!
//! NO `--channel`, deliberately, and none is coming: uu says what the event IS
//! with `--kind health`, and pns maps that kind to the route it pages on
//! (operator ruling, 2026-09-15). A tool that named a route would be a tool
//! that had to know which Discord channels exist, and the record path is
//! already where the quiet weekly entry goes.
//!
//! FAIL OPEN. An absent or refusing engine is reported on stderr and the run
//! stays clean, because a notification must never fail the work it reports on.

/// The flags one alert is sent with. Pure, so what crosses the boundary is
/// decided where it can be read rather than inside a spawn.
pub fn alert_argv(host: &str, lane: &str, summary: &str) -> Vec<String> {
    [
        "--agent",
        uu_protocol::AGENT,
        "--state",
        "failed",
        "--kind",
        "health",
        "--project",
        host,
        "--detail",
        &format!("{lane}: {summary}"),
    ]
    .map(str::to_string)
    .to_vec()
}

/// The spawn seam: the engine, and the flags it is handed.
pub trait Alerter {
    /// `Err` carries why the alert did not go out, already fit to print.
    fn alert(&self, binary: &str, args: &[String]) -> Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_alert_names_uu_the_failure_the_host_and_the_lane() {
        let argv = alert_argv("dresden", "herdr", "2 failure(s)");
        assert_eq!(
            argv,
            vec![
                "--agent",
                "uu",
                "--state",
                "failed",
                "--kind",
                "health",
                "--project",
                "dresden",
                "--detail",
                "herdr: 2 failure(s)",
            ]
        );
    }

    #[test]
    fn an_alert_names_no_route_channel_or_gateway_of_any_kind() {
        // THE RULE THIS PINS, over uu's own request rather than over the
        // engine's answer: uu states what happened and never where it lands,
        // because a route and a channel belong to whatever gateway somebody
        // configured and uu is a tool other people install.
        let argv = alert_argv("dresden", "herdr", "2 failure(s)");
        for flag in ["--channel", "--route", "--url", "--webhook", "--priority"] {
            assert!(
                !argv.iter().any(|token| token == flag),
                "`{flag}` names a destination uu is not entitled to choose: {argv:?}"
            );
        }
        for token in &argv {
            assert!(
                !token.contains("://") && !token.starts_with('#'),
                "`{token}` is a gateway or a channel: {argv:?}"
            );
        }
    }

    #[test]
    fn a_failed_lane_is_a_health_event_because_a_failed_upgrade_pages() {
        let argv = alert_argv("dresden", "herdr", "x");
        let kind = argv
            .iter()
            .position(|a| a == "--kind")
            .map(|at| &argv[at + 1]);
        assert_eq!(
            kind.map(String::as_str),
            Some("health"),
            "an alert with no kind lands on the routine route nobody watches"
        );
    }

    #[test]
    fn every_flag_is_followed_by_its_own_value() {
        // pns drops a value flag whose next token is another recognized flag,
        // which would silently strip whatever the pair carried.
        let argv = alert_argv("dresden", "herdr", "2 failure(s)");
        assert_eq!(argv.len() % 2, 0, "{argv:?}");
        for pair in argv.chunks(2) {
            assert!(pair[0].starts_with("--"), "{argv:?}");
            assert!(!pair[1].starts_with("--"), "{argv:?}");
        }
    }
}
