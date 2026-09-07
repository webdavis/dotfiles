//! The bridge transport and the writes addressed to its fixtures.

use super::{clear_body, grouped_light_ids_for_rooms, inventory};
use crate::channels::hue::{Fixture, Muting, Routing, resolve};
use std::time::Duration;

/// The bridge seam: authenticated GETs and PUTs against the CLIP paths.
pub trait Bridge {
    fn get(&self, path: &str) -> Option<String>;
    /// Fire and forget: `run` discards every outcome, so a bridge that
    /// refuses tells no one. Returning a result would be a seam with no
    /// consumer.
    fn put(&self, path: &str, body: &str);
}

/// The CLIP resource path this fixture is written to.
///
/// WHICH IS THE WHOLE POINT OF THE DISTINCTION. Addressing either as the
/// other is a PUT to a resource id of the wrong type, which the bridge
/// answers by doing nothing and telling no one, because `put` is fire and
/// forget.
pub fn fixture_path(fixture: &Fixture) -> String {
    match fixture {
        Fixture::Grouped(id) => format!("grouped_light/{id}"),
        Fixture::Light(id) => format!("light/{id}"),
    }
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
        bridge.put(&fixture_path(fixture), &body);
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
