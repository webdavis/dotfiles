use crate::*;

/// One upkeep pass: read the machine, derive the one state the house is in,
/// and write it to every lamp that should show it.
///
/// EXIT 0 ON EVERY PATH, and SILENT on every happy one. This runs three times
/// a minute forever under a daemon nobody is watching, so a line per tick is a
/// log the rotation job then rotates a real log out of.
///
/// EVERY STATE IS RE-DERIVED FROM SCRATCH. Nothing is carried between runs
/// except what is on disk, which is the daemon's own rule: this process exists
/// for a fraction of a second and the next one is a different process
/// entirely.
///
/// THE JOURNAL IS READ AND NEVER CLAIMED. `claim_journal` is how the replay
/// CONSUMES a queue; a tick that claimed it would delete the misses the
/// operator has not seen yet, which is the opposite of what the glow is for.
pub(crate) fn lights_tick() -> i32 {
    pns_adapters::turn_markers::sweep_turn_markers(&state_dir(), now_secs());
    let home = std::env::var("HOME").unwrap_or_default();
    // AN UNREADABLE CONFIG ASKED FOR NOTHING, which is the same reading the
    // event path takes of the lamps one function over: a file nobody could
    // parse routed no lamp, and a map this could not read must not be replaced
    // with a guess about which lamps carry what.
    let Ok(LoadOutcome::Loaded(config)) = load_config(&config_path(&home)) else {
        return 0;
    };
    // NO BRIDGE NAMED IS NO CLEAR EITHER, so held lamps KEEP their record here.
    // Hue switched off, or absent, is a machine this process cannot reach a
    // lamp on at all; forgetting the record would leave the lamp lit with
    // nothing in the system that knows about it, and the operator with the wall
    // switch. Keeping it means the tick that follows the switch going back on
    // still has a name to write the clear to.
    let Some(settings) = enabled_hue_table(&config) else {
        return 0;
    };
    let state = state_dir();
    let records = pns_adapters::SqliteStore::for_records(state.clone());
    let probes = system_probes();
    pns_application::MaintainLamps {
        records: &records,
        work: &pns_adapters::HerdrWork(SystemCommandRunner),
        markers: &pns_adapters::FileLampMarkers(state.clone()),
        claim: &pns_adapters::FileLampTick(state.clone()),
        jobs: &pns_adapters::FileJobSpool::new(state),
    }
    .run(
        config.lights.as_deref(),
        &probes,
        |refresh| {
            let hue = hue_settings(&settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref())?;
            Some(pns_adapters::TypedLampBridge(UreqBridge {
                base: format!("https://{}/clip/v2/resource", hue.bridge),
                key: hue.key,
                deadline: refresh.map(tick_bridge_deadline).unwrap_or(BRIDGE_DEADLINE),
            }))
        },
        pns_application::LampReadings {
            minutes: local_minutes_since_midnight,
            presence: || {
                presence_snapshot(
                    pns::config::parse_presence(&config).ok().flatten().as_ref(),
                    &probes,
                    pns::probes::IdleProbe::idle_secs(&probes),
                    pns::probes::ScreenLockProbe::screen_locked(&probes),
                    home_presence(),
                )
            },
            last_interaction: || pns_application::last_lamp_interaction(&system_probes()),
            interval: || {
                let started = std::time::Instant::now();
                (
                    move || u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    std::thread::sleep,
                )
            },
        },
        |line| eprintln!("{line}"),
    );
    0
}
