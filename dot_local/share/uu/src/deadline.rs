//! The per-lane deadline: how long one lane may run before uu stops it.
//!
//! WHY EVERY LANE HAS ONE BY DEFAULT. `uu run` takes a non-blocking whole-run
//! `flock` and holds it across lane execution, so a lane that never ends is a
//! lock that is never released and every later run does nothing at all. A
//! default is what bounds the lanes nobody thought about, and those are the
//! ones that produce this failure.

use std::time::Duration;

use crate::config::ConfigError;

/// SIX HOURS, chosen against both ends of the range it has to sit between.
///
/// The floor is the slowest HONEST lane. A full `brew upgrade` downloads and
/// installs multi-gigabyte casks and can build from source, and the herdr lane
/// reinstalls every plugin from its source's tip, which compiles. NOTHING IN
/// THIS REPO TIMES EITHER, so an hour or two is an ESTIMATE and not a
/// measurement; six hours is picked to sit well clear of it rather than close
/// to it, because a deadline near the honest run time turns a slow week into
/// an outage uu inflicted on itself.
///
/// The ceiling is the SCHEDULE. These jobs run weekly, so anything well under
/// 168 hours hands the lock back long before the next slot. A lane that
/// genuinely needs longer states its own `deadline_secs`. What bounds the
/// whole locked run, which is a different question, is `RUN_DEADLINE`.
pub const DEFAULT_LANE_DEADLINE: Duration = Duration::from_secs(6 * 60 * 60);

/// TWENTY-FOUR HOURS for the WHOLE RUN, because a per-lane bound does not
/// bound the run. One `flock` covers every lane, lanes run in sequence, and
/// nothing caps how many a config declares, so five defaulted lanes would hold
/// that lock for thirty hours and twenty-nine for a week. The next weekly slot
/// would then find the lock still held and do nothing, which is the exact
/// failure the per-lane deadline exists to prevent.
///
/// A day is a seventh of the period and longer than every lane's honest work
/// put together, so it fires on a run that has gone wrong and on nothing else.
/// Not configurable: a machine that needs a longer weekly run needs fewer
/// lanes per lock, which is the per-lane invocation capability, not a bigger
/// number here.
pub const RUN_DEADLINE: Duration = Duration::from_secs(24 * 60 * 60);

/// What one lane may actually have: its own deadline, or all that is left of
/// the run's, whichever is less.
pub fn lane_budget(declared: Duration, run_elapsed: Duration) -> Duration {
    declared.min(RUN_DEADLINE.saturating_sub(run_elapsed))
}

/// `deadline_secs`: a positive whole number of seconds.
///
/// ZERO IS REFUSED rather than read as "no deadline". A lane that may run
/// forever is the one state this key exists to end, and a config that asks for
/// it by writing `0` is a typo far more often than a request.
pub fn parse_deadline(table_label: &str, setting: &toml::Value) -> Result<Duration, ConfigError> {
    let Some(seconds) = setting.as_integer() else {
        return Err(ConfigError::Invalid(format!(
            "`{table_label}` key `deadline_secs` has type `{}`, not an integer",
            setting.type_str()
        )));
    };
    match u64::try_from(seconds) {
        Ok(seconds) if seconds > 0 => Ok(Duration::from_secs(seconds)),
        _ => Err(ConfigError::Invalid(format!(
            "`{table_label}` key `deadline_secs` is `{seconds}`, which is not a positive number \
             of seconds; remove the key to take the {}s default",
            DEFAULT_LANE_DEADLINE.as_secs()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LANE_TYPES, parse_config};

    fn deadline_of(text: &str, lane: &str) -> Duration {
        parse_config(text)
            .expect("this config is valid")
            .lanes
            .get(lane)
            .expect("this lane is declared")
            .deadline
    }

    fn refusal(text: &str) -> String {
        match parse_config(text) {
            Err(error) => error.detail().to_string(),
            Ok(config) => panic!("this config should have been refused, got {config:?}"),
        }
    }

    #[test]
    fn a_lane_gets_its_own_deadline_while_the_run_has_room_for_it() {
        assert_eq!(
            lane_budget(Duration::from_secs(90), Duration::ZERO),
            Duration::from_secs(90)
        );
    }

    #[test]
    fn a_lane_is_cut_to_what_is_left_of_the_run_rather_than_extending_it() {
        // The run holds ONE lock across every lane, so a lane starting late
        // gets the remainder and not its own full deadline.
        assert_eq!(
            lane_budget(
                DEFAULT_LANE_DEADLINE,
                RUN_DEADLINE - Duration::from_secs(60)
            ),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn a_run_already_past_its_own_deadline_leaves_a_lane_nothing() {
        assert!(lane_budget(DEFAULT_LANE_DEADLINE, RUN_DEADLINE * 2).is_zero());
    }

    #[test]
    fn a_lane_that_states_no_deadline_takes_the_default() {
        assert_eq!(
            deadline_of("[lanes.herdr]\n", "herdr"),
            DEFAULT_LANE_DEADLINE
        );
    }

    #[test]
    fn a_lane_may_state_its_own_deadline_in_seconds() {
        assert_eq!(
            deadline_of("[lanes.herdr]\ndeadline_secs = 90\n", "herdr"),
            Duration::from_secs(90)
        );
    }

    #[test]
    fn every_built_in_lane_type_is_bounded_whether_or_not_its_block_says_so() {
        // The structural guarantee the deadline lives beside the lane's KIND
        // rather than inside it for: a lane type added later is bounded by
        // construction, not by its author remembering to carry the field.
        let fixtures: &[(&str, &str)] = &[
            ("command", "[lanes.command]\nrun = [\"x\"]\n"),
            ("herdr", "[lanes.herdr]\n"),
        ];
        assert_eq!(LANE_TYPES.len(), fixtures.len());
        for (kind, text) in fixtures {
            assert_eq!(deadline_of(text, kind), DEFAULT_LANE_DEADLINE, "{kind}");
            assert_eq!(
                deadline_of(&format!("{text}deadline_secs = 7\n"), kind),
                Duration::from_secs(7),
                "{kind}"
            );
        }
    }

    #[test]
    fn a_deadline_that_is_not_a_positive_number_of_seconds_is_refused_by_name() {
        // Zero would mean the lane may run forever, which is the state the key
        // exists to end; a negative one is no duration at all.
        for stated in ["0", "-1"] {
            let detail = refusal(&format!("[lanes.herdr]\ndeadline_secs = {stated}\n"));
            assert!(
                detail.contains(&format!("key `deadline_secs` is `{stated}`")),
                "{stated}: {detail}"
            );
            assert!(
                detail.contains("not a positive number of seconds"),
                "{stated}: {detail}"
            );
        }
    }

    #[test]
    fn a_deadline_that_is_not_an_integer_is_refused_naming_what_was_written_instead() {
        for (stated, written) in [("\"90\"", "string"), ("1.5", "float"), ("true", "boolean")] {
            let detail = refusal(&format!("[lanes.herdr]\ndeadline_secs = {stated}\n"));
            assert!(
                detail.contains(&format!("has type `{written}`, not an integer")),
                "{stated}: {detail}"
            );
        }
    }

    #[test]
    fn a_lane_block_naming_a_deadline_is_still_refused_for_a_key_it_misspelled() {
        let detail = refusal("[lanes.herdr]\ndeadline_secs = 90\nbogus = 1\n");
        assert!(
            detail.contains("unknown `lanes.herdr` key `bogus`"),
            "{detail}"
        );
        assert!(
            detail.contains("deadline_secs"),
            "the refusal lists it: {detail}"
        );
    }
}
