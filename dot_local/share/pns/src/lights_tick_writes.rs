use crate::*;

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
// Keep the bridge, elapsed clock and sleeper independently injectable; the
// other seven inputs are the tick state already acquired by the caller.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_tick_writes<B: pns::channels::hue::Bridge>(
    bridge: &B,
    state: &Path,
    lights: &pns::config::Lights,
    active: &[pns::lights::Held],
    reading: &pns::channels::hue::Reading<'_>,
    held_before: Option<&[pns::lights::HeldEntry]>,
    now_ms: u64,
    presence: Option<&pns::presence_policy::Snapshot>,
    mut elapsed_ms: impl FnMut() -> u64,
    sleep: impl FnMut(Duration),
) -> Vec<String> {
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
    let lock = state.join(LIGHTS_TICK_LOCK);
    if !claim_lock(&lock, now_ms / 1000, lights_tick_stale_secs()) {
        return complaints;
    }
    let _lock = HeldLock(lock);
    let mut breathing: Vec<Breathing> = Vec::new();
    if !active.is_empty() {
        // The doctor is where an unreachable bridge is REPORTED; this process
        // runs unattended and has no reader for that sentence.
        let Some(mut routing) = pns::channels::hue::resolve_on_bridge(bridge, lights) else {
            return complaints;
        };
        // OFF THE WHOLE RESOLUTION, before the narrowing, for
        // `run_pulse_writes`'s reason: a name the bridge could not answer is a
        // typo whether or not the operator is standing in that room.
        complaints.extend(routing_complaints(&routing));
        // WHAT THIS TICK WOULD ACTUALLY ARM, in `run_pulse_writes`'s shape and
        // for its reason: presence has to narrow over the lamps this house
        // state reaches, or a room whose only lamp carries some other state
        // reads as narrowable and then breathes on nothing.
        let breath_for = |routed: &pns::channels::hue::Routed| -> Option<(
            pns::lights::Held,
            pns::channels::hue::Showing,
        )> {
            if pns::channels::hue::muted_now(&routed.lamp, reading.muted) {
                return None;
            }
            let held = pns::lights::shown(active, &routed.shows)?;
            let showing = pns::channels::hue::dim_showing(
                routed.dim.as_ref(),
                held.behaviour(),
                reading.minutes_now,
            );
            (showing != pns::channels::hue::Showing::Dark).then_some((held, showing))
        };
        routing.lamps.retain(|routed| breath_for(routed).is_some());
        let routing = narrow_to_presence(state, routing, presence);
        for routed in &routing.lamps {
            let Some((held, showing)) = breath_for(routed) else {
                continue;
            };
            let (color, cycle) = pns::channels::hue::held_render(held, lights, showing);
            let path = pns::channels::hue::fixture_path(&pns::channels::hue::Fixture::Light(
                routed.lamp.id.clone(),
            ));
            // A LAMP NOT NAMED IN LAST TICK'S RECORD, OR NAMED THERE WITH NO
            // PHASE, RESUMES AT THE DEFAULT: a fresh arm, an external switch,
            // a killed child's bare token and a dim-window shape change all
            // read the same way, and all cost at most one fade of motion.
            let previous =
                held_before.and_then(|entries| entries.iter().find(|entry| entry.path == path));
            let resume = pns::lights::resume_from(previous, now_ms, held, &cycle);
            breathing.push(Breathing {
                path,
                held,
                cycle,
                color,
                resume,
            });
        }
    }
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
    if held_lamps(state).as_deref() != held_before_bare.as_deref() {
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
    pns::channels::hue::clear_held(bridge, &stale);
    // A RECORD THAT DID NOT LAND STOPS THE ARM, and that is the whole reason
    // this answer is read. Every held body is a plain state write that does not
    // expire, so arming a lamp the record does not name is a lamp nothing in
    // the system can ever put out: not the next tick, which computes its clear
    // by name off this file, not the return from an absence, and not the
    // operator's own mute. Nothing armed is one interval of a dark lamp, which
    // the next tick fixes by itself.
    let pre_arm: Vec<pns::lights::HeldEntry> = held_now
        .iter()
        .cloned()
        .map(pns::lights::HeldEntry::bare)
        .collect();
    if let Err(error) = remember_held(state, &pre_arm) {
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
        bridge,
        budget_ms,
        &breathing,
        || elapsed_ms().saturating_sub(spent_ms),
        sleep,
    );
    // THE PHASE, WRITTEN ONLY IF THE PRE-ARM LIST IS STILL THIS TICK'S OWN. A
    // return that cleared every held lamp during the breath already emptied
    // the record; resurrecting it here with a phase would hold a lamp the
    // operator just put out. A lamp whose schedule came back empty (a budget
    // too short to fit even one fade) keeps its bare, phaseless entry.
    if held_lamps(state).as_deref() == Some(held_now.as_slice()) {
        // WALKED OVER `breathing` AND NOT OVER THE BARE PATHS, because a phase
        // carries the STATE it belongs to and that is the one place still
        // holding it. The two lists are the same paths in the same order:
        // `held_now` is this one, mapped.
        let phased: Vec<pns::lights::HeldEntry> = breathing
            .iter()
            .map(|entry| {
                landings
                    .iter()
                    .find(|(landed_path, _, _)| *landed_path == entry.path)
                    // THE RESOLVE IS PART OF THE OFFSET. A landing is reported
                    // from the DRIVER's own start, which is `spent_ms` after
                    // this tick's, so a record written without that term would
                    // put every end a whole resolve early and the next tick
                    // would take the breath over before this one finished it.
                    .map(
                        |(path, landed_on, end_relative_ms)| pns::lights::HeldEntry {
                            path: path.clone(),
                            resume: Some(pns::lights::Phase {
                                end_unix_ms: now_ms + spent_ms + end_relative_ms,
                                landed_on: *landed_on,
                                held: entry.held,
                            }),
                        },
                    )
                    .unwrap_or_else(|| pns::lights::HeldEntry::bare(entry.path.clone()))
            })
            .collect();
        let _ = remember_held(state, &phased);
    }
    complaints
}
/// Where a lights tick holds the whole house for as long as it is driving it.
///
/// THE DAEMON'S OWN BOOKKEEPING IS NOT A LOCK. `decide` refuses to fire a
/// second lights child while the first is still listed, and that list is ONE
/// process's memory: a tick the operator ran by hand and an orphan left behind
/// by a daemon replacement are both invisible to it. Two ticks driving one lamp
/// interleave their fades against two schedules, and the phase the LAST of them
/// writes is the one the next tick resumes off, so the breath it picks up is
/// one no lamp was ever running. A file the operating system arbitrates is the
/// only guard every writer can see.
///
/// IT DOES NOT LOCK OUT THE EVENT PATH, deliberately. The operator's return
/// clears the held record from a process that holds no lock and must never wait
/// on one; `run_tick_writes` re-reads the record instead and stands down when
/// it moved, which is the guard that case has always had.
const LIGHTS_TICK_LOCK: &str = "lights-tick.lock";

/// How long a lights tick's lock is believed before it is read as an orphan.
///
/// `child_bound`'S OWN ARITHMETIC FOR THIS JOB, because it bounds the same
/// process: the longest interval the config permits, plus the longest a single
/// write may take at that interval, plus the second the daemon takes to notice
/// the child is gone. A tick still holding the lock past that has already been
/// killed, so the file is leavings. Standing down for a live holder costs one
/// interval of an unchanged lamp; stealing the lock from one that is still
/// driving is the failure the lock exists to stop, so the bound errs long.
fn lights_tick_stale_secs() -> u64 {
    pns::config::MAX_REFRESH_SECS
        + tick_bridge_deadline(pns::config::MAX_REFRESH_SECS).as_secs()
        + 1
}
/// How long ONE of a tick's bridge calls may take.
///
/// A FIFTH OF THE INTERVAL, so the three the resolve makes cannot outlive the
/// child that makes them AND still leave a breath: at the transport's own ten
/// seconds they outlive every interval the config permits, and a wedged bridge
/// would then have tick after tick piling up, each still dialling while the next
/// was spawned. A fifth is what keeps a full cycle of the shortest locked shape
/// inside what is left even when all three calls run to their deadline, which is
/// the whole point of the child staying alive.
///
/// A SECOND AT LEAST, which the division cannot reach anyway inside the config's
/// own bounds; a bridge on the same LAN answers these in milliseconds either
/// way.
pub(crate) fn tick_bridge_deadline(refresh_secs: u64) -> Duration {
    Duration::from_secs((refresh_secs / 5).max(1))
}

#[cfg(test)]
#[path = "lights_tick_writes/tests/lifecycle.rs"]
mod lifecycle_tests;

#[cfg(test)]
#[path = "lights_tick_writes/tests/budget.rs"]
mod budget_tests;

#[cfg(test)]
#[path = "lights_tick_writes/tests/ownership.rs"]
mod ownership_tests;

#[cfg(test)]
#[path = "lights_tick_writes/tests/phase.rs"]
mod phase_tests;

#[cfg(test)]
#[path = "lights_tick_writes/tests/routing.rs"]
mod routing_tests;
