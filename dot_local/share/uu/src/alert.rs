//! The alerts path: a failed lane, said out loud through the pns engine.
//!
//! ALERTS ARE ARGV, NOT STDIN. pns's client interface is flags, the same ones
//! the shell notifier and the weekly bash jobs use; piping an event at it
//! yields an empty event and exit 0. uu is a pns CLIENT here and nothing more:
//! it never decides presence, escalation or which destination fires, because
//! that is the engine's whole job.
//!
//! NO `--channel`, deliberately. pns's default route IS the alert route, and
//! the record path is where the quiet weekly entry goes. An alert that landed
//! on the record channel would be a failure nobody is paged about.
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
                "--project",
                "dresden",
                "--detail",
                "herdr: 2 failure(s)",
            ]
        );
    }

    #[test]
    fn an_alert_never_names_a_channel_because_the_default_route_is_the_alert_route() {
        assert!(
            !alert_argv("dresden", "herdr", "x")
                .iter()
                .any(|a| a == "--channel"),
            "an alert on the record route is a failure nobody is paged about"
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
