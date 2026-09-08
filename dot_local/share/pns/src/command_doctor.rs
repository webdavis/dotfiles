use crate::*;
use pns_application::DOCTOR_OPENING;

/// The `doctor` mode: one test send through every enabled channel, and one
/// line per REGISTERED plugin about what happened.
///
/// EVERY SUPPRESSION GATE IS BYPASSED, and structurally rather than by a flag.
/// `decide()` is never called, so the presence verdict, the viewed-pane rule
/// and the two phone overrides have nothing to say here; the mute is read in
/// `run_event`, which this is not on; and the pulse goes through `fire_pulse`,
/// the hand-run path `pns pulse` uses, so the lights' quiet window never sees
/// it either. A check that can be suppressed proves nothing about the channel
/// it was checking, and every one of those gates exists to stop a destination
/// receiving.
///
/// THE CENSUS IS THE WHOLE ROSTER, never the selection: a plugin the config
/// left off has to be VISIBLY absent by choice, or the report answers "what is
/// on" when the operator asked "what will reach me".
///
/// EVERY SEND GOES THROUGH THE ENGINE'S OWN WIRING, down to the constructors
/// and `dispatch_legs`, so a doctor cannot report green through a path an
/// event would not use.
pub(crate) fn doctor_mode() -> i32 {
    // ANY EXTRA WORD IS A REFUSAL, before anything is sent or printed. A
    // doctor that quietly ignored an argument is a check the operator believes
    // was narrower or wider than it was.
    if std::env::args_os().nth(2).is_some() {
        eprintln!("{DOCTOR_USAGE}");
        return 2;
    }
    println!("{DOCTOR_OPENING}");

    let home = std::env::var("HOME").unwrap_or_default();
    let loaded = load_config(&config_path(&home));
    // The same readings `run_event` takes off the same config, before
    // selection consumes it.
    let (
        hue_table,
        mobile,
        hermes_key,
        replay_card,
        focus_silence,
        daemon_enabled,
        nag_after_secs,
        lights,
        hue_declared,
    ) = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => (
            enabled_hue_table(config),
            read_mobile(config),
            plugin_settings(config, "hermes").and_then(hermes_secret),
            config.recap.replay_card,
            config.focus_silence.clone(),
            config.daemon_enabled,
            config.nag_after_secs,
            config.lights.clone(),
            // WHETHER THE TABLE WAS WRITTEN AT ALL, which
            // `enabled_hue_table` cannot say: it answers `None` both for a
            // table nobody wrote and for one whose switch is off, and the
            // lamps' report tells those two apart.
            config.plugins.contains_key("hue"),
        ),
        // THE SWITCH FALLS BACK ON, which is the fallback `run_event` takes
        // for the same reading. The two must agree or the doctor describes a
        // delivery the event would not make, and the Focus list falls back
        // EMPTY here for the same reason it does there.
        // AND THE NAG FALLS BACK OFF, which is the fallback `nag_after_secs`
        // takes for the same reading: the two must agree or the doctor
        // describes a schedule the fire would not keep.
        _ => (
            None,
            Mobile::default(),
            None,
            true,
            Vec::new(),
            true,
            NAG_OFF,
            None,
            false,
        ),
    };
    // THE SWITCHED-OFF TABLES THE EVENT PATH SAYS NOTHING ABOUT, said here
    // and only here: see `disabled_backend_warning`.
    if let Ok(LoadOutcome::Loaded(config)) = &loaded {
        for warning in disabled_backend_warnings(config) {
            eprintln!("{warning}");
        }
    }
    // THE ROOM SENSOR'S OWN SETTINGS, read here because the census below has
    // one line to print about them. A refusal is LOUD and leaves the reading
    // absent rather than half-honoured.
    let presence_settings = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => match pns::config::parse_presence(config) {
            Ok(settings) => settings,
            Err(error) => {
                eprintln!(
                    "pns: config error ({}); the room sensor is unread",
                    error.detail()
                );
                None
            }
        },
        _ => None,
    };
    let registry = roster();
    // WHAT LOADING FOUND, taken BEFORE `select_plugins` consumes it: the
    // census reports a plugin the selection left out, and which sentence is
    // true of that depends entirely on whether there was a config to read.
    let config_state = match &loaded {
        Ok(LoadOutcome::Loaded(_)) => pns_domain::doctor::ConfigState::Read,
        Ok(LoadOutcome::Missing) => pns_domain::doctor::ConfigState::Absent,
        Err(_) => pns_domain::doctor::ConfigState::Unreadable,
    };
    // THE CONFIG FALLBACK IS INHERITED ON PURPOSE. `select_plugins` is what an
    // event would run and warn about, and the doctor's job is to say what an
    // event would do, not what a tidier engine would do.
    let (selection, warning) = select_plugins(&registry, loaded);
    if let Some(warning) = warning {
        eprintln!("{warning}");
    }
    let checks = pns_domain::doctor::checks(&registry.all(), &selection, config_state);

    pns_application::RunDoctor {
        checks: &checks,
        records: &pns_adapters::FileRecords::new(state_dir()),
        clock: &now_secs,
        replay_card,
        nag_after_secs,
    }
    .run(
        pns_application::DoctorActions {
            deliver: |legs: &[pns_domain::routing::Leg], event: &pns_domain::EventArgs| {
                dispatch_legs(legs, false, event, &home, &mobile, hermes_key)
            },
            pulse: || {
                pns_application::doctor_pulse(
                    pns_adapters::hue_resolves(hue_table.as_ref()),
                    || fire_pulse(hue_table.clone(), pns::config::Behaviour::Done),
                )
            },
            presence: || {
                (
                    presence_status(presence_settings.as_ref()),
                    last_narrowing(&state_dir()),
                )
            },
            pairing: pns_adapters::read_pairing,
            focus: || {
                pns_application::doctor_focus(!focus_silence.is_empty(), || {
                    pns_adapters::focus_now(&home, &focus_silence).map_err(|error| error.kind())
                })
            },
            daemon: || {
                let state = state_dir();
                let beat = pns_adapters::daemon_heartbeat(&state);
                pns_domain::doctor::daemon_line(
                    daemon_enabled,
                    beat,
                    now_secs(),
                    pns_adapters::job_spool::job_count(&state),
                )
            },
            lamps: || {
                pns_application::doctor_lamps(lights.as_deref(), || {
                    pns_adapters::doctor_bridge(hue_table.as_ref(), hue_declared)
                })
            },
        },
        |line| println!("{line}"),
    )
}
/// What a doctor typed wrong is told. ONE WORD AND NO FLAGS: a namespace built
/// for callers that do not exist makes the common case longer to type, and the
/// report absorbs a new section without a new spelling.
const DOCTOR_USAGE: &str = "pns: usage: pns doctor";
