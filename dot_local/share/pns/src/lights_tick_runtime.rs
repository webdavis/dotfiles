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
    // THE FEATURE BEING OFF STILL PUTS A HELD LAMP OUT. `[lights]` removed, or a
    // clock this machine cannot read, is a tick that can arm nothing; the
    // bridge above is still named, so the one thing it can still do is put out
    // what the last tick was holding and forget it.
    // ONE PROBE SET FOR THE WHOLE TICK, and its clock is the tick's `now`.
    // Every reading the narrowing takes below comes off this set's memoized
    // cells, so the reading, the age it is judged by and the moment the
    // decision is stamped with cannot straddle a boundary.
    let probes = system_probes();
    let (Some(lights), Some(now)) = (config.lights.as_deref(), probes.now_secs()) else {
        clear_held_lamps(Some(&settings));
        return 0;
    };
    // AND CREDENTIALS THAT ARE GONE KEEP THE RECORD for the reason the hue
    // switch does: nothing here can address a lamp.
    let Some(hue) = hue_settings(&settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref())
    else {
        return 0;
    };
    let state = state_dir();
    sweep_legacy_state(&state);
    let standing = lights_house(&state, lights, now);
    let (muted, mut complaints) = ad_hoc_quiet(&state, Some(now));
    // A RECORD THIS CANNOT READ NAMES NOTHING TO CLEAR, and the tick is its
    // only writer, so it goes on: the pass below publishes the record it
    // derived, which is what repairs the file. The residue is stated: a lamp
    // held under a name this run could not read stays lit until the repaired
    // record names it again or the operator's next return clears it.
    // ONE READ FOR BOTH THE BARE GATE AND THE PHASE A RESUMED BREATH NEEDS,
    // rather than two: `held_lamps` is `read_held` with the phase dropped, and
    // reading the record twice here would be two disk reads of one fact this
    // tick only ever reads once.
    let held_before_entries = read_held(&state);
    let held_before: Option<Vec<String>> = held_before_entries
        .as_deref()
        .map(|entries| entries.iter().map(|entry| entry.path.clone()).collect());
    if held_before.is_none() {
        complaints.push(HELD_RECORD_UNREADABLE.to_string());
    }
    let active = pns::lights::active_held(&standing.house);
    // NOTHING TO LIGHT AND NOTHING TO PUT OUT IS NO BRIDGE CALL AT ALL, which
    // is what keeps an idle machine off the network several times a minute.
    //
    // THE GATE IS THE HOUSE STATE ALONE, and that is a deliberate narrowing from
    // the shipped one. The old gate also asked whether any place could be awake,
    // which took the quiet-hours chain out of the config with no bridge listing
    // to judge it against and paid for it with two stated limits; the dim window
    // is now a per-lamp answer that needs the listing anyway, so the cheap half
    // of that question no longer exists. A house holding nothing still costs
    // nothing, which is the case that matters.
    if !active.is_empty() || held_before.as_deref().is_none_or(|held| !held.is_empty()) {
        // THE ONE MONOTONIC CLOCK THE WHOLE TICK IS MEASURED ON, started here
        // and read by nothing else: the resolve's cost, every fade's due
        // millisecond and the moment each write actually happened are all
        // offsets from this instant, so they can never disagree about when the
        // tick began. It is a parameter for the reason the sleeper is one: the
        // driver fills its whole interval by design, so a test that read the
        // real clock would live the interval too.
        let started = std::time::Instant::now();
        complaints.extend(run_tick_writes(
            &UreqBridge {
                base: format!("https://{}/clip/v2/resource", hue.bridge),
                key: hue.key,
                // THE CHILD IS BOUNDED BY ITS OWN INTERVAL, and the resolve is
                // the part of it that is not this process's to shorten: three
                // calls at the transport's ten seconds outlive every interval
                // the config permits, so a wedged bridge would have tick after
                // tick piling up, each one still dialling. A quarter of the
                // interval apiece leaves the fades the rest of it, and a bridge
                // on the same LAN answers these in milliseconds.
                deadline: tick_bridge_deadline(lights.refresh_secs),
            },
            &state,
            lights,
            &active,
            &pns::channels::hue::Reading {
                minutes_now: local_minutes_since_midnight(now),
                muted: &muted,
            },
            held_before_entries.as_deref(),
            now.saturating_mul(1000),
            // THE TICK TAKES ITS OWN READINGS, because it decides no event and
            // so inherits nobody's, and it takes them off the ONE probe set
            // this tick built, whose clock read is the `now` above.
            presence_snapshot(
                pns::config::parse_presence(&config).ok().flatten().as_ref(),
                &probes,
                pns::probes::IdleProbe::idle_secs(&probes),
                pns::probes::ScreenLockProbe::screen_locked(&probes),
                home_presence(),
            )
            .as_ref(),
            || u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            std::thread::sleep,
        ));
    }
    // AND THE SAYING IS OUTSIDE THAT GATE, deliberately. `say` FORGETS a
    // complaint that has cleared, and a complaint clears exactly when the house
    // goes dark; leaving the bookkeeping inside the gate meant a remembered
    // complaint was never forgotten on the tick that ended it, so the same
    // complaint returning later would not read as news.
    say_lights_once(&state, &complaints, LIGHTS_SAID);
    // AND THE TICK KEEPS ITSELF ALIVE while anything could still light a lamp.
    // Its lease was refreshed by EVENTS alone, which reaches only the states an
    // event ARRIVES with: a shell command produces no events at all, and the
    // automatic loop trigger is five minutes by default and six on the
    // operator's own machine, both PAST the five-minute lease an event leaves.
    // So the one lamp whose whole job is a long run could never arm itself, and
    // a lease taken by hand in a pane that then went quiet expired unread.
    //
    // IT IS STILL BOUNDED BY THE CONDITION, not a self-perpetuating job: a
    // house holding nothing with no run and no lease renews nothing, so an idle
    // machine's tick lapses exactly as it did.
    if !active.is_empty() || standing.in_flight {
        schedule_lights_tick(&state, lights, now, ORDINARY_LEASE_SECS);
    }
    0
}
/// Where a tick remembers what it last complained about.
pub(crate) const LIGHTS_SAID: &str = "lights-said";

/// What a tick says about a held record it could not read at all.
///
/// THE TICK GOES ON, because it is the file's only writer: it names no lamp to
/// clear, derives the states it wants and publishes a record for them, which is
/// what repairs an unreadable file. Where the path cannot be WRITTEN either, the
/// publish refuses and nothing is armed, which is the second sentence the
/// operator gets.
const HELD_RECORD_UNREADABLE: &str = "pns lights: the held record could not be read, \
so no lamp can be put out by name";
