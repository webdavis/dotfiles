use super::*;
/// The held record's entries, path and phase both, or None for a record this
/// cannot read.
///
/// ABSENT AND UNREADABLE ARE DIFFERENT ANSWERS, and collapsing them into an
/// empty list is what made a corrupt record read as a house holding nothing.
/// The event path's pulse gate then flashed straight over a lamp that was
/// breathing, and no reader was told. The ordinary case, a machine holding
/// nothing at all, is an ABSENT file and still answers with an empty list.
///
/// THE ONE PARSE, shared by every reader: `held_lamps` is this with the phase
/// dropped, so the three path-only consumers (the event path's pulse gate, the
/// operator's return, and the mute) read bare paths off the very same tokens
/// the breath's resume reads a phase from, and neither can drift from the
/// other's idea of what a token means.
pub fn read_held(state: &Path) -> Option<Vec<pns_domain::lights::phase::HeldEntry>> {
    match std::fs::read_to_string(state.join(LIGHTS_HELD)) {
        Ok(line) => Some(
            line.split_whitespace()
                .map(crate::lights_codec::parse_held_token)
                .collect(),
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(Vec::new()),
        Err(_) => None,
    }
}

/// The fixture paths a held write is currently holding, bare, or None for a
/// record this cannot read. See `read_held` for the phase this drops.
pub fn held_lamps(state: &Path) -> Option<Vec<String>> {
    read_held(state).map(|entries| entries.into_iter().map(|entry| entry.path).collect())
}

/// Record what is held now, or forget the file when nothing is.
///
/// ONE LINE, SPACE SEPARATED, because a fixture path is `light/<id>` or
/// `grouped_light/<id>` and neither can carry a space, and neither can carry
/// `@` or `:` either, which is what lets a phased token
/// (`light/<id>@<end-unix-ms>:<h|l>:<state>`) share the line with a bare one.
/// That keeps this a `publish_state_line` write like every other state file
/// rather than a second file format.
///
/// A TICK CAN REPUBLISH A GLOW THE RETURN JUST CLEARED, and that is a stated
/// limit rather than a rule. The tick reads its condition before it reaches the
/// bridge, so a present event that advances the return edge and clears the held
/// paths while an older tick is still resolving fixtures loses the race here:
/// that tick writes the glow and records it again. Nothing arbitrates, because
/// there is no lock between two processes that are deliberately independent.
/// The next present event clears it with no daemon at all, and the next tick
/// after it reads the advanced edge and finds no condition, so the exposure is
/// one refresh interval. It is unbounded only for a tick that was its lease's
/// LAST run, and there the lamp waits for the operator's return, which is the
/// event that clears it.
/// THE FAILURE IS RETURNED, not dropped, because the caller has to stop: a
/// lamp armed after a record that did not land is a lamp nothing in the system
/// knows the name of, and the return from an absence, the next tick and the
/// operator's own mute all put lamps out BY NAME off this file.
pub fn remember_held(
    state: &Path,
    held: &[pns_domain::lights::phase::HeldEntry],
) -> std::io::Result<()> {
    let marker = state.join(LIGHTS_HELD);
    if held.is_empty() {
        return match std::fs::remove_file(&marker) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    }
    let line = held
        .iter()
        .map(crate::lights_codec::render_held_token)
        .collect::<Vec<_>>()
        .join(" ");
    publish_state_line(&marker, &line)
}
/// Where the fixture paths a steady glow is holding are recorded.
pub const LIGHTS_HELD: &str = "lights-held";
