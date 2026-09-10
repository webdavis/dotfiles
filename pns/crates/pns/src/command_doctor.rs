use crate::*;

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
/// and the destination registry, so a doctor cannot report green through a path an
/// event would not use.
pub(crate) fn doctor_mode() -> i32 {
    // ANY EXTRA WORD IS A REFUSAL, before anything is sent or printed. A doctor
    // that quietly ignored an argument is a check the operator believes was
    // narrower or wider than it was. `--no-color` never reaches here: it is
    // tool-wide, so the dispatcher takes it out of argv and remembers it, which
    // is what lets this stay a plain refusal of everything.
    // ONE WORD IS ACCEPTED AND EVERY OTHER IS A REFUSAL, before anything is
    // sent or printed. A doctor that quietly ignored an argument is a check the
    // operator believes was narrower or wider than it was.
    let decisions = match crate::arguments_after_subcommand().as_slice() {
        [] => pns_domain::doctor::Detail::Spoken,
        [only] if only == RAW_FLAG => pns_domain::doctor::Detail::Raw,
        _ => {
            eprintln!("{DOCTOR_USAGE}");
            return 2;
        }
    };
    let mut report = doctor_style::Report::new(style::Paint::for_stdout());
    print_lines(report.open());

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
        Ok(LoadOutcome::Loaded(config)) => match pns_adapters::parse_presence(config) {
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

    let code = pns_application::RunDoctor {
        decisions,
        checks: &checks,
        records: &pns_adapters::SqliteStore::for_records(state_dir()),
        clock: &now_secs,
        replay_card,
        nag_after_secs,
    }
    .run(
        pns_application::DoctorActions {
            deliver: |legs: &[pns_domain::routing::Leg], event: &pns_domain::EventArgs| {
                let destinations =
                    channel_dispatch::destinations(&selection, "", &home, &mobile, hermes_key);
                let identity = match delivery_runtime::fresh_identity() {
                    Ok(identity) => identity,
                    Err(_) => {
                        return legs
                            .iter()
                            .map(|leg| {
                                (
                                    *leg,
                                    Delivery::Unlaunched("delivery identity unavailable".into()),
                                )
                            })
                            .collect();
                    }
                };
                let rendered = channel_dispatch::rendered_event(event, false);
                legs.iter()
                    .map(|leg| {
                        let request = pns_application::DeliveryRequest {
                            producer: &identity.producer,
                            request_id: Some(&identity.request_id),
                            event: &rendered,
                            route: "",
                            mode: leg.mode,
                        };
                        (
                            *leg,
                            pns_application::deliver_guarded(leg.name, || {
                                destinations.deliver(leg.name, &request)
                            }),
                        )
                    })
                    .collect()
            },
            pulse: || {
                pns_application::doctor_pulse(
                    pns_adapters::hue_resolves(hue_table.as_ref()),
                    || {
                        fire_pulse(
                            hue_table.clone(),
                            pns_domain::lamps::config::Behaviour::Done,
                        )
                    },
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
            delivery_health: || {
                pns_adapters::SqliteStore::new(state_dir())
                    .delivery_health()
                    .map_err(|_| "delivery ledger unreadable".into())
            },
            routes: || {
                // The ledger is the only roster pns has: a producer names a
                // route at call time and nothing else writes it down. An
                // unreadable ledger reports NO routes rather than inventing
                // one, and the summary then says nothing has been posted yet,
                // which is the honest reading of an empty list either way.
                let posted = pns_adapters::SqliteStore::new(state_dir())
                    .posted_routes()
                    .unwrap_or_default();
                let base = std::env::var("PNS_HERMES_URL")
                    .unwrap_or_else(|_| pns_adapters::DEFAULT_HERMES_URL.to_string());
                // The SAME client an event's hermes leg posts through, so a
                // probe cannot succeed on a path a delivery would not take.
                pns_adapters::probe_routes(&pns_hermes::UreqSignedPost, &base, &posted)
            },
            imports: || {
                pns_adapters::SqliteStore::for_records(state_dir())
                    .import_failures()
                    .map_err(|error| error.to_string())
            },
            lamps: || {
                pns_application::doctor_lamps(lights.as_deref(), || {
                    pns_adapters::doctor_bridge(hue_table.as_ref(), hue_declared)
                })
            },
        },
        |item| print_lines(report.item(&item)),
    );
    print_lines(report.close());
    code
}

/// The report's own lines, as they are produced.
fn print_lines(lines: Vec<String>) {
    for line in lines {
        println!("{line}");
    }
}
/// What a doctor typed wrong is told. ONE FLAG AND NO NAMESPACE: a namespace
/// built for callers that do not exist makes the common case longer to type,
/// and the report absorbs a new section without a new spelling. The one flag
/// earns its place because a report that reaches a file or a pipe wants plain
/// text and the automatic detection cannot see through a pty.
const DOCTOR_USAGE: &str = "pns: usage: pns doctor [--raw]";

/// The one argument the doctor takes: every input behind each recorded
/// decision, instead of the sentence the report says them in.
const RAW_FLAG: &str = "--raw";
