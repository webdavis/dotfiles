//! The bridge seam and the bodies put over it: what a signal, a breath arm, a
//! fade and a clear look like on the wire, and the two renderers that decide
//! which of them one tick sends.

use super::{grouped_light_ids_for_rooms, inventory};
use crate::channels::hue::{Fixture, Muting, Routing, Showing, resolve};
use std::time::Duration;

/// The bridge seam: authenticated GETs and PUTs against the CLIP paths.
pub trait Bridge {
    fn get(&self, path: &str) -> Option<String>;
    /// Fire and forget: `run` discards every outcome, so a bridge that
    /// refuses tells no one. Returning a result would be a seam with no
    /// consumer.
    fn put(&self, path: &str, body: &str);
}

/// The three listings the routing is resolved from.
///
/// ALL THREE OR NOTHING. A listing that failed and a listing that was empty are
/// different answers, and collapsing them would resolve a config against an
/// empty inventory: every name it holds reported as a typo, every lamp dark,
/// all of it stated confidently about a bridge that said nothing.
pub fn resolve_on_bridge<B: Bridge>(bridge: &B, lights: &crate::config::Lights) -> Option<Routing> {
    let rooms = bridge.get("room")?;
    let lamps = bridge.get("light")?;
    let zones = bridge.get("zone")?;
    Some(resolve(&inventory(&rooms, &lamps, &zones), lights))
}

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

/// Put out every lamp a held write is still holding.
///
/// OFF THE HELD PATHS ALONE, with no listing resolved: the paths were recorded
/// when they were written, so a clear costs no GET and cannot be defeated by a
/// bridge that has stopped answering its listings. That is what lets the EVENT
/// path make this call with no daemon involved at all.
pub fn clear_held<B: Bridge>(bridge: &B, held: &[String]) {
    let body = clear_body();
    for path in held {
        bridge.put(path, &body);
    }
}

/// The colour and the CYCLE one held state runs at, dim form or full.
///
/// THE ONE MAPPING from a state to what it looks like, read by the tick and by
/// nothing else. Its two halves travel together because a dim breath in a full
/// colour, or the reverse, is a lamp saying half of one thing.
///
/// THE SHAPE IS SETTLED HERE AND NOWHERE ELSE. This is the only place that
/// knows the loop runs a three-leg motion while everything else runs a two-leg
/// breath; the driver below schedules whichever cycle it is handed. That is
/// also why the accent is a property of the RENDER rather than of the
/// behaviour: a loop lamp inside its dim window runs the shared dim breath, so
/// the same state has an accent at full and none dimmed.
pub fn held_render(
    held: crate::lights::Held,
    lights: &crate::config::Lights,
    showing: Showing,
) -> (crate::pulse::PulseColor, Vec<crate::lights::Leg>) {
    let (color, cycle) = match held {
        crate::lights::Held::Blocked => (
            crate::pulse::BLOCKED_COLOR,
            crate::lights::breath_cycle(&lights.blocked.breath),
        ),
        crate::lights::Held::Looping => (
            crate::pulse::LOOP_COLOR,
            crate::lights::breathe_then_flare_cycle(&lights.looping.breathe_then_flare),
        ),
        crate::lights::Held::UnreadFailure => (
            crate::pulse::FAILURE_COLOR,
            crate::lights::breath_cycle(&lights.unread.breath),
        ),
        crate::lights::Held::UnreadSuccess => (
            crate::pulse::UNREAD_SUCCESS_COLOR,
            crate::lights::breath_cycle(&lights.unread.breath),
        ),
    };
    // THE DIM FORM IS ONE SHAPE FOR EVERY BEHAVIOUR, which is what the operator
    // locked: the colour still says which state it is, and the shape says the
    // house is asleep.
    match showing {
        Showing::Dimmed => (color, crate::lights::breath_cycle(&lights.dim)),
        Showing::Dark | Showing::Full => (color, cycle),
    }
}

/// The colour and brightness one pulse fires at.
pub fn pulse_render(
    behaviour: crate::config::Behaviour,
    lights: &crate::config::Lights,
    showing: Showing,
) -> Option<(crate::pulse::PulseColor, crate::config::Pulse, u8)> {
    let (color, pulse) = match behaviour {
        crate::config::Behaviour::Done => (crate::pulse::SUCCESS_COLOR, lights.done),
        crate::config::Behaviour::Failed => (crate::pulse::FAILURE_COLOR, lights.failed),
        // A HELD STATE IS NOT A PULSE, and there is no nearest shape to fall
        // back to: a lamp asked to flash a state it holds would be armed with
        // something nobody measured.
        _ => return None,
    };
    // A DIMMED PULSE IS THE SAME BLINK AT THE DIM FLOOR, which is the faintest
    // the hardware goes; there is no low end for a blink to fade to.
    match showing {
        Showing::Dark => None,
        Showing::Full => Some((color, pulse, pulse.brightness)),
        Showing::Dimmed => Some((color, pulse, lights.dim.low)),
    }
}

/// What one lamp is judged against: the minute it is being asked about, and the
/// names the operator's own mute is covering.
pub struct Reading<'reading> {
    pub minutes_now: Option<u16>,
    /// AN EMPTY `Places` IS THE ORDINARY CASE, and a machine that has never run
    /// `pns lights quiet` reads an absent file as exactly that.
    pub muted: &'reading Muting,
}

/// The signal: one PUT per wanted room, and the bridge does the rest.
pub struct HuePulse<B: Bridge> {
    pub bridge: B,
    pub rooms: Vec<String>,
}

impl<B: Bridge> HuePulse<B> {
    /// Signal every wanted room, and answer with HOW MANY were signalled.
    ///
    /// THE COUNT IS THE ONLY OBSERVABLE FACT ON THIS PATH. `put` is fire and
    /// forget, so a write the bridge refused is invisible; what a caller can
    /// still learn is whether anything was addressed at all, and zero is the
    /// shape both likely misconfigurations take.
    pub fn run(&self, behaviour: crate::config::Behaviour) -> usize {
        let Some(rooms_json) = self.bridge.get("room") else {
            return 0;
        };
        let fixtures: Vec<Fixture> = grouped_light_ids_for_rooms(&rooms_json, &self.rooms)
            .into_iter()
            .map(Fixture::Grouped)
            .collect();
        signal_fixtures(&self.bridge, &fixtures, behaviour)
    }
}

/// One PUT per fixture, addressed by WHAT EACH ONE IS, and how many were
/// written.
///
/// INDEPENDENT per fixture, and every outcome ignored: there is no shared
/// choreography left for a refused write to corrupt, so one lamp's failure must
/// not cost another its signal, and a failed pulse still never fails the caller.
///
/// AND IT STATES NO BRIGHTNESS, ever. This is the path of a machine with no
/// `[lights]` table and of `pns pulse` on a machine with one: no routing is in
/// reach to dim, so nothing here can have left a floor on a lamp. The duration
/// stays fixed; the color follows wherever `SUCCESS_COLOR`/`FAILURE_COLOR` are
/// locked to, so it is not byte-identical across a color relock.
pub fn signal_fixtures<B: Bridge>(
    bridge: &B,
    fixtures: &[Fixture],
    behaviour: crate::config::Behaviour,
) -> usize {
    let (signal, color) = match behaviour {
        crate::config::Behaviour::Done => ("on_off_color", crate::pulse::SUCCESS_COLOR),
        crate::config::Behaviour::Failed => ("on_off_color", crate::pulse::FAILURE_COLOR),
        _ => return 0,
    };
    let body = serde_json::json!({
        "signaling": {
            "signal": signal,
            "duration": UNMAPPED_SIGNAL_DURATION_MS,
            "colors": [{"xy": {"x": color.x, "y": color.y}}],
        },
    })
    .to_string();
    for fixture in fixtures {
        bridge.put(&fixture.path(), &body);
    }
    fixtures.len()
}

/// How long the no-map pulse flashes, in milliseconds.
///
/// THREE SECONDS, AND IT IS NOT THE LOCKED FOUR. This is the body a machine with
/// no `[lights]` table sends. The duration and the no-brightness shape are kept
/// exactly as shipped; the color is not pinned here, it follows whatever
/// `SUCCESS_COLOR`/`FAILURE_COLOR` are locked to. The four-second figure was
/// locked on the ROUTED path, where a per-behaviour knob states it, and moving
/// this one would change what an unconfigured machine does without anybody
/// asking for it.
const UNMAPPED_SIGNAL_DURATION_MS: u64 = 3000;

/// The CLIP v2 bridge over ureq.
pub struct UreqBridge {
    pub base: String,
    pub key: String,
    /// How long ONE call may take. A FIELD rather than one constant, because
    /// the callers wait for different reasons: an unattended tick and the
    /// doctor can spend the full transport deadline, and a human standing at a
    /// terminal typing a mute cannot.
    pub deadline: Duration,
}

impl UreqBridge {
    fn agent(&self) -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_global(Some(self.deadline))
            .max_redirects(0)
            // The bridge serves a self-signed certificate for its own LAN
            // address, so verification is disabled here exactly as openhue
            // does it; there is no CA that could vouch for a Hue bridge.
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .disable_verification(true)
                    .build(),
            )
            .build()
            .new_agent()
    }
}

/// How long one bridge call may take. The pulse is decoration on a
/// notification, so it must never be what makes one slow.
pub const BRIDGE_DEADLINE: Duration = Duration::from_secs(10);

/// And how long one may take with a HUMAN waiting on it, which is the mute
/// command's inventory read and nothing else.
pub const TYPED_COMMAND_DEADLINE: Duration = Duration::from_secs(1);

impl Bridge for UreqBridge {
    fn get(&self, path: &str) -> Option<String> {
        self.agent()
            .get(format!("{}/{path}", self.base))
            .header("hue-application-key", &self.key)
            .call()
            .ok()?
            .body_mut()
            .read_to_string()
            .ok()
    }

    fn put(&self, path: &str, body: &str) {
        // Nothing reads the outcome: a pulse that did not land is not worth
        // failing, reporting or retrying on a notification path.
        let _ = self
            .agent()
            .put(format!("{}/{path}", self.base))
            .header("hue-application-key", &self.key)
            .content_type("application/json")
            .send(body);
    }
}
