use crate::*;

pub(crate) fn lights_mode(verb: &str) -> i32 {
    match verb {
        "tick" => lights_tick(),
        "quiet" => lights_quiet(),
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

const LIGHTS_USAGE: &str = "pns: usage: pns lights tick | \
pns lights quiet [<place> [<duration>|off]]";
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
