use crate::legacy::{SEND_USAGE, is_help_flag};

/// Every subcommand this binary answers to, paired with the usage text that
/// subcommand prints for itself.
///
/// ONE LIST, TWO READERS: the help router below answers `--help` off it, and
/// the test beside it checks the tool-wide listing names every word in it, so
/// a subcommand added without a usage string fails the suite rather than
/// answering its own help with the whole catalog.
///
/// A KEY IS A PATH, so a verb that has a usage of its own is reached by both
/// its words and the subcommand's own text answers the rest.
pub(crate) const SUBCOMMAND_USAGE: &[(&str, &str)] = &[
    ("send", SEND_USAGE),
    ("hook", crate::hook_dispatch::HOOK_USAGE),
    ("quiet", crate::command_quiet::QUIET_USAGE),
    ("daemon", crate::command_daemon::DAEMON_USAGE),
    ("gateway", crate::command_gateway::GATEWAY_USAGE),
    ("lights", crate::command_lights::LIGHTS_USAGE),
    ("lights pulse", crate::command_lights::PULSE_USAGE),
    ("lights enroll", crate::command_enroll::ENROLL_USAGE),
    ("presence", crate::command_presence::PRESENCE_USAGE),
    ("github", crate::command_github::GITHUB_USAGE),
    ("shell", crate::shell::SHELL_USAGE),
    ("loop", crate::lights_command::LOOP_USAGE),
    ("nag", crate::command_nag::NAG_USAGE),
    ("stale", crate::command_stale::STALE_USAGE),
    ("failures", crate::command_failures::FAILURES_USAGE),
    ("recap", pns_application::RECAP_USAGE),
    ("setup", pns_application::SETUP_USAGE),
    ("doctor", crate::command_doctor::DOCTOR_USAGE),
    ("tap", crate::command_tap::TAP_USAGE),
];

/// The subcommand's own usage when its argument tail asked for help, and
/// `None` for every tail that did not.
///
/// HELP IS ANSWERED BEFORE THE REST OF ARGV IS PARSED, which is what makes
/// `pns send --help` a printed usage rather than a complaint about a missing
/// state, and it EXITS 0: help asked for is a success, where the refusals
/// beside it exit 2 because the caller typed something wrong.
pub(crate) fn requested(subcommand: &str, tail: &[String]) -> Option<&'static str> {
    if !asked_for_help(tail) {
        return None;
    }
    let verb = tail
        .first()
        .filter(|word| !word.starts_with('-'))
        .map(|word| format!("{subcommand} {word}"));
    // THE VERB'S OWN TEXT WINS where it has one, because that is the narrower
    // answer to the narrower question.
    verb.as_deref()
        .and_then(usage_of)
        .or_else(|| usage_of(subcommand))
}

fn usage_of(path: &str) -> Option<&'static str> {
    SUBCOMMAND_USAGE
        .iter()
        .find(|(word, _)| *word == path)
        .map(|(_, usage)| *usage)
}

/// Whether a help flag sits where a flag belongs rather than where a value
/// does.
///
/// SLOT 0, OR SLOT 1 BEHIND A BARE VERB. A flag's value always follows its
/// flag, so a help flag in slot 0 is never a value, and slot 1 is only read
/// when slot 0 was a word rather than a flag. That is what keeps
/// `pns send --detail --help` a detail text while `pns daemon schedule --help`
/// is a question.
fn asked_for_help(tail: &[String]) -> bool {
    let mut words = tail.iter().map(String::as_str);
    match (words.next(), words.next()) {
        (Some(word), _) if is_help_flag(word) => true,
        (Some(verb), Some(word)) => !verb.starts_with('-') && is_help_flag(word),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legacy::USAGE;

    /// Every spelling a hook, a launchd job, the daemon or the shell notifier
    /// calls rather than an operator typing it.
    ///
    /// LISTED BECAUSE A READER WHO CANNOT FIND ONE CONCLUDES IT DOES NOT EXIST.
    /// The tool-wide help names each of these, and the test beside it is what
    /// keeps the listing and this list from drifting apart.
    const MACHINE_CALLED: &[&str] = &[
        "pns send",
        "pns hook <event>",
        "pns shell begin",
        "pns shell end",
        "pns daemon retry",
        "pns lights tick",
        "pns nag",
        "pns stale",
        "pns failures serve",
        "pns recap --since-epoch",
        "pns recap agent",
        "pns recap git",
        "pns presence poll [--daemon]",
        "pns github poll [--daemon]",
    ];

    fn strings(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_string()).collect()
    }

    #[test]
    fn every_subcommand_answers_both_help_flags_with_its_own_usage() {
        for (path, usage) in SUBCOMMAND_USAGE {
            let (word, verb) = path
                .split_once(' ')
                .map_or((*path, None), |(subcommand, verb)| (subcommand, Some(verb)));
            for flag in ["--help", "-h"] {
                let mut tail: Vec<&str> = verb.into_iter().collect();
                tail.push(flag);
                assert_eq!(
                    requested(word, &strings(&tail)),
                    Some(*usage),
                    "pns {path} {flag}"
                );
            }
            let word = path;
            assert!(
                usage.contains(&format!("pns {word}")),
                "{word}'s usage must name it: {usage}"
            );
        }
    }

    #[test]
    fn a_verb_still_reaches_its_subcommands_help() {
        for tail in [
            strings(&["schedule", "--help"]),
            strings(&["schedule", "-h"]),
        ] {
            assert_eq!(
                requested("daemon", &tail),
                Some(crate::command_daemon::DAEMON_USAGE),
                "{tail:?}"
            );
        }
    }

    #[test]
    fn a_help_flag_in_a_value_position_is_still_just_a_value() {
        assert_eq!(requested("send", &strings(&["--detail", "--help"])), None);
    }

    #[test]
    fn a_tail_that_asked_nothing_is_left_to_the_subcommand() {
        for tail in [strings(&[]), strings(&["poll"]), strings(&["--raw"])] {
            assert_eq!(requested("doctor", &tail), None, "{tail:?}");
        }
    }

    #[test]
    fn a_word_that_names_no_subcommand_has_no_usage_to_print() {
        assert_eq!(requested("stpo", &strings(&["--help"])), None);
    }

    #[test]
    fn the_tool_wide_listing_names_every_subcommand() {
        for (word, _) in SUBCOMMAND_USAGE {
            assert!(
                USAGE.contains(&format!("pns {word}")),
                "pns --help must name `{word}`"
            );
        }
    }

    #[test]
    fn the_tool_wide_listing_names_every_machine_called_spelling() {
        for spelling in MACHINE_CALLED {
            assert!(
                USAGE.contains(spelling),
                "pns --help must name `{spelling}`"
            );
        }
    }
}
