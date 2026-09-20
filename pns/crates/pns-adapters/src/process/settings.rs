use std::ops::RangeInclusive;
use std::time::Duration;

/// A deadline override, typed as the same `<count><ms|s|m|h>` duration string
/// as every flag and config key, for tests that must prove expiry without
/// waiting out the production window.
///
/// A VALUE THE PARSER REFUSES IS REPORTED AND DROPPED, leaving the caller's
/// own default: the paths reading these are hooks whose contract is exiting
/// 0, so a mistyped variable must say so rather than end the run.
pub fn env_duration(variable: &str, range: RangeInclusive<Duration>) -> Option<Duration> {
    duration_from(variable, std::env::var(variable).ok().as_deref(), range)
}

/// The reading itself, with the environment handed in, which is how the two
/// rows below drive it without setting a process-wide variable.
fn duration_from(
    variable: &str,
    raw: Option<&str>,
    range: RangeInclusive<Duration>,
) -> Option<Duration> {
    match pns_domain::duration::parse_duration(variable, raw?, range) {
        Ok(duration) => Some(duration),
        Err(refusal) => {
            eprintln!("{refusal}");
            None
        }
    }
}
/// Where the moshi-hook binary is, asked ONE WAY for every caller.
///
/// Two spellings of "where is moshi-hook" is exactly the duplicated rule this
/// crate keeps being bitten by: the day one of them learns a second lookup the
/// other keeps answering the old address, and the two disagree silently. It is
/// also the seam every test drives the binary through, which is what makes a
/// caller stubbable at all.
pub fn moshi_hook_bin() -> String {
    std::env::var("PNS_MOSHI_HOOK_BIN").unwrap_or_else(|_| DEFAULT_MOSHI_HOOK_BIN.to_string())
}
/// Homebrew's own prefix, which is where the cask puts it. `PNS_MOSHI_HOOK_BIN`
/// overrides it, and that override is how every test points a caller at a stub
/// instead of at the operator's own moshi.
const DEFAULT_MOSHI_HOOK_BIN: &str = "/opt/homebrew/bin/moshi-hook";

#[cfg(test)]
mod tests {
    use super::duration_from;
    use std::time::Duration;

    const RANGE: std::ops::RangeInclusive<Duration> =
        Duration::from_millis(1)..=Duration::from_secs(60);

    #[test]
    fn a_deadline_variable_takes_the_same_duration_string_as_a_flag() {
        assert_eq!(
            duration_from("PNS_PAYLOAD_DEADLINE", Some("500ms"), RANGE),
            Some(Duration::from_millis(500))
        );
        assert_eq!(
            duration_from("PNS_PAYLOAD_DEADLINE", Some("30s"), RANGE),
            Some(Duration::from_secs(30))
        );
        assert_eq!(duration_from("PNS_PAYLOAD_DEADLINE", None, RANGE), None);
    }

    #[test]
    fn a_bare_number_is_refused_by_the_variable_that_was_typed() {
        // It meant milliseconds to this reader and seconds to the one beside
        // it, which is the ambiguity the unit removes. The refusal names the
        // variable and the shapes it accepts, because one parser now serves
        // every duration in pns.
        assert_eq!(
            duration_from("PNS_PAYLOAD_DEADLINE", Some("500"), RANGE),
            None
        );
        assert_eq!(
            pns_domain::duration::parse_duration("PNS_PAYLOAD_DEADLINE", "500", RANGE),
            Err("pns: PNS_PAYLOAD_DEADLINE \"500\" is not <count><ms|s|m|h>".to_string())
        );
    }

    #[test]
    fn a_duration_outside_the_callers_range_is_refused_rather_than_clamped() {
        assert_eq!(
            duration_from("PNS_PAYLOAD_DEADLINE", Some("2h"), RANGE),
            None
        );
    }
}
