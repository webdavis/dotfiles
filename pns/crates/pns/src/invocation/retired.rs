//! The words that moved, and where each went.
//!
//! ONE PLACE FOR ALL THREE, so a reader who wonders whether a spelling still
//! works finds every answer together rather than three branches apart, and
//! each still exits 2 with the sentence naming its replacement.

/// The exit code for a retired spelling, or None for a word that never was
/// one.
pub(crate) fn retired(first: &str) -> Option<i32> {
    match first {
        "pulse" => Some(crate::command_lights::retired_pulse()),
        "quiet" => Some(crate::command_mute::retired_quiet()),
        "click" => Some(crate::command_failures::retired_click()),
        _ => None,
    }
}
