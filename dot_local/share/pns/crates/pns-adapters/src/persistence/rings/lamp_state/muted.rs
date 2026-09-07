use super::*;

/// Publish the file, or remove it when nothing is muted.
///
/// AN EMPTY FILE IS NO FILE, which is `remember_held`'s own rule and is
/// what keeps the reader's refusal of an empty one honest: this never writes
/// one, so a file with no lines in it was written by something else.
pub fn publish_muted(
    state: &Path,
    kept: &[pns_domain::lights::mute::Muted],
) -> std::io::Result<()> {
    if kept.is_empty() {
        return match std::fs::remove_file(state) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    }
    publish_state_line(state, &crate::lights_codec::render_muted(kept))
}

/// Everything the ad-hoc quiet file holds, and the complaint from a file this
/// cannot vouch for.
///
/// ONE READER FOR BOTH READERS, which is why the command and the event path
/// share it: they want different things out of the file (the entries to rebuild
/// and the names that are live), and two readers is two chances for one of them
/// to swallow a failure the other reports.
///
/// A MISSING FILE IS THE ORDINARY CASE and says nothing: the command has
/// never been run, or its last mute expired and took the file with it. EVERY
/// OTHER READ FAILURE IS A COMPLAINT, and the distinction is the point: a file
/// that is unreadable, not UTF-8, or a directory standing where it should be
/// says NOTHING about which places are quiet, exactly as a corrupt one does,
/// and the two readers of that complaint take opposite directions with it.
/// `ad_hoc_quiet` mutes EVERYTHING (a lamp path fails dark), and the command
/// prints it and rebuilds from an empty list. Either way the operator is told,
/// which is what a complaint is for: a mute nobody can see, in either
/// direction, is the state worth a sentence.
pub fn muted_state(state: &Path) -> (Vec<pns_domain::lights::mute::Muted>, Vec<String>) {
    let contents = match std::fs::read_to_string(state.join(LIGHTS_QUIET)) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (Vec::new(), Vec::new());
        }
        Err(error) => {
            return (
                Vec::new(),
                vec![format!(
                    "pns: state error (lights-quiet could not be read: {error}); \
                     nothing is quiet"
                )],
            );
        }
    };
    match crate::lights_codec::muted_entries(&contents) {
        Ok(entries) => (entries, Vec::new()),
        Err(complaint) => (Vec::new(), vec![complaint]),
    }
}

/// Where the operator's own ad-hoc quiet lives: one line per place, each an
/// expiry second and the name they typed.
///
/// ONE FILE RATHER THAN ONE PER PLACE, and that is a path-safety decision as
/// much as a tidiness one: a place is a room name the operator typed, spaces
/// and all, and nothing in this crate turns typed text into a filename unless a
/// predicate already vouches for it.
pub const LIGHTS_QUIET: &str = "lights-quiet";
/// Where the EVENT path remembers the ad-hoc quiet complaint it last made,
/// which is a file of its own for the reason `say_lights_once` states.
pub const LIGHTS_QUIET_SAID: &str = "lights-quiet-said";
