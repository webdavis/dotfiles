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
    lights: Option<&pns_domain::lamps::config::Lights>,
    behaviour: pns_domain::lamps::config::Behaviour,
    presence: Option<&pns_domain::Snapshot>,
) {
    // No table is nothing to quiet: an operator who never enabled the lights
    // gets the same silence `fire_pulse` would have given them.
    let Some(settings) = hue_table else {
        return;
    };
    pns_application::signal_after_delivery(
        &now_secs,
        local_minutes_since_midnight,
        lights,
        || quiet_window(&settings),
        || {
            fire_pulse(Some(settings.clone()), behaviour);
        },
        |lights, now, minutes| {
            pns_application::signal_mapped(
                &pns_adapters::SqliteStore::for_records(state_dir()),
                now,
                minutes,
                |reading, held| fire_lights(&settings, lights, behaviour, reading, held, presence),
                |line| eprintln!("{line}"),
            )
        },
        |line| eprintln!("{line}"),
    );
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
    behaviour: pns_domain::lamps::config::Behaviour,
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
    lights: &pns_domain::lamps::config::Lights,
    behaviour: pns_domain::lamps::config::Behaviour,
    reading: &pns_domain::lamps::Reading<'_>,
    held: Option<&[String]>,
    presence: Option<&pns_domain::Snapshot>,
) -> Vec<String> {
    let Some(hue) = hue_settings(settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref()) else {
        return Vec::new();
    };
    let bridge = UreqBridge {
        base: format!("https://{}/clip/v2/resource", hue.bridge),
        key: hue.key,
        deadline: BRIDGE_DEADLINE,
    };
    pns_application::SignalLamps {
        bridge: &pns_adapters::TypedLampBridge(&bridge),
        presence: &pns_adapters::SqliteStore::for_records(state_dir()),
    }
    .run(lights, behaviour, reading, held, presence)
}
#[cfg(test)]
fn run_pulse_writes<B: pns_adapters::Bridge>(
    bridge: &B,
    state: &Path,
    lights: &pns_domain::lamps::config::Lights,
    behaviour: pns_domain::lamps::config::Behaviour,
    reading: &pns_domain::lamps::Reading<'_>,
    held: Option<&[String]>,
    presence: Option<&pns_domain::Snapshot>,
) -> Vec<String> {
    pns_application::SignalLamps {
        bridge: &pns_adapters::TypedLampBridge(bridge),
        presence: &pns_adapters::SqliteStore::for_records(state.to_path_buf()),
    }
    .run(lights, behaviour, reading, held, presence)
}
#[cfg(test)]
#[path = "lamp_pulse/tests.rs"]
mod lamp_pulse_tests;
