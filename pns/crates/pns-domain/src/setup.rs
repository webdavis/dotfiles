//! The first-run wizard's answers and feature-arming decisions.
//!
//! The application owns question order; the configuration adapter owns serialization.

/// What the walk came back with. EVERY CREDENTIAL IS A PLAIN STRING AND EMPTY
/// MEANS DECLINED, so answering "no" to a feature and answering "yes" and then
/// pasting nothing compose the same file: an empty value parses as absent and
/// would deliver nothing while reading as configured, which is the state this
/// wizard exists to keep off a fresh machine.
///
/// THE DEFAULT IS THE WHOLE WALK DECLINED, which is the shipped posture: the
/// banner and the phone card, and nothing else armed.
#[derive(Debug, Default, PartialEq)]
pub struct Answers {
    /// The moshi webhook secret the phone card is submitted with. Skipped
    /// leaves mobile on and uncarded until a pairing supplies one.
    pub mobile_token: String,
    pub hermes_key: String,
    pub hue_bridge: String,
    pub hue_key: String,
    pub hue_rooms: Vec<String>,
    /// Which compiled-in backend answers the home probe.
    pub router_type: String,
    pub router_url: String,
    pub router_api_key: String,
    pub router_device_hostname: String,
    /// The Focus mode names that mean "not now". Empty is the feature off,
    /// which is what the parser reads an absent table as.
    pub focus_modes: Vec<String>,
    pub nag: bool,
}

/// Whether the walk armed the light pulse. THE ROOMS COUNT AS A CREDENTIAL:
/// with none named the plugin falls back to a compiled-in room list that names
/// nobody else's rooms, so a bridge and key alone are a pulse that reaches no
/// lamp and reports nothing.
pub fn hue_is_armed(answers: &Answers) -> bool {
    !answers.hue_bridge.is_empty() && !answers.hue_key.is_empty() && !answers.hue_rooms.is_empty()
}

/// Whether the walk armed the home probe. THE BACKEND COUNTS AS A CREDENTIAL:
/// the table's keys are free text to the parser, so a name no compiled-in
/// backend answers composes a probe that loads and then refuses every time it
/// runs, which is the same silent nothing an empty credential writes.
pub fn router_is_armed(answers: &Answers) -> bool {
    answers.router_type == crate::home::UNIFI_TYPE
        && !answers.router_url.is_empty()
        && !answers.router_api_key.is_empty()
        && !answers.router_device_hostname.is_empty()
}

/// What a typed line means as an answer.
///
/// A LINE OF NOTHING BUT SPACES IS A BLANK ONE, which is the rule the whole
/// walk rests on: `compose_config` declines a feature whose credential is
/// empty, and it asks `is_empty`, so a credential of two spaces would arm a
/// plugin with two spaces and deliver nothing while reading as set up. That is
/// the exact state this wizard exists to keep off a fresh machine, and the
/// trailing newline every line carries is what makes it reachable.
pub fn answered(line: &str) -> String {
    line.trim().to_string()
}

/// Whether an answer to a yes-or-no question was a yes.
///
/// ONLY A YES IS ONE. Enter, a word nobody meant, and a mistyped `yes` all
/// mean no, because every question this answers arms something that delivers
/// to a phone or to a lamp and takes a credential to do it.
pub fn means_yes(answer: &str) -> bool {
    matches!(answer.to_lowercase().as_str(), "y" | "yes")
}

/// Which compiled-in backend an answer names, or `None` for one no backend
/// answers.
///
/// THE SET IS THE CODE'S, never a list kept here: `home` is what refuses a
/// type at probe time, so a wizard restating its own copy of that set would go
/// on accepting yesterday's answer the day a second backend lands. Enter names
/// the one there is, and a spelling that differs only in case is that one too,
/// written back as the code spells it rather than as it was typed.
pub fn router_backend(answer: &str) -> Option<&'static str> {
    (answer.is_empty() || answer.eq_ignore_ascii_case(crate::home::UNIFI_TYPE))
        .then_some(crate::home::UNIFI_TYPE)
}

/// A comma-separated answer as the values it names, blanks dropped.
pub fn list(answer: String) -> Vec<String> {
    answer
        .split(',')
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

#[cfg(test)]
#[path = "setup/tests.rs"]
mod setup_walk_tests;
