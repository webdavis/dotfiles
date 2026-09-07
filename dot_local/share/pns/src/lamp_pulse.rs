use crate::*;

/// The event path's pulse, which the lights' own quiet window may mute.
///
/// THE GATE LIVES HERE, at the call site, and not in `fire_pulse` below:
/// `pns pulse` shares that function and is deliberately exempt, because the
/// hand-run pulse is how a bridge and key are checked and gating it would make
/// the quiet window untestable exactly while it is on. Inside the `if` that
/// already earned a pulse, so a refusal is printed only where a room would
/// otherwise have lit.
pub(crate) fn fire_pulse_unless_quiet(
    hue_table: Option<toml::Table>,
    lights: Option<&pns::config::Lights>,
    behaviour: pns::config::Behaviour,
    presence: Option<&pns::presence_policy::Snapshot>,
) {
    // No table is nothing to quiet: an operator who never enabled the lights
    // gets the same silence `fire_pulse` would have given them.
    let Some(settings) = hue_table else {
        return;
    };
    // FRESH, not the run's start: the legs above dial the network under their
    // own deadlines, so a run can cross into a dim window between starting and
    // reaching the moment a lamp would actually light, and the older reading
    // would flash it just inside quiet hours. HONEST LIMIT: no suite pins the
    // freshness, because a test's clock does not advance mid-run.
    let now = now_secs();
    let minutes_now = now.and_then(local_minutes_since_midnight);
    let Some(lights) = lights else {
        // TODAY'S PATH, UNCHANGED, and it is the compatibility claim of this
        // whole change: one house window for the whole pulse, one write per room
        // in `[plugins.hue] rooms`, and one refusal that costs the pulse when
        // nobody can read the window. A machine that never wrote a `[lights]`
        // table reaches nothing new.
        match quiet_window(&settings) {
            Ok(window) => {
                if !quiet_now(window.as_ref(), minutes_now) {
                    fire_pulse(Some(settings), behaviour);
                }
            }
            // FAIL CLOSED, the direction the pulse takes on every unreadable
            // reading: a window nobody can parse is an operator who asked for
            // quiet hours and cannot be told which ones, so the room stays
            // dark and the refusal says why.
            Err(refusal) => eprintln!("{refusal}"),
        }
        return;
    };
    // THE OPERATOR'S OWN AD-HOC QUIET, read here rather than inside the walk
    // for the reason every reading on this path is: the modules take no files
    // and no clock, and the composition root decides where a complaint goes.
    // A machine that has never typed the command reads no file and pays one
    // failed open.
    let state = state_dir();
    let (muted, mut complaints) = ad_hoc_quiet(&state, now);
    complaints.extend(fire_lights(
        &settings,
        lights,
        behaviour,
        &pns::channels::hue::Reading {
            minutes_now,
            muted: &muted,
        },
        held_lamps(&state).as_deref(),
        presence,
    ));
    // SAY-ONCE, NOT ONCE PER EVENT. A state file something else corrupted stays
    // corrupt until a human fixes it, and this path fires many times a session,
    // so a bare print here is one stderr line per hook invocation forever.
    //
    // AND IT CARRIES THE RESOLUTION'S OWN FINDINGS TOO, which used to be
    // discarded here. A machine whose map routes only `done` and `failed` holds
    // no state, so its tick never resolves anything and never complains: a
    // mistyped lamp name on such a config was dark forever with the whole
    // system silent about it, and this is the path that meets it.
    say_lights_once(&state, &complaints, LIGHTS_QUIET_SAID);
}
/// The ROOM-BASED lights signal, from whichever mode asked for it, and how many
/// rooms it reached. Both notification callers discard the count; the hand-run
/// check is what it exists for, since the bridge acknowledges no write and a
/// room that was addressed is the last observable fact on this path.
///
/// `[plugins.hue] rooms` IS THE PATH WITHOUT A `[lights]` TABLE, and it is also
/// `pns pulse`'s path with one. That is deliberate: the hand-run pulse is the
/// bridge-and-key check, not a feature, and keeping it room-based means it
/// stays one write to one obvious place while the routing map grows.
pub(crate) fn fire_pulse(
    hue_table: Option<toml::Table>,
    behaviour: pns::config::Behaviour,
) -> usize {
    let Some(hue) = hue_table.and_then(|settings| {
        hue_settings(&settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref())
    }) else {
        return 0;
    };
    HuePulse {
        bridge: UreqBridge {
            base: format!("https://{}/clip/v2/resource", hue.bridge),
            key: hue.key,
            deadline: BRIDGE_DEADLINE,
        },
        rooms: hue.rooms,
    }
    .run(behaviour)
}
/// The ROUTED lights signal: resolve the map on the bridge, then flash every
/// lamp routed for this pulse that nothing is currently holding.
///
/// THE HELD RECORD IS THE GATE, and it is the TICK'S record read here rather
/// than a held state re-derived on this path. One writer and one reader, at the
/// cost of up to one refresh interval of staleness: a lamp that took a held
/// state a second ago may still flash once, and a lamp whose state ended a
/// second ago may skip one flash. Re-deriving it here would mean two processes
/// each deciding what the house is holding, from readings taken at different
/// moments, which is the divergence this crate keeps paying for.
fn fire_lights(
    settings: &toml::Table,
    lights: &pns::config::Lights,
    behaviour: pns::config::Behaviour,
    reading: &pns::channels::hue::Reading<'_>,
    held: Option<&[String]>,
    presence: Option<&pns::presence_policy::Snapshot>,
) -> Vec<String> {
    let Some(hue) = hue_settings(settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref()) else {
        return Vec::new();
    };
    let bridge = UreqBridge {
        base: format!("https://{}/clip/v2/resource", hue.bridge),
        key: hue.key,
        deadline: BRIDGE_DEADLINE,
    };
    run_pulse_writes(
        &bridge,
        &state_dir(),
        lights,
        behaviour,
        reading,
        held,
        presence,
    )
}
/// The event path's routed writes: one pulse body per lamp the behaviour is
/// routed for, with the mute and the TICK'S held record each answered at the
/// per-lamp decision, once.
///
/// IT ANSWERS WITH THE RESOLUTION'S COMPLAINTS rather than printing or dropping
/// them. This path resolves the map on every pulse, so it is where a mistyped
/// name on a pulse-only config is met; the caller owns the say-once memory.
///
/// A HELD RECORD OF `None` IS EVERY LAMP HELD, which is the fail-dark direction
/// on the one gate that decides whether a blink writes over a breath. Read as
/// nothing held, an unreadable record let the pulse flash straight over a lamp
/// that was breathing about a question.
fn run_pulse_writes<B: pns::channels::hue::Bridge>(
    bridge: &B,
    state: &Path,
    lights: &pns::config::Lights,
    behaviour: pns::config::Behaviour,
    reading: &pns::channels::hue::Reading<'_>,
    held: Option<&[String]>,
    presence: Option<&pns::presence_policy::Snapshot>,
) -> Vec<String> {
    // A BRIDGE THAT ANSWERED NOTHING RESOLVES NOTHING, and says nothing here.
    // The doctor is where an unreachable bridge is reported; a warning on every
    // notification for the rest of a machine's life is noise.
    let Some(routing) = pns::channels::hue::resolve_on_bridge(bridge, lights) else {
        return Vec::new();
    };
    // THE COMPLAINTS COME OFF THE WHOLE RESOLUTION, before the narrowing: a
    // lamp name the bridge could not answer is a typo in the config whether or
    // not the operator is standing in that room.
    let mut routing = routing;
    let complaints = routing_complaints(&routing);
    // WHAT THIS EVENT WOULD ACTUALLY WRITE, decided ONCE and used twice: as
    // the set presence narrows over, and as the write itself. It answers the
    // whole per-lamp question, the mute, the routing, the held record and the
    // dim window together, because "eligible" has to mean "would light" or the
    // narrowing's own fallback is judging the wrong set.
    let write_for = |routed: &pns::channels::hue::Routed| -> Option<(String, String)> {
        let path = pns::channels::hue::fixture_path(&pns::channels::hue::Fixture::Light(
            routed.lamp.id.clone(),
        ));
        let lamp_is_held = held.is_none_or(|held| held.contains(&path));
        if pns::channels::hue::muted_now(&routed.lamp, reading.muted)
            || !pns::lights::pulse_fires(&routed.shows, behaviour, lamp_is_held)
        {
            return None;
        }
        let showing =
            pns::channels::hue::dim_showing(routed.dim.as_ref(), behaviour, reading.minutes_now);
        let (color, pulse, brightness) =
            pns::channels::hue::pulse_render(behaviour, lights, showing)?;
        Some((
            path,
            pns::channels::hue::pulse_body(&pulse, color, brightness),
        ))
    };
    // NARROWED OVER THE ELIGIBLE SET AND NOT THE WHOLE ONE. A room holding a
    // lamp that carries some OTHER behaviour is a room this event lights
    // nothing in, so narrowing to it and filtering afterwards produced exactly
    // the silence the fallback exists to prevent.
    routing.lamps.retain(|routed| write_for(routed).is_some());
    let routing = narrow_to_presence(state, routing, presence);
    for routed in &routing.lamps {
        if let Some((path, body)) = write_for(routed) {
            bridge.put(&path, &body);
        }
    }
    complaints
}
/// What one resolution has to say for itself: every declared name the bridge
/// could not answer, and every declaration it refused.
///
/// ONE WORDING FOR BOTH READERS, the tick's and the event path's, because a
/// typo reported in two spellings is two entries in two say-once memories and
/// an operator reading the same problem twice.
///
/// `pns ` AND NOT `pns lights: `, because every sentence already begins
/// `lights: ` (the doctor prefixes the same sentences `pns doctor: `).
pub(crate) fn routing_complaints(routing: &pns::channels::hue::Routing) -> Vec<String> {
    routing
        .unresolved
        .iter()
        .map(|missing| format!("pns {}", pns::channels::hue::missing_sentence(missing)))
        .chain(
            routing
                .refusals
                .iter()
                .map(|refusal| format!("pns {refusal}")),
        )
        .collect()
}

#[cfg(test)]
#[path = "lamp_pulse/tests.rs"]
mod lamp_pulse_tests;
