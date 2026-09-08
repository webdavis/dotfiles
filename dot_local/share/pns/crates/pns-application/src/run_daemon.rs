use crate::{Clock, DaemonNotice, DaemonSettings, DaemonSpool, JobChildren, JobSpool};
use std::time::Duration;

pub struct RunDaemon<'a, S, C> {
    pub settings: &'a S,
    pub clock: &'a C,
}
impl<S: DaemonSettings, C: Clock> RunDaemon<'_, S, C> {
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
    pub fn run<J, K, L>(
        &self,
        prepare: impl FnOnce() -> Result<(J, K, L), String>,
        mut notice: impl FnMut(DaemonNotice),
    ) -> i32
    where
        J: DaemonSpool + JobSpool,
        K: JobChildren,
        L: FnMut(),
    {
        if !self.enabled(&mut notice) {
            // ONE LINE, ONCE, on the path that exits. `SuccessfulExit = false` in
            // the plist is what keeps a clean exit 0 exited, so this is written at
            // most once per bootstrap rather than once per throttle window.
            notice(DaemonNotice::Output(DISABLED.into()));
            return 0;
        }
        // EXIT 0 ON A REFUSAL RETRYING CANNOT FIX. Both of them (a spool path that
        // is not a directory, a state directory that will not take one) are
        // permanent, and `KeepAlive { SuccessfulExit = false }` relaunches a
        // non-zero exit every ten seconds forever: ~8,640 relaunches and ~8,640
        // copies of this line a day, which is behavior 15's chatter arriving
        // through the restart door. A clean exit keeps the job DOWN and the
        // doctor's line is what tells the operator.
        let (jobs, mut children, mut sleep) = match prepare() {
            Ok(prepared) => prepared,
            Err(refusal) => {
                notice(DaemonNotice::Error(format!("pns daemon: {refusal}")));
                return 0;
            }
        };
        let mut reported: std::collections::BTreeSet<J::Entry> = std::collections::BTreeSet::new();
        let mut ticks: u64 = 0;
        loop {
            sleep();
            ticks = ticks.wrapping_add(1);
            // THE SWITCH IS RE-READ, so `enabled = false` reaches a daemon that is
            // ALREADY RUNNING. Read once at startup it was inert: nothing bounces
            // this job on a config change (the loader's trigger is the plist hash),
            // so the operator's off switch did nothing until a hand-typed bootout.
            // Once every `SWITCH_TICKS` rather than every tick, which is one config
            // read per thirty seconds at the production tick.
            let now = self.clock.now_secs();
            if ticks.is_multiple_of(SWITCH_TICKS) {
                if !self.enabled(&mut notice) {
                    notice(DaemonNotice::Output(DISABLED.into()));
                    return 0;
                }
                // THE ROOM SENSOR IS THE DAEMON'S OWN JOB, on the same cadence
                // through its own config read: no event asks for a room reading,
                // so nothing else would ever register it, and a job registered
                // once at startup would die with its lease on the first daemon
                // that outran it.
                if let Some(now) = now {
                    crate::ensure_presence_poll(&jobs, self.settings.presence_interval(), now);
                }
            }
            crate::RunDaemonTick {
                spool: &jobs,
                children: &mut children,
            }
            .run(now, &mut reported, &mut notice);
        }
    }
    /// Whether the clock is switched on.
    ///
    /// THE BROKEN-CONFIG FALLBACK IS ON, inherited from `select_plugins`' own: a
    /// file that will not parse must not silently stop a service the operator
    /// enabled, and the warning says which it was.
    fn enabled(&self, notice: &mut impl FnMut(DaemonNotice)) -> bool {
        match self.settings.enabled() {
            Ok(enabled) => enabled,
            Err(detail) => {
                notice(DaemonNotice::Error(format!(
                    "pns daemon: the config could not be read ({detail}); carrying on enabled"
                )));
                true
            }
        }
    }
}

const DISABLED: &str = "pns daemon: disabled in the config; exiting";
/// How many ticks pass between two reads of the config's own switch.
///
/// THIRTY, so the cost is one config read per thirty seconds at the production
/// tick, and the switch still takes effect within half a minute of being
/// flipped. Counted in TICKS rather than seconds for `CHILD_TICKS`'s reason:
/// one knob moves with the clock instead of two disagreeing about it.
const SWITCH_TICKS: u64 = 30;

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
pub fn daemon_tick(raw: Option<&str>) -> Duration {
    let milliseconds = raw
        .and_then(pns_domain::count::parse_count)
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
mod tests;
