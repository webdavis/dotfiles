use crate::{
    AgentWork, Clock, HeldLamps, JobSpool, LampBridge, LampComplaint, LampComplaints,
    LampHouseRecords, LampMarkers, LampMutes, LampTickClaim, PresenceDecisions, ReadLampHouse,
    ReconcileLights, TickReading,
};
use pns_domain::{
    Snapshot,
    lamps::{Reading, config::Lights},
};
use std::time::Duration;

pub struct MaintainLamps<'a, R, W, M, T, J> {
    pub records: &'a R,
    pub work: &'a W,
    pub markers: &'a M,
    pub claim: &'a T,
    pub jobs: &'a J,
}

pub struct LampReadings<M, P, L, I> {
    pub minutes: M,
    pub presence: P,
    pub last_interaction: L,
    pub interval: I,
}

impl<R, W, M, T, J> MaintainLamps<'_, R, W, M, T, J>
where
    R: HeldLamps + LampMutes + LampComplaints + LampHouseRecords + PresenceDecisions,
    W: AgentWork,
    M: LampMarkers,
    T: LampTickClaim,
    J: JobSpool,
{
    /// One upkeep pass: read the machine, derive the one state the house is in,
    /// and write it to every lamp that should show it.
    ///
    /// EXIT 0 ON EVERY PATH, and SILENT on every happy one. This runs three times
    /// a minute forever under a daemon nobody is watching, so a line per tick is a
    /// log the rotation job then rotates a real log out of.
    ///
    /// EVERY STATE IS RE-DERIVED FROM SCRATCH. Nothing is carried between runs
    /// except what is on disk, which is the daemon's own rule: this process exists
    /// for a fraction of a second and the next one is a different process
    /// entirely.
    ///
    /// THE JOURNAL IS READ AND NEVER CLAIMED. `claim_journal` is how the replay
    /// CONSUMES a queue; a tick that claimed it would delete the misses the
    /// operator has not seen yet, which is the opposite of what the glow is for.
    pub fn run<B, MN, PR, LI, I, EL, SL>(
        &self,
        lights: Option<&Lights>,
        clock: &impl Clock,
        connect: impl FnOnce(Option<u64>) -> Option<B>,
        readings: LampReadings<MN, PR, LI, I>,
        mut report: impl FnMut(&str),
    ) where
        B: LampBridge,
        MN: FnOnce(u64) -> Option<u16>,
        PR: FnOnce() -> Option<Snapshot>,
        LI: FnOnce() -> Option<u64>,
        I: FnOnce() -> (EL, SL),
        EL: FnMut() -> u64,
        SL: FnMut(Duration),
    {
        // A missing lamp map or clock can still clear recorded lamps. No connection
        // is constructed until the held record names something that needs clearing.
        let (Some(lights), Some(now)) = (lights, clock.now_secs()) else {
            crate::clear_held_lamps(self.records, || connect(None));
            return;
        };
        // Missing credentials keep the record because no lamp can be addressed.
        let Some(bridge) = connect(Some(lights.refresh_secs)) else {
            return;
        };
        self.markers.sweep_legacy();
        let standing = ReadLampHouse {
            work: self.work,
            markers: self.markers,
            records: self.records,
        }
        .read(lights, now, readings.last_interaction);
        let (muted, mut complaints) = crate::ad_hoc_quiet(self.records, Some(now));
        // A RECORD THIS CANNOT READ NAMES NOTHING TO CLEAR, and the tick is its
        // only writer, so it goes on: the pass below publishes the record it
        // derived, which is what repairs the file. The residue is stated: a lamp
        // held under a name this run could not read stays lit until the repaired
        // record names it again or the operator's next return clears it.
        // ONE READ FOR BOTH THE BARE GATE AND THE PHASE A RESUMED BREATH NEEDS,
        // rather than two: `held_lamps` is `read_held` with the phase dropped, and
        // reading the record twice here would be two disk reads of one fact this
        // tick only ever reads once.
        let held_before_entries = HeldLamps::read(self.records);
        let held_before: Option<Vec<String>> = held_before_entries
            .as_deref()
            .map(|entries| entries.iter().map(|entry| entry.path.clone()).collect());
        if held_before.is_none() {
            complaints.push(HELD_RECORD_UNREADABLE.to_string());
        }
        let active = pns_domain::lights::held::active_held(&standing.house);
        // NOTHING TO LIGHT AND NOTHING TO PUT OUT IS NO BRIDGE CALL AT ALL, which
        // is what keeps an idle machine off the network several times a minute.
        //
        // THE GATE IS THE HOUSE STATE ALONE, and that is a deliberate narrowing from
        // the shipped one. The old gate also asked whether any place could be awake,
        // which took the quiet-hours chain out of the config with no bridge listing
        // to judge it against and paid for it with two stated limits; the dim window
        // is now a per-lamp answer that needs the listing anyway, so the cheap half
        // of that question no longer exists. A house holding nothing still costs
        // nothing, which is the case that matters.
        if !active.is_empty() || held_before.as_deref().is_none_or(|held| !held.is_empty()) {
            // THE ONE MONOTONIC CLOCK THE WHOLE TICK IS MEASURED ON, started here
            // and read by nothing else: the resolve's cost, every fade's due
            // millisecond and the moment each write actually happened are all
            // offsets from this instant, so they can never disagree about when the
            // tick began. It is a parameter for the reason the sleeper is one: the
            // driver fills its whole interval by design, so a test that read the
            // real clock would live the interval too.
            let (elapsed_ms, sleep) = (readings.interval)();
            complaints.extend(
                ReconcileLights {
                    bridge: &bridge,
                    held: self.records,
                    claim: self.claim,
                    presence: self.records,
                }
                .run(
                    TickReading {
                        lights,
                        active: &active,
                        reading: &Reading {
                            minutes_now: (readings.minutes)(now),
                            muted: &muted,
                        },
                        held_before: held_before_entries.as_deref(),
                        now_ms: now.saturating_mul(1000),
                        presence: (readings.presence)().as_ref(),
                    },
                    elapsed_ms,
                    sleep,
                ),
            );
        }
        // AND THE SAYING IS OUTSIDE THAT GATE, deliberately. `say` FORGETS a
        // complaint that has cleared, and a complaint clears exactly when the house
        // goes dark; leaving the bookkeeping inside the gate meant a remembered
        // complaint was never forgotten on the tick that ended it, so the same
        // complaint returning later would not read as news.
        crate::report_lamp_complaints(self.records, LampComplaint::Tick, &complaints, &mut report);
        // AND THE TICK KEEPS ITSELF ALIVE while anything could still light a lamp.
        // Its lease was refreshed by EVENTS alone, which reaches only the states an
        // event ARRIVES with: a shell command produces no events at all, and the
        // automatic loop trigger is five minutes by default and six on the
        // operator's own machine, both PAST the five-minute lease an event leaves.
        // So the one lamp whose whole job is a long run could never arm itself, and
        // a lease taken by hand in a pane that then went quiet expired unread.
        //
        // IT IS STILL BOUNDED BY THE CONDITION, not a self-perpetuating job: a
        // house holding nothing with no run and no lease renews nothing, so an idle
        // machine's tick lapses exactly as it did.
        if !active.is_empty() || standing.in_flight {
            crate::schedule_lights_tick(self.jobs, lights, now, crate::ORDINARY_LEASE_SECS);
        }
    }
}

/// What a tick says about a held record it could not read at all.
///
/// THE TICK GOES ON, because it is the file's only writer: it names no lamp to
/// clear, derives the states it wants and publishes a record for them, which is
/// what repairs an unreadable file. Where the path cannot be WRITTEN either, the
/// publish refuses and nothing is armed, which is the second sentence the
/// operator gets.
const HELD_RECORD_UNREADABLE: &str = "pns lights: the held record could not be read, \
so no lamp can be put out by name";

#[cfg(test)]
mod tests;
