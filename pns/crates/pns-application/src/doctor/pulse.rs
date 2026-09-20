/// The pulse behind the same boundary every leg gets, so a panicking bridge
/// call costs the census the rest of its lines rather than ending the report
/// where the operator reads it as complete.
pub fn doctor_pulse(resolves: bool, pulse: impl FnOnce() -> usize) -> pns_domain::doctor::Outcome {
    if !resolves {
        return pns_domain::doctor::Outcome::Failed(no_hue_bridge_line());
    }
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(pulse)) {
        Ok(rooms) => pns_domain::doctor::Outcome::Signalled(rooms),
        // NO ROOM IS CLAIMED, and no panic text is quoted: the message is
        // written for a developer and may hold anything the pulse was carrying.
        Err(_) => pns_domain::doctor::Outcome::Failed(
            "the pulse PANICKED; no room was signalled".to_string(),
        ),
    }
}

/// The line for lights that were selected and never set up. It names the
/// settings to write, the way moshi's and hermes's do, because "no rooms"
/// without an address sends the operator to a bridge nothing dialled.
fn no_hue_bridge_line() -> String {
    use pns_domain::config_keys::{LIGHTS_API_KEY, LIGHTS_BRIDGE_HOST};
    format!(
        "pulse SKIPPED, no hue bridge and key in the config \
         ([plugins.lights] {LIGHTS_BRIDGE_HOST}, {LIGHTS_API_KEY}); nothing was signalled"
    )
}
