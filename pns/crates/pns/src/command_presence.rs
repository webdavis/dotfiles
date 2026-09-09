use crate::*;

pub(crate) fn presence_mode(verb: &str) -> i32 {
    let arguments: Vec<String> = crate::arguments_after_verb();
    match (verb, presence_launch(&arguments)) {
        ("poll", Some(launch)) => presence_poll(launch),
        // UNKNOWN IS AN ERROR, never a silent fallthrough, exactly as the
        // lamps' verb is.
        _ => {
            eprintln!("{PRESENCE_USAGE}");
            2
        }
    }
}

const PRESENCE_USAGE: &str = "pns: usage: pns presence poll [--daemon]";

/// Who launched a poll, which is the whole difference between a refusal worth
/// printing and one worth swallowing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Launch {
    /// The daemon's own clock, every few seconds, with its stderr pointed at a
    /// log file and nobody in front of it.
    Daemon,
    /// A person, at a terminal, waiting for the answer.
    Operator,
}

use pns_application::PRESENCE_DAEMON_FLAG;

/// Who launched this poll, or `None` for an argument tail this does not serve.
fn presence_launch(arguments: &[String]) -> Option<Launch> {
    match arguments {
        [] => Some(Launch::Operator),
        [flag] if flag == PRESENCE_DAEMON_FLAG => Some(Launch::Daemon),
        _ => None,
    }
}

/// `pns presence poll`: one read of the bridge, published as the line the room
/// sensor reads.
///
/// EVERY REFUSAL HERE PUBLISHES NOTHING and says nothing, which is the same
/// direction the lamps' tick takes and for a stronger reason: this runs every
/// few seconds under the daemon, so a complaint would be a line a second in the
/// daemon's log, and the reading it failed to refresh ages out to Unknown by
/// itself. The doctor is what names a table it could not read.
fn presence_poll(launch: Launch) -> i32 {
    let home = std::env::var("HOME").unwrap_or_default();
    let Ok(LoadOutcome::Loaded(config)) = load_config(&config_path(&home)) else {
        return 0;
    };
    // THE HUE TABLE IS WHERE THE CREDENTIALS LIVE, which is why the registry
    // refuses a presence table without one: there is no second place to reach
    // a bridge from.
    let (Ok(Some(presence)), Some(settings), Some(now)) = (
        pns_adapters::parse_presence(&config),
        enabled_hue_table(&config),
        now_secs(),
    ) else {
        return 0;
    };
    let Some(hue) = hue_settings(&settings, None) else {
        return 0;
    };
    let polled = write_presence_reading(
        &UreqBridge {
            base: format!("https://{}/clip/v2/resource", hue.bridge),
            key: hue.key,
            // THE TRANSPORT'S OWN DEADLINE, twice, which is what keeps the
            // whole poll inside the 30-second bound the daemon spawns it
            // under. A wedged bridge is killed there, and the reading it
            // never refreshed goes stale, which is the answer that was
            // wanted anyway.
            deadline: pns_adapters::BRIDGE_DEADLINE,
        },
        &state_dir(),
        &presence,
        now,
    );
    let (code, complaint) = reported(polled, launch);
    if let Some(complaint) = complaint {
        eprintln!("{complaint}");
    }
    code
}
pub(crate) fn write_presence_reading<B: pns_adapters::Bridge>(
    bridge: &B,
    state: &Path,
    presence: &pns_adapters::Presence,
    now: u64,
) -> Polled {
    pns_application::poll_presence(
        &pns_adapters::BridgePresencePoll { bridge, state },
        &presence.rooms,
        &presence.exclude,
        now,
    )
}
pub(crate) use pns_application::Polled;
/// The exit status, and the one line the operator is owed.
///
/// SILENT AND ZERO FOR EVERY REFUSAL BUT ONE, because the daemon runs this
/// every few seconds with its stderr pointed at the log: a complaint on the
/// ordinary refusals would be a line a second, and the doctor is what names
/// a sensor that has stopped reading. Contention is the exception. It is
/// transient by construction, so it cannot flood anything, and a hand-typed
/// poll that read no bridge and published nothing otherwise looks exactly
/// like one that worked.
fn reported(polled: Polled, launch: Launch) -> (i32, Option<&'static str>) {
    match (polled, launch) {
        (Polled::Busy, Launch::Operator) => (
            1,
            Some(
                "pns presence: another poll holds the bridge read; \
                     this one published nothing",
            ),
        ),
        _ => (0, None),
    }
}
#[cfg(test)]
#[path = "command_presence/tests/publication.rs"]
mod publication_tests;

#[cfg(test)]
#[path = "command_presence/tests/locking.rs"]
mod locking_tests;

#[cfg(test)]
#[path = "command_presence/tests/daemon.rs"]
mod daemon_tests;

#[cfg(test)]
fn ensure_presence_poll(state: &Path, presence: Option<&pns_adapters::Presence>, now: u64) {
    pns_application::ensure_presence_poll(
        &pns_adapters::FileJobSpool::new(state.to_path_buf()),
        presence.map(|presence| presence.poll_secs),
        now,
    );
}
