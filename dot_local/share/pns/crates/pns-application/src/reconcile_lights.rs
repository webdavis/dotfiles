use crate::lamp_breath::{Breathing, drive_breaths};
use crate::{HeldLamps, LampBridge, LampTickClaim, LampWrite, PresenceDecisions};
use pns_domain::{
    Snapshot,
    lamps::{Reading, config::Lights},
    lights::{held::Held, phase::HeldEntry},
};
use std::time::Duration;
mod phase;
mod prepare;

pub struct ReconcileLights<'a, B, H, T, P> {
    pub bridge: &'a B,
    pub held: &'a H,
    pub claim: &'a T,
    pub presence: &'a P,
}
#[derive(Clone, Copy)]
pub struct TickReading<'a> {
    pub lights: &'a Lights,
    pub active: &'a [Held],
    pub reading: &'a Reading<'a>,
    pub held_before: Option<&'a [HeldEntry]>,
    pub now_ms: u64,
    pub presence: Option<&'a Snapshot>,
}

fn bare_held(held: &impl HeldLamps) -> Option<Vec<String>> {
    held.read()
        .map(|entries| entries.into_iter().map(|entry| entry.path).collect())
}

impl<B: LampBridge, H: HeldLamps, T: LampTickClaim, P: PresenceDecisions>
    ReconcileLights<'_, B, H, T, P>
{
    /// The tick's writes, in the ONE ORDER that cannot leave a lamp lit and
    /// unaccounted for: arm every lamp, clear only what the arm did not write to,
    /// record what is held (bare), breathe, and only then record the phase each
    /// lamp landed on.
    ///
    /// THE ORDER IS THE BEHAVIOUR, which is why these are one function rather than
    /// five lines at the bottom of the tick. Every held body is a plain state write
    /// that does NOT expire, so a clear computed before the arm, or a record written
    /// before the clear, is a lamp left lit with nothing that knows its name.
    ///
    /// THE PRE-ARM WRITE IS BARE, deliberately dropping any phase this tick read:
    /// it is what a killed child leaves behind, and a killed child cannot finish a
    /// fade, so a bare token is a lamp this run cannot promise landed anywhere in
    /// particular. The PHASE is a SECOND write, after the breath returns, guarded
    /// by a re-read of the SAME bare list this tick's own pre-arm write left: a
    /// return that cleared the record mid-breath already emptied it, and writing
    /// the phase over that would resurrect a hold the operator just ended.
    ///
    /// THE BREATHING RUNS LAST AND HOLDS THIS PROCESS OPEN until the last fade has
    /// been ISSUED, one seamless turn-around's lead before the budget ends. That is
    /// what makes the lamp a liveness signal: the fades are issued by this process
    /// on a cadence, so a daemon that dies, a machine that sleeps and a pns that
    /// crashes all stop the motion within one interval. The record and the clear are
    /// already on disk before the first sleep, so a driver killed mid-breath costs a
    /// lamp frozen at its last brightness and never a lamp nothing can put out.
    ///
    /// AND THE CHILD IS GONE BEFORE THE NEXT TICK'S CHILD RUNS, which the daemon's
    /// own `running` check enforces rather than the schedule alone: the last fade
    /// is routinely still running on the bridge when this budget ends (that is the
    /// seamless join), so a write that overran its lead can no longer be met by a
    /// second child. The tick's own lock is the half of that the daemon cannot
    /// see, and it covers a tick run by hand and an orphan a daemon replacement
    /// left behind.
    ///
    /// A BRIDGE THAT ANSWERED NO LISTING CHANGES NOTHING AT ALL. It is direct
    /// evidence the transport is down, and both halves of acting on it are wrong: a
    /// clear it refused is invisible, and forgetting the paths after it leaves the
    /// lamp lit with nothing in the system that knows about it.
    ///
    /// IT PRINTS NOTHING. The complaints are answered for the caller to say once.
    pub fn run(
        &self,
        tick: TickReading<'_>,
        mut elapsed_ms: impl FnMut() -> u64,
        sleep: impl FnMut(Duration),
    ) -> Vec<String> {
        let TickReading {
            lights,
            held_before,
            now_ms,
            ..
        } = tick;
        let mut complaints = Vec::new();
        // ONE TICK DRIVES THE HOUSE AT A TIME. Taken before the resolve rather than
        // around the record alone, because two ticks that both got past a record
        // comparison would still spend a whole interval issuing fades at each
        // other. The second returns having done nothing at all, which is what a
        // tick with nothing to say has always returned.
        //
        // `now_ms / 1000` IS THE SECOND THE CALLER IS ON: production hands this
        // function the wall clock in milliseconds, and the age rule compares that
        // against the lock file's own mtime.
        let Some(_lock) = self.claim.claim(now_ms / 1000) else {
            return complaints;
        };
        let Some(breathing) = self.breathing(&tick, &mut complaints) else {
            return complaints;
        };
        let held_before_bare: Option<Vec<String>> =
            held_before.map(|entries| entries.iter().map(|entry| entry.path.clone()).collect());
        // THE RECORD IS READ AGAIN BEFORE ANYTHING IS WRITTEN, and this run stands
        // down if it moved. The states above were derived BEFORE the bridge work,
        // which is seconds of network, and the event path clears every held lamp and
        // empties this record the moment the operator comes back: a tick still
        // resolving when that happened would arm the lamps again off a snapshot
        // taken before the clear, and the operator would watch a lamp they had just
        // put out come back on. The other writer has already done the clearing, so
        // there is nothing left here to do.
        if bare_held(self.held).as_deref() != held_before_bare.as_deref() {
            return complaints;
        }
        let held_now: Vec<String> = breathing.iter().map(|entry| entry.path.clone()).collect();
        // WHATEVER WAS HELD AND IS NOT HELD NOW GETS PUT OUT BY NAME. Written as a
        // difference rather than as a special case, so a lamp dropped by a dim
        // window, a mute, a config edit or the condition simply ending is covered by
        // one line rather than four.
        let stale: Vec<String> = held_before_bare
            .unwrap_or_default()
            .iter()
            .filter(|path| !held_now.contains(path))
            .cloned()
            .collect();
        for path in &stale {
            self.bridge.write(path, &LampWrite::Clear);
        }
        // A RECORD THAT DID NOT LAND STOPS THE ARM, and that is the whole reason
        // this answer is read. Every held body is a plain state write that does not
        // expire, so arming a lamp the record does not name is a lamp nothing in
        // the system can ever put out: not the next tick, which computes its clear
        // by name off this file, not the return from an absence, and not the
        // operator's own mute. Nothing armed is one interval of a dark lamp, which
        // the next tick fixes by itself.
        let pre_arm: Vec<pns_domain::lights::phase::HeldEntry> = held_now
            .iter()
            .cloned()
            .map(pns_domain::lights::phase::HeldEntry::bare)
            .collect();
        if let Err(error) = self.held.remember(&pre_arm) {
            complaints.push(format!(
                "pns lights: the held record could not be written ({error}); no lamp \
             was armed, because nothing would have been able to put one out"
            ));
            return complaints;
        }
        // WHAT IS LEFT OF THE INTERVAL, and not the interval: the resolve above is
        // three bridge calls, and the fades have to be issued and finished inside
        // the time this child still has.
        let spent_ms = elapsed_ms();
        let budget_ms = lights
            .refresh_secs
            .saturating_mul(1000)
            .saturating_sub(spent_ms);
        let landings = drive_breaths(
            self.bridge,
            budget_ms,
            &breathing,
            || elapsed_ms().saturating_sub(spent_ms),
            sleep,
        );
        phase::remember_phase(
            self.held, &held_now, &breathing, &landings, now_ms, spent_ms,
        );
        complaints
    }
}
