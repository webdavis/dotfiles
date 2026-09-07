use crate::*;

/// The loop. It sleeps, drains the spool, and reaps what it started.
///
/// IT HOLDS NO DURABLE STATE. Restarting re-reads the directory, which is the
/// whole recovery path, and reboot works the same way because the state
/// directory survives it and the lease drops whatever went stale. There is no
/// in-memory schedule to diverge from the disk.
///
/// SIGTERM NEEDS NO HANDLER. launchd stops a job with SIGTERM and the default
/// disposition terminates the process; a loop sleeping one second dies inside
/// the tick. A child mid-flight is orphaned rather than killed, and an orphaned
/// nudge is at worst one extra card.
pub(crate) fn daemon_run() -> i32 {
    if std::env::args_os().nth(3).is_some() {
        eprintln!("{DAEMON_USAGE}");
        return 2;
    }
    if !daemon_enabled() {
        // ONE LINE, ONCE, on the path that exits. `SuccessfulExit = false` in
        // the plist is what keeps a clean exit 0 exited, so this is written at
        // most once per bootstrap rather than once per throttle window.
        println!("pns daemon: disabled in the config; exiting");
        return 0;
    }
    let state = state_dir();
    let spool = pns::daemon::spool_dir(&state);
    // EXIT 0 ON A REFUSAL RETRYING CANNOT FIX. Both of them (a spool path that
    // is not a directory, a state directory that will not take one) are
    // permanent, and `KeepAlive { SuccessfulExit = false }` relaunches a
    // non-zero exit every ten seconds forever: ~8,640 relaunches and ~8,640
    // copies of this line a day, which is behavior 15's chatter arriving
    // through the restart door. A clean exit keeps the job DOWN and the
    // doctor's line is what tells the operator.
    if let pns::daemon::Startup::Refused(refusal) = pns::daemon::prepare_spool(&state) {
        eprintln!("pns daemon: {refusal}");
        return 0;
    }
    let tick = daemon_tick();
    let mut children: Vec<Bounded> = Vec::new();
    let mut reported: std::collections::BTreeSet<std::path::PathBuf> =
        std::collections::BTreeSet::new();
    let mut ticks: u64 = 0;
    loop {
        std::thread::sleep(tick);
        ticks = ticks.wrapping_add(1);
        // THE SWITCH IS RE-READ, so `enabled = false` reaches a daemon that is
        // ALREADY RUNNING. Read once at startup it was inert: nothing bounces
        // this job on a config change (the loader's trigger is the plist hash),
        // so the operator's off switch did nothing until a hand-typed bootout.
        // Once every `SWITCH_TICKS` rather than every tick, which is one config
        // read per thirty seconds at the production tick.
        let now = now_secs();
        if ticks.is_multiple_of(SWITCH_TICKS) {
            if !daemon_enabled() {
                println!("pns daemon: disabled in the config; exiting");
                return 0;
            }
            // THE ROOM SENSOR IS THE DAEMON'S OWN JOB, on the same cadence and
            // out of the same config read: no event asks for a room reading,
            // so nothing else would ever register it, and a job registered
            // once at startup would die with its lease on the first daemon
            // that outran it.
            if let Some(now) = now {
                ensure_presence_poll(&state, presence_settings().as_ref(), now);
            }
        }
        daemon_pass(&spool, &state, now, tick, &mut children, &mut reported);
    }
}

/// Everything one turn of the daemon's loop does, in the ONE ORDER that makes
/// `decide`'s running answer true.
///
/// REAPED BEFORE THE SPOOL IS DRAINED, so a child `decide` finds still in
/// `children` really is alive THIS pass. Reaped the other way round, a child
/// that exited moments ago still reads as running and holds its own due
/// occurrence to one more `Wait` than it needed, which on the lights job is a
/// tick of a lamp that has stopped breathing.
///
/// IT IS A FUNCTION AND NOT FOUR LINES IN THE LOOP for exactly that reason:
/// the order is the behaviour, so a test has to be able to run it in the
/// order production runs it rather than in one of its own.
///
/// A SECOND THAT COULD NOT BE READ STOPS THE DRAIN AND NEVER THE REAP. A bound
/// is still a bound with no wall clock to publish against, and a child left
/// running past its own because the clock would not answer is the one failure
/// here that accumulates.
fn daemon_pass(
    spool: &Path,
    state: &Path,
    now: Option<u64>,
    tick: Duration,
    children: &mut Vec<Bounded>,
    reported: &mut std::collections::BTreeSet<std::path::PathBuf>,
) {
    reap(children);
    let Some(now) = now else {
        return;
    };
    // FAIL-QUIET, in `remember_staleness`'s style: a heartbeat that did not
    // land costs one doctor line, and complaining about it every tick is
    // the chatter this daemon must never produce.
    let _ = pns::daemon::publish_heartbeat(
        state,
        &pns::daemon::Heartbeat {
            pid: std::process::id(),
            at: now,
        },
    );
    drain_spool(spool, state, now, tick, children, reported);
}
/// How many ticks pass between two reads of the config's own switch.
///
/// THIRTY, so the cost is one config read per thirty seconds at the production
/// tick, and the switch still takes effect within half a minute of being
/// flipped. Counted in TICKS rather than seconds for `CHILD_TICKS`'s reason:
/// one knob moves with the clock instead of two disagreeing about it.
const SWITCH_TICKS: u64 = 30;

/// Whether the clock is switched on.
///
/// THE BROKEN-CONFIG FALLBACK IS ON, inherited from `select_plugins`' own: a
/// file that will not parse must not silently stop a service the operator
/// enabled, and the warning says which it was.
fn daemon_enabled() -> bool {
    match load_config(&config_path(&std::env::var("HOME").unwrap_or_default())) {
        Ok(LoadOutcome::Loaded(config)) => config.daemon_enabled,
        Ok(LoadOutcome::Missing) => true,
        Err(error) => {
            eprintln!(
                "pns daemon: the config could not be read ({}); carrying on enabled",
                error.detail()
            );
            true
        }
    }
}
/// How long the loop sleeps between passes.
///
/// A CONSTANT WITH A TEST HATCH rather than a config key, following
/// `PNS_PAYLOAD_DEADLINE_MS`: the only party who has ever needed a different
/// tick is a test, and a knob nobody turns is a knob that only ever holds a
/// wrong value.
///
/// STRICTLY PARSED, FLOORED AND CAPPED, and anything else falls back to the
/// constant rather than being clamped towards it. A stray `1` in a launchd
/// environment would spin the loop a thousand times a second, and clamping
/// would honour a value nobody meant to write.
fn daemon_tick() -> Duration {
    let milliseconds = std::env::var("PNS_DAEMON_TICK_MS")
        .ok()
        .and_then(|raw| pns::parse_count(&raw))
        .filter(|milliseconds| (MIN_TICK_MS..=MAX_TICK_MS).contains(milliseconds))
        .unwrap_or(DEFAULT_TICK_MS);
    Duration::from_millis(milliseconds)
}

/// One second: fast enough that a nag is on time and a light re-arms before it
/// lapses, slow enough that the idle cost is one `read_dir` of an empty
/// directory per second.
const DEFAULT_TICK_MS: u64 = 1000;

/// The floor, so no environment can spin the loop.
const MIN_TICK_MS: u64 = 10;

/// The ceiling, so no environment can park it.
const MAX_TICK_MS: u64 = 60_000;

#[cfg(test)]
#[path = "daemon_runtime/tests.rs"]
mod daemon_runtime_tests;
