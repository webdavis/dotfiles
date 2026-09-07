//! The JSON bodies for a pulse, breath, fade and clear.

/// The brightness a body that states one runs at, as the bridge takes it.
fn dimming(percent: u8) -> serde_json::Value {
    serde_json::json!({"brightness": f64::from(percent)})
}

/// The PULSE body: a timed signal the BRIDGE runs and ends by itself.
///
/// THE BRIDGE OWNS THE WHOLE EFFECT. It flashes the colour for the duration and
/// then puts the lamp back exactly as it was, with no snapshot, no restore
/// writes and no choreography from us, which is why this channel is one PUT.
/// MEASURED ON 2026-09-01, on a real lamp, in both directions: a full state read
/// before and after a signal came back byte-identical with the lamp on and with
/// it off.
///
/// IT ALWAYS STATES A BRIGHTNESS, and that is the price of a config that can
/// dim: a `dimming` written beside a signal PERSISTS after the signal ends
/// (drill D4, 2026-08-30), so a body that said nothing would inherit whatever
/// the last dim write left. The `[plugins.hue] rooms` path below states none,
/// and the lamp comes back byte-identical, because nothing on that path can
/// ever write a floor.
pub fn pulse_body(
    pulse: &crate::config::Pulse,
    color: crate::pulse::PulseColor,
    brightness: u8,
) -> String {
    serde_json::json!({
        "signaling": {
            "signal": "on_off_color",
            "duration": pulse.duration_ms,
            "colors": [{"xy": {"x": color.x, "y": color.y}}],
        },
        "dimming": dimming(brightness),
    })
    .to_string()
}

/// The body that ARMS a breath: the colour, the lamp on, and the first fade all
/// in one write.
///
/// ONE WRITE RATHER THAN TWO, because a colour write followed by a fade is a
/// visible jump: the lamp would land at whatever brightness it was already at,
/// in the new colour, before starting to move. Stating the first fade's target
/// here means the first move begins from wherever the lamp is, toward
/// whichever end the tick's `Resume` picked, which is the seamless join
/// between two ticks. THIS RUNS ON EVERY TICK, resumed or not: an externally
/// switched-off lamp comes back on with its first fade whichever end the
/// held record names.
pub fn breath_arm_body(color: crate::pulse::PulseColor, fade: &crate::lights::Fade) -> String {
    serde_json::json!({
        "on": {"on": true},
        "color": {"xy": {"x": color.x, "y": color.y}},
        "dimming": dimming(fade.brightness),
        "dynamics": {"duration": fade.duration_ms},
    })
    .to_string()
}

/// Every fade after the first: brightness and how long to take getting there,
/// and nothing else. THE DURATION IS THE FADE'S OWN, so the accent at the peak
/// of the loop's motion is issued at its own short duration rather than at the
/// duration of the fades around it.
///
/// NO COLOUR AND NO `on`. The arm already stated both, and repeating them would
/// be two more fields the bridge has to reconcile mid-transition on every fade
/// of every breath.
pub fn fade_body(fade: &crate::lights::Fade) -> String {
    serde_json::json!({
        "dimming": dimming(fade.brightness),
        "dynamics": {"duration": fade.duration_ms},
    })
    .to_string()
}

/// What puts a held lamp out.
///
/// OFF, AND NOT A RESTORE. Nothing snapshotted what the lamp was doing before
/// the breath took it, and a grouped_light GET carries no colour at all, so
/// there is nothing honest to put back. Dark is what "the state is over" means
/// everywhere else on this path, and the operator's own ruling is that pns
/// animates in-use lamps.
pub fn clear_body() -> String {
    serde_json::json!({"on": {"on": false}}).to_string()
}
