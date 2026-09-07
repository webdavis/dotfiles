use super::*;

/// Say a complaint ONCE, and say it again only when it changes.
///
/// THE MARKER IS A PARAMETER because two paths say things at different rates
/// about different sets: the tick folds every refusal of a pass into one line,
/// and the event path says only what it read off the ad-hoc quiet file. Sharing
/// one memory would have each of them forgetting the other's line and repeating
/// it, which is the chatter this whole mechanism exists to stop.
pub fn say_lights_once(state: &Path, complaints: &[String], marker: &str) {
    let marker = state.join(marker);
    let remembered = std::fs::read_to_string(&marker).unwrap_or_default();
    match pns_domain::lights::phase::say(complaints, remembered.trim_end_matches('\n')) {
        pns_domain::lights::phase::Say::Nothing => {}
        pns_domain::lights::phase::Say::Aloud(said) => {
            for complaint in complaints {
                eprintln!("{complaint}");
            }
            let _ = publish_state_line(&marker, &said);
        }
        pns_domain::lights::phase::Say::Forget => {
            let _ = std::fs::remove_file(&marker);
        }
    }
}
/// Where a tick remembers what it last complained about.
pub const LIGHTS_SAID: &str = "lights-said";
