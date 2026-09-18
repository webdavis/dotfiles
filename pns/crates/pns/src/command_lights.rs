use crate::*;

pub(crate) fn lights_mode(verb: &str) -> i32 {
    match verb {
        "tick" => lights_tick(),
        "quiet" => lights_quiet(),
        "enroll" => crate::command_enroll::lights_enroll(),
        "pulse" => lights_pulse(),
        // UNKNOWN IS AN ERROR, never a silent fallthrough. Argv parsing on the
        // event path is deliberately lenient, so a bare `pns lights` reaching
        // it would skip the word it did not know and fire a notification about
        // an empty event.
        _ => {
            eprintln!("{LIGHTS_USAGE}");
            2
        }
    }
}

pub(crate) const LIGHTS_USAGE: &str = "pns: usage: pns lights tick | \
pns lights quiet [<place> [<duration>|off]] | \
pns lights pulse [<exit-code>] | \
pns lights enroll --bridge-id <id> | --allow-unverified";
/// The lamps' own mute: one place, quiet for a bounded while, by hand.
///
/// LIGHTS ONLY, and that is the operator's own scope: cards, banners, the
/// durable log and `pns quiet` are untouched, so an agent that needs an answer
/// still reaches the phone while the bedroom lamp stays out of it. The two
/// mutes share a duration parser and nothing else, and neither reads the
/// other's file.
///
/// FAIL OPEN AT EVERY TURN, which is `quiet.rs`'s direction rather than the
/// window's: a state file nobody can parse mutes NOTHING and says so, because a
/// lights mute the operator cannot see is worse than a lamp that flashed.
///
/// THE READ-MODIFY-WRITE RACE IS REAL AND ACCEPTED. This is hand-typed, so two
/// runs racing means an operator typing two commands in the same second, and
/// the loser is one mute they can see is missing and retype. A lock between two
/// interactive commands would be a mechanism with no reader.
fn lights_quiet() -> i32 {
    let arguments: Vec<String> = crate::arguments_after_verb();
    let home = std::env::var("HOME").unwrap_or_default();
    let loaded = load_config(&config_path(&home));
    let known = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => config
            .lights
            .as_deref()
            .map(|lights| {
                pns_application::quiet_names(lights, &arguments, || {
                    pns_adapters::bridge_inventory(config)
                })
            })
            .unwrap_or_default(),
        // A CONFIG THIS CANNOT READ NAMES NO PLACE, so every mute is refused by
        // name rather than stored against a map nobody could load. The report
        // still runs, which is what an operator with a broken config needs from
        // this command first.
        _ => Vec::new(),
    };
    let state = state_dir();
    let now = now_secs();
    // HOW LONG A BARE MUTE LASTS, off the operator's OWN schedule rather than
    // any one room's dim window: a mute typed at bedtime is about their night.
    // A window nobody can parse states no schedule, which the refusal covers.
    let until_quiet_ends = pns_domain::lights::mute::bare_mute_secs(
        match &loaded {
            Ok(LoadOutcome::Loaded(config)) => enabled_hue_table(config)
                .and_then(|settings| quiet_window(&settings).ok().flatten())
                .map(|window| window.ends_at()),
            _ => None,
        },
        now.and_then(local_minutes_since_midnight),
    );
    let command = match crate::quiet_command(&arguments, &known, until_quiet_ends) {
        Ok(command) => command,
        Err(refusal) => {
            eprintln!("{refusal}");
            eprintln!("{LIGHTS_USAGE}");
            return 2;
        }
    };
    match (pns_application::SetLightsQuiet {
        mutes: &pns_adapters::SqliteStore::for_records(state),
    })
    .run(&command, now, |warning| eprintln!("{warning}"))
    {
        Ok(lines) => {
            let paint = crate::style::Paint::for_stdout();
            for line in crate::style::header(
                paint,
                "pns lights quiet",
                &[crate::style::HeaderLine {
                    label: "Scope",
                    text: "the lamps only; cards, banners and `pns quiet` are untouched",
                }],
            ) {
                println!("{line}");
            }
            println!();
            println!(
                "{}",
                crate::style::heading(
                    paint,
                    "Quiet now",
                    "which lamps are muted, and for how long"
                )
            );
            println!();
            for line in lines {
                println!(
                    "{}",
                    crate::style::row(paint, crate::style::Tone::Quiet, "\u{b7}", 2, &line)
                );
            }
            0
        }
        Err(refusal) => {
            eprintln!("{refusal}");
            1
        }
    }
}

/// `pns lights pulse <exit-code>`: read the hue table and signal the bridge
/// with the exit code it was handed. Every absence is a silent exit 0.
///
/// NOTHING IN THIS REPO CALLS IT. The tiers that used to are part of the event
/// plan now, which is what stopped the tier being decided twice; this stays as
/// the operator's own command for signalling the lights by hand, and for
/// checking that a bridge and key in the config actually work. It ignores
/// `hue.quiet_hours` on purpose: the gate lives at the event path's call site
/// in `fire_pulse_unless_quiet`, so a hand-run pulse still lights the room
/// inside the window, which is what keeps the window checkable while it is on.
///
/// THE WORD IS READ BEFORE THE CONFIG LOADS. `lights pulse --help` used to load
/// the config first: with none it silently exited 0 having printed nothing, and
/// with one it pulsed the room red, because a non-numeric word was read as a
/// failing exit code. Reading the word first means `--help` and a bad code
/// both answer with no machine read at all.
fn lights_pulse() -> i32 {
    // THE WHOLE TAIL IS READ, not just the word right after `pulse`: H-B
    // requires help to win in flag position anywhere, and an unknown extra
    // word to be refused rather than silently dropped.
    let tail: Vec<String> = crate::arguments_after_verb();
    if tail.iter().any(|token| crate::legacy::is_help_flag(token)) {
        println!("{PULSE_USAGE}");
        return 0;
    }
    if tail.len() > 1 {
        eprintln!("{PULSE_USAGE}");
        return 2;
    }
    let word = tail.first().cloned().unwrap_or_default();
    let Some(behaviour) = pns_domain::pulse::exit_behaviour(&word) else {
        eprintln!("{PULSE_USAGE}");
        return 2;
    };
    let home = std::env::var("HOME").unwrap_or_default();
    // FAIL CLOSED, unlike an event. The roster fallback that keeps every
    // notification working through a broken config is an EVENT-mode rule:
    // applying it here would let an unrelated typo switch a deliberately
    // disabled pulse back on. The pulse runs only when its own table says
    // enabled, explicitly.
    let config = match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => config,
        // Absent is not a mistake; never opting in earns no warning.
        Ok(LoadOutcome::Missing) => return 0,
        Err(error) => {
            // The sanitized detail event mode prints, with the outcome THIS
            // mode had: there is no recoverable setting to fall back to, so
            // nothing pulses.
            eprintln!("pns: config error ({}); no pulse", error.detail());
            return 0;
        }
    };
    fire_pulse(enabled_hue_table(&config), behaviour);
    0
}

pub(crate) const PULSE_USAGE: &str = "pns: usage: pns lights pulse [<exit-code>] | \
pns lights pulse --help, -h (a bare `pulse` is a success pulse)";

/// What `pns pulse` answers now: the verb that replaced it, and no pulse.
pub(crate) fn retired_pulse() -> i32 {
    eprintln!("pns: pulse is now a verb: run `pns lights pulse <exit-code>`");
    eprintln!("{PULSE_USAGE}");
    2
}
