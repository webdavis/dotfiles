use crate::*;

pub(crate) fn presence_mode(verb: &str) -> i32 {
    let arguments: Vec<String> = std::env::args_os()
        .skip(3)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
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

/// The daemon's own spelling, passed by the registration in
/// `ensure_presence_poll` and by nothing else.
///
/// A FLAG RATHER THAN AN ENVIRONMENT VARIABLE, because the argv is what the
/// spool already records and what one parser already reads: an inherited
/// variable would also mark every unrelated process a poll ever started, and
/// the poll is the thing being described, not its ancestry.
const PRESENCE_DAEMON_FLAG: &str = "--daemon";

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
        pns::config::parse_presence(&config),
        enabled_hue_table(&config),
        now_secs(),
    ) else {
        return 0;
    };
    let Some(hue) = hue_settings(&settings, None) else {
        return 0;
    };
    let (code, complaint) = write_presence_reading(
        &UreqBridge {
            base: format!("https://{}/clip/v2/resource", hue.bridge),
            key: hue.key,
            // THE TRANSPORT'S OWN DEADLINE, twice, which is what keeps the
            // whole poll inside the 30-second bound the daemon spawns it
            // under. A wedged bridge is killed there, and the reading it
            // never refreshed goes stale, which is the answer that was
            // wanted anyway.
            deadline: pns::channels::hue::BRIDGE_DEADLINE,
        },
        &state_dir(),
        &presence,
        now,
    )
    .reported(launch);
    if let Some(complaint) = complaint {
        eprintln!("{complaint}");
    }
    code
}

/// The poll's whole effect: read the bridge, publish the reading. `true` when
/// a line was published.
///
/// A BRIDGE THAT DID NOT ANSWER PUBLISHES NOTHING. That single choice is what
/// makes a dead bridge, a wrong key and a wedged LAN all read as Unknown a few
/// seconds later, instead of pinning the operator wherever the last poll left
/// them. AND NEITHER DOES A READING THE FORMAT CANNOT CARRY: `render` hands
/// its own line back to its own parser and answers nothing at all when what
/// comes back is not what went in, so the write is never a line the reader
/// would read as a different reading.
///
/// THE BRIDGE IS A PARAMETER so this, the join of the config, the network and
/// the state directory, is the thing a test drives end to end. Everything
/// under it is pure and everything over it is six lines of composition.
///
/// ONE POLLER AT A TIME ACROSS THE WHOLE MACHINE, held from before the first
/// read to after the rename. The running-child check in the daemon is
/// PROCESS-LOCAL, so a second daemon, a replacement daemon that orphaned the
/// first one's child, or a hand-typed `pns presence poll` can each be inside
/// this at once. Without the lock the LAST rename wins rather than the newest
/// reading: a poller stalled between its two reads publishes an older room
/// over a newer one and `classify` accepts it as current. Standing down costs
/// one interval of a reading somebody else is already taking.
///
/// THE LOCK IS THE KERNEL'S, not a name on disk, so the poll a killed poller
/// was inside is claimable the moment it dies: see `presence_lock`.
pub(crate) fn write_presence_reading<B: pns::channels::hue::Bridge>(
    bridge: &B,
    state: &Path,
    presence: &pns::config::Presence,
    now: u64,
) -> Polled {
    // THE DIRECTORY IS MADE BEFORE THE LOCK, because the lock now runs ahead
    // of `publish_state_line`, which used to be the thing that made it: a
    // first poll on a machine with no state directory yet would otherwise fail
    // to take a lock nobody holds and never publish at all.
    let _ = std::fs::create_dir_all(state);
    // HELD BY THE HANDLE AND NOT BY THE NAME, which is what makes a killed
    // poller cost nothing: the kernel closes its file and the lock is gone.
    let _lock = match pns::presence_lock::claim(&state.join(pns::presence_lock::LOCK_FILE)) {
        pns::presence_lock::Claim::Held(file) => file,
        pns::presence_lock::Claim::Busy => return Polled::Busy,
        pns::presence_lock::Claim::Unavailable => return Polled::Nothing,
    };
    // THE EXCLUSION IS APPLIED BEFORE THE NEWEST EDGE IS CHOSEN, not left to
    // the reader. `classify` refuses an excluded room outright, so publishing
    // one would throw away the newest edge in a room that DOES count and
    // answer Unknown. The key is documented for "a room you pass through",
    // which is the room that reports MOST often, so the reading it swallowed
    // would be the common case rather than the corner.
    let watched: Vec<String> = presence
        .rooms
        .iter()
        .filter(|room| !presence.exclude.contains(room))
        .cloned()
        .collect();
    let Some(reading) = pns::presence_hue::poll(bridge, &watched, now) else {
        return Polled::Nothing;
    };
    // A READING THIS FORMAT CANNOT CARRY PUBLISHES NOTHING, the same direction
    // a silent bridge takes. `render` used to substitute the poll-only line
    // for one, which says "the bridge answered and no watched room reported"
    // on evidence that said a watched room had.
    let Some(line) = pns::presence_file::render(&reading) else {
        return Polled::Nothing;
    };
    // FAIL-QUIET, in `remember_staleness`'s style: an unwritable state
    // directory costs the reading, which ages out on its own.
    if publish_state_line(&state.join(pns::presence_file::STATE_FILE), &line).is_ok() {
        Polled::Published
    } else {
        Polled::Nothing
    }
}

/// What one poll did, and what the operator who typed it is told.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Polled {
    /// A reading reached the state file.
    Published,
    /// Nothing was published and nothing this can name is wrong: a bridge that
    /// did not answer, a reading the format cannot carry, a state directory it
    /// cannot write. Every one of them ages the last reading out to Unknown,
    /// which is what they all mean.
    Nothing,
    /// Another poller is inside the bridge read.
    Busy,
}

impl Polled {
    /// The exit status, and the one line the operator is owed.
    ///
    /// SILENT AND ZERO FOR EVERY REFUSAL BUT ONE, because the daemon runs this
    /// every few seconds with its stderr pointed at the log: a complaint on the
    /// ordinary refusals would be a line a second, and the doctor is what names
    /// a sensor that has stopped reading. Contention is the exception. It is
    /// transient by construction, so it cannot flood anything, and a hand-typed
    /// poll that read no bridge and published nothing otherwise looks exactly
    /// like one that worked.
    fn reported(self, launch: Launch) -> (i32, Option<&'static str>) {
        match (self, launch) {
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
}
/// The spool name the room sensor's poll is registered under.
const PRESENCE_JOB: &str = "presence";

/// How long that registration runs for. FIVE MINUTES, which is ten of the
/// daemon's own config reads at the production tick: long enough that a missed
/// sweep changes nothing, short enough that a daemon which stopped leaves
/// nothing polling the bridge behind it.
const PRESENCE_LEASE_SECS: u64 = 300;

/// The room sensor's settings, or `None` when its table is absent, switched
/// off, or refused.
///
/// A REFUSAL READS AS OFF here, deliberately unlike `daemon_enabled`'s
/// carry-on-enabled fallback. That one keeps a whole service alive through a
/// typo; this one governs a sensor whose every unknown already means Unknown,
/// so the fail-closed reading costs a narrowing and the doctor is what names
/// the refusal.
pub(crate) fn presence_settings() -> Option<pns::config::Presence> {
    match load_config(&config_path(&std::env::var("HOME").unwrap_or_default())) {
        Ok(LoadOutcome::Loaded(config)) => pns::config::parse_presence(&config).ok().flatten(),
        _ => None,
    }
}

/// Keep the poll registered while the sensor is on, and cancelled while it is
/// not.
///
/// THE SETTINGS ARRIVE AS AN ARGUMENT rather than being read here, which is
/// what makes the sweep a function of the spool alone: the config read is the
/// caller's, and this can be driven a state directory at a time.
///
/// THE PENDING DUE IS KEPT, `schedule_lights_tick`'s rule for its own reason:
/// re-registering replaces the job by name, so a sweep that pushed `due` out
/// every thirty seconds would keep moving a five-second poll away from itself.
/// Only the LEASE is refreshed.
pub(crate) fn ensure_presence_poll(
    state: &Path,
    presence: Option<&pns::config::Presence>,
    now: u64,
) {
    let Some(presence) = presence else {
        // The failure is dropped for `record_decision`'s reason: a cancel that
        // did not land costs one more poll, and the lease ends it regardless.
        let _ = pns::daemon::cancel(state, PRESENCE_JOB);
        return;
    };
    let pending = match pns::daemon::peek(
        &pns::daemon::spool_dir(state).join(PRESENCE_JOB),
        PRESENCE_JOB,
    ) {
        pns::daemon::Peeked::Job(job) => Some(job.due),
        _ => None,
    };
    // DUE NOW when nothing is pending, so the first sweep after the switch
    // goes on is followed by a reading on the next tick rather than one
    // interval later.
    let due = pending.filter(|due| *due > now).unwrap_or(now);
    let job = pns::daemon::Job {
        id: PRESENCE_JOB.to_string(),
        due,
        until: due.max(now.saturating_add(PRESENCE_LEASE_SECS)),
        every: Some(presence.poll_secs),
        unless_marker: None,
        args: vec![
            "presence".to_string(),
            "poll".to_string(),
            PRESENCE_DAEMON_FLAG.to_string(),
        ],
    };
    // The failure is DROPPED here for `schedule_lights_tick`'s reason: a
    // registration that did not land must never cost the daemon a line a
    // second, and the next sweep tries again.
    let _ = pns::daemon::schedule(state, &job, now);
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
