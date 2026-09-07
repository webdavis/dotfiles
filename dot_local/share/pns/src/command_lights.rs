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
    let arguments: Vec<String> = std::env::args_os()
        .skip(3)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let home = std::env::var("HOME").unwrap_or_default();
    let loaded = load_config(&config_path(&home));
    let known = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => config
            .lights
            .as_deref()
            .map(|lights| mutable_names(lights, config, &arguments))
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
    let until_quiet_ends = pns::lights::bare_mute_secs(
        match &loaded {
            Ok(LoadOutcome::Loaded(config)) => enabled_hue_table(config)
                .and_then(|settings| quiet_window(&settings).ok().flatten())
                .map(|window| window.ends_at()),
            _ => None,
        },
        now.and_then(local_minutes_since_midnight),
    );
    let command = match pns::lights::quiet_command(&arguments, &known, until_quiet_ends) {
        Ok(command) => command,
        Err(refusal) => {
            eprintln!("{refusal}");
            eprintln!("{LIGHTS_USAGE}");
            return 2;
        }
    };
    let (entries, complaints) = muted_state(&state);
    // SAID BEFORE ANYTHING IS WRITTEN, because the write below republishes the
    // whole file: an operator whose file was unreadable is losing whatever it
    // held, and that is a line they get to see rather than a silent repair.
    for complaint in &complaints {
        eprintln!("{complaint}");
    }
    let rebuilt = match &command {
        pns::lights::QuietCommand::Report => Ok(entries.clone()),
        pns::lights::QuietCommand::Unmute { place } => {
            pns::lights::muted_after(&entries, place, None, now)
        }
        pns::lights::QuietCommand::Mute { place, seconds } => {
            match now.map(|now| now.saturating_add(*seconds)) {
                Some(expiry) => pns::lights::muted_after(&entries, place, Some(expiry), now),
                // THE CLOCK IS WHAT A MUTE IS MADE OF, so a run that cannot
                // read one says the mute was not set rather than writing an
                // expiry it guessed. `pns quiet`'s own wording, one file over.
                None => Err(
                    "pns: state error (the clock cannot be read); the mute was not set".to_string(),
                ),
            }
        }
    };
    // A REFUSED REBUILD IS A MUTE THAT WAS NOT SET, and nothing is written or
    // reported after one: the file on disk is exactly what it was, and a report
    // built from a list this run refused to publish would describe a house that
    // does not exist.
    let kept = match rebuilt {
        Ok(kept) => kept,
        Err(refusal) => {
            eprintln!("{refusal}");
            return 1;
        }
    };
    if !matches!(command, pns::lights::QuietCommand::Report)
        && let Err(error) = publish_muted(&state.join(LIGHTS_QUIET), &kept)
    {
        // LOUD, because a human is waiting on the answer: reporting a mute that
        // is not in effect is the worst outcome available.
        eprintln!(
            "pns: state error (lights-quiet could not be written: {error}); \
             the mute was not set"
        );
        // AND NO REPORT AFTER IT. `kept` is what the file WOULD have held: for
        // a failed mute it would say the place is quiet when it is not, and for
        // a failed `off` it would say nothing is quiet while the old mute is
        // still on disk and still taking the lamp. The disk is the answer and
        // this run did not change it.
        return 1;
    }
    for line in pns::lights::muted_report(&kept, now) {
        println!("{line}");
    }
    0
}

/// Every name `pns lights quiet` will take, for the command as it was typed.
///
/// THE GRAMMAR IS LAMP, ROOM AND ZONE, which are the BRIDGE'S nouns as much as
/// the config's: a lamp that inherits its room's declaration has a real name no
/// declaration writes, and refusing it sends the operator away from the room
/// they are standing in. So the bridge's own listing widens the vocabulary.
///
/// AND THE DIAL IS ON THE MISS PATH ALONE. A place a declaration already holds
/// is a name a mute can enforce whatever the bridge says, so the ordinary
/// command, muting a room the config routes, costs no network at all. Only a
/// word neither this run's declarations nor `off` can account for is worth
/// asking a bridge about, and `off` is allowed over any name because it can
/// only remove.
fn mutable_names(
    lights: &pns::config::Lights,
    config: &pns::config::Config,
    arguments: &[String],
) -> Vec<String> {
    let declared = pns::channels::hue::mutable_names(lights, None);
    if !asks_the_bridge(&declared, arguments) {
        return declared;
    }
    pns::channels::hue::mutable_names(lights, bridge_inventory(config).as_ref())
}

/// Whether the typed command holds a word only a bridge listing could account
/// for.
///
/// THE FIRST ARGUMENT IS THE PLACE in every form that names one (`<place>`,
/// `<place> <duration>`, `<place> off`), and the bare report names none. A
/// second word of `off` needs no listing either: `off` is allowed over any
/// name, because it can only remove a mute the operator can see.
fn asks_the_bridge(declared: &[String], arguments: &[String]) -> bool {
    arguments.first().is_some_and(|place| {
        !declared.contains(place) && arguments.get(1).is_none_or(|word| word != "off")
    })
}

/// What the bridge says it holds, or nothing at all.
///
/// A BRIDGE THAT ANSWERS NOTHING IS NOT A REFUSAL. The declarations are still
/// names a mute can enforce once the transport is back, so the command works
/// with the bridge down at the cost of a narrower vocabulary.
fn bridge_inventory(config: &pns::config::Config) -> Option<pns::channels::hue::Inventory> {
    let settings = enabled_hue_table(config)?;
    let hue = hue_settings(&settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref())?;
    // THE HUMAN'S OWN DEADLINE, not the transport's. Nothing else here dials a
    // bridge with somebody standing at a terminal waiting on the answer, and
    // three calls at the transport's ten seconds is half a minute before a mute
    // typed at bedtime says anything at all. A bridge on the same LAN answers
    // these in milliseconds, so a second apiece is generous; past it the
    // vocabulary narrows to the declarations, which is what a bridge that
    // answered nothing leaves anyway.
    let bridge = UreqBridge {
        base: format!("https://{}/clip/v2/resource", hue.bridge),
        key: hue.key,
        deadline: TYPED_COMMAND_DEADLINE,
    };
    Some(pns::channels::hue::inventory(
        &pns::channels::hue::Bridge::get(&bridge, "room")?,
        &pns::channels::hue::Bridge::get(&bridge, "light")?,
        &pns::channels::hue::Bridge::get(&bridge, "zone")?,
    ))
}

/// Publish the file, or remove it when nothing is muted.
///
/// AN EMPTY FILE IS NO FILE, which is `remember_held`'s own rule and is
/// what keeps the reader's refusal of an empty one honest: this never writes
/// one, so a file with no lines in it was written by something else.
fn publish_muted(state: &Path, kept: &[pns::lights::Muted]) -> std::io::Result<()> {
    if kept.is_empty() {
        return match std::fs::remove_file(state) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    }
    publish_state_line(state, &pns::lights::render_muted(kept))
}

/// Everything the ad-hoc quiet file holds, and the complaint from a file this
/// cannot vouch for.
///
/// ONE READER FOR BOTH READERS, which is why the command and the event path
/// share it: they want different things out of the file (the entries to rebuild
/// and the names that are live), and two readers is two chances for one of them
/// to swallow a failure the other reports.
///
/// A MISSING FILE IS THE ORDINARY CASE and says nothing: the command has
/// never been run, or its last mute expired and took the file with it. EVERY
/// OTHER READ FAILURE IS A COMPLAINT, and the distinction is the point: a file
/// that is unreadable, not UTF-8, or a directory standing where it should be
/// says NOTHING about which places are quiet, exactly as a corrupt one does,
/// and the two readers of that complaint take opposite directions with it.
/// `ad_hoc_quiet` mutes EVERYTHING (a lamp path fails dark), and the command
/// prints it and rebuilds from an empty list. Either way the operator is told,
/// which is what a complaint is for: a mute nobody can see, in either
/// direction, is the state worth a sentence.
fn muted_state(state: &Path) -> (Vec<pns::lights::Muted>, Vec<String>) {
    let contents = match std::fs::read_to_string(state.join(LIGHTS_QUIET)) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (Vec::new(), Vec::new());
        }
        Err(error) => {
            return (
                Vec::new(),
                vec![format!(
                    "pns: state error (lights-quiet could not be read: {error}); \
                     nothing is quiet"
                )],
            );
        }
    };
    match pns::lights::muted_entries(&contents) {
        Ok(entries) => (entries, Vec::new()),
        Err(complaint) => (Vec::new(), vec![complaint]),
    }
}

/// What an ad-hoc quiet is muting right now, and that same complaint.
///
/// A READING THIS CANNOT TAKE MUTES EVERYTHING, which is the fail direction
/// every lamp-path input takes and the OPPOSITE of what both halves used to do.
/// A record nobody can parse and a clock nobody can read each answered with an
/// empty list, which is a house with every lamp loud: exactly the 3am the mute
/// was armed to prevent, on the one night the machine could not tell anybody
/// why.
///
/// THE COMPLAINT IS STILL THE OTHER HALF. Going dark silently would be a lamp
/// that stopped working for a reason nobody can see, so the caller says it
/// once through `say_lights_once` and the state is repaired by the next
/// `pns lights quiet` write, which republishes the whole file.
pub(crate) fn ad_hoc_quiet(
    state: &Path,
    now: Option<u64>,
) -> (pns::channels::hue::Muting, Vec<String>) {
    let (entries, complaints) = muted_state(state);
    if !complaints.is_empty() {
        return (pns::channels::hue::Muting::Everything, complaints);
    }
    let Some(now) = now else {
        return (
            pns::channels::hue::Muting::Everything,
            vec![pns::lights::NO_CLOCK_FOR_THE_MUTE.to_string()],
        );
    };
    (
        pns::channels::hue::Muting::Places(pns::lights::muted_places(&entries, Some(now))),
        complaints,
    )
}
/// Where the EVENT path remembers the ad-hoc quiet complaint it last made,
/// which is a file of its own for the reason `say_lights_once` states.
pub(crate) const LIGHTS_QUIET_SAID: &str = "lights-quiet-said";

/// Where the operator's own ad-hoc quiet lives: one line per place, each an
/// expiry second and the name they typed.
///
/// ONE FILE RATHER THAN ONE PER PLACE, and that is a path-safety decision as
/// much as a tidiness one: a place is a room name the operator typed, spaces
/// and all, and nothing in this crate turns typed text into a filename unless a
/// predicate already vouches for it.
const LIGHTS_QUIET: &str = "lights-quiet";

#[cfg(test)]
#[path = "command_lights/tests.rs"]
mod command_lights_tests;
