use crate::*;

/// The `pulse` mode: read the hue table and signal the bridge with the exit
/// code it was handed. Every absence is a silent exit 0.
///
/// NOTHING IN THIS REPO CALLS IT. The tiers that used to are part of the event
/// plan now, which is what stopped the tier being decided twice; this stays as
/// the operator's own command for signalling the lights by hand, and for
/// checking that a bridge and key in the config actually work. It ignores
/// `hue.quiet_hours` on purpose: the gate lives at the event path's call site
/// in `fire_pulse_unless_quiet`, so a hand-run pulse still lights the room
/// inside the window, which is what keeps the window checkable while it is on.
///
/// THE WORD IS READ BEFORE THE CONFIG LOADS. `pulse --help` used to load the
/// config first: with none it silently exited 0 having printed nothing, and
/// with one it pulsed the room red, because a non-numeric word was read as a
/// failing exit code. Reading the word first means `--help` and a bad code
/// both answer with no machine read at all.
pub(crate) fn pulse_mode() -> i32 {
    // THE WHOLE TAIL IS READ, not just the word right after `pulse`: H-B
    // requires help to win in flag position anywhere, and an unknown extra
    // word to be refused rather than silently dropped.
    let tail: Vec<String> = std::env::args_os()
        .skip(2)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    if tail.iter().any(|token| pns::args::is_help_flag(token)) {
        println!("{PULSE_USAGE}");
        return 0;
    }
    if tail.len() > 1 {
        eprintln!("{PULSE_USAGE}");
        return 2;
    }
    let word = tail.first().cloned().unwrap_or_default();
    let Some(behaviour) = pns::pulse::exit_behaviour(&word) else {
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

const PULSE_USAGE: &str = "pns: usage: pns pulse [<exit-code>] | \
pns pulse --help, -h (a bare `pulse` is a success pulse)";
