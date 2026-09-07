use crate::*;

/// The spool name the tick job is registered under. ONE JOB FOR THE WHOLE
/// HOUSE, not one per lamp: the tick derives every state from scratch and
/// writes every fixture, so a second job would be a second writer of the same
/// bulbs.
pub(crate) const LIGHTS_JOB: &str = "lights";
/// How long the tick runs on after an ordinary event. A working loop emits
/// events constantly, so five minutes covers an agent's thinking gap without
/// covering a stall.
pub(crate) const ORDINARY_LEASE_SECS: u64 = 300;
/// And after a journalled one, which is an operator who is away or muted. The
/// glow has to survive the whole absence, and the absence is precisely when no
/// further event arrives to refresh this.
const JOURNALLED_LEASE_SECS: u64 = 12 * 60 * 60;
/// Put out whatever a steady glow write is still holding, and forget it.
///
/// THE FILE IS THE FENCE. An ordinary event reads whether it exists and stops
/// there, so every event that is not a return from an absence costs one failed
/// open and no network at all.
///
/// IT FORGETS EVEN THOUGH THE WRITE MIGHT HAVE FAILED, and the cost is stated
/// rather than coded around: `put` is fire and forget, so a refused clear is
/// invisible and the lamp stays lit with nothing recorded to put it out. That
/// is the same exposure the steady write already carries by not expiring, and
/// the alternative is worse: a record kept until somebody proved the write
/// landed would have every later event re-clearing a lamp that is already
/// dark, forever, on a machine whose daemon is down.
pub(crate) fn clear_held_lamps(settings: Option<&toml::Table>) {
    let state = state_dir();
    // A RECORD THIS CANNOT READ NAMES NO LAMP TO PUT OUT, and it is KEPT: the
    // clear works off names alone, so there is nothing to write, and forgetting
    // the file would take the tick's only chance of repairing it with it.
    let Some(held) = held_lamps(&state) else {
        return;
    };
    if held.is_empty() {
        return;
    }
    let Some(hue) = settings.and_then(|settings| {
        hue_settings(settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref())
    }) else {
        return;
    };
    pns::channels::hue::clear_held(
        &UreqBridge {
            base: format!("https://{}/clip/v2/resource", hue.bridge),
            key: hue.key,
            deadline: BRIDGE_DEADLINE,
        },
        &held,
    );
    // The failure is DROPPED here, in this function's own stated style: the
    // PUTs are already out, so the worst a failed forget costs is one more
    // clear of a lamp that is already dark.
    let _ = remember_held(&state, &[]);
}
/// Register the repeating tick, or drop the refusal.
///
/// THE FAILURE IS DROPPED, exactly as `record_decision`'s is and for the same
/// reason: a lamp that did not re-arm must never cost a card, a line of stdout
/// or an exit code. `daemon::schedule` returns its error rather than printing
/// it precisely so each caller can state its own direction, and this one drops
/// it.
///
/// IT CANNOT BLOCK. The registration is one file written by rename into a
/// directory; there is no connection, no handshake and nothing to wait on, so
/// a daemon that is dead, wedged or mid-restart changes nothing about this
/// call.
///
/// TWO LEASE LENGTHS, off ONE question: was this event journalled. An ordinary
/// event means the operator is here and a working loop emits events
/// constantly, so five minutes covers an agent's thinking gap without covering
/// a stall. A journalled one means they are away or muted, which is exactly
/// when no further event will arrive to refresh this, and the glow has to
/// survive the whole absence.
///
/// THE DUE SECOND IS KEPT WHEN ONE IS ALREADY PENDING, and that is not
/// decoration: re-registering replaces the job by name, so an event storm that
/// pushed `due` out to `now + refresh` every time would keep moving the tick
/// away from itself and a busy machine's lamps would never be re-armed at all.
/// The lease is what every event refreshes; the schedule is left where the
/// last tick put it.
pub(crate) fn register_lights_tick(
    lights: Option<&pns::config::Lights>,
    decision: &pns::engine::Decision,
    overrides: &Overrides,
) {
    // THE DECISION'S OWN CLOCK, like record_news and renew_loop_lease beside
    // this call: a fresh wall-clock read here would be a second reading of the
    // same moment, which is exactly the boundary R4-1 exists to close. NO
    // CLOCK IS NO REGISTRATION, never a job due at epoch zero.
    let (Some(lights), Some(now)) = (lights, decision.inputs.now_secs) else {
        return;
    };
    let lease = if pns::missed_notifications::was_missed(decision, overrides) {
        JOURNALLED_LEASE_SECS
    } else {
        ORDINARY_LEASE_SECS
    };
    schedule_lights_tick(&state_dir(), lights, now, lease);
}
/// The tick registered to run for the next `lease_secs`, keeping whatever due
/// second is already pending.
///
/// THREE CALLERS AND ONE REGISTRATION, because the tick's lease is what decides
/// whether a lamp can EVER light, and three spellings of it would be three
/// answers. An event refreshes it, a lease taken by hand starts it, and the
/// tick renews its own while anything is still in flight.
///
/// THE DUE SECOND IS KEPT WHEN ONE IS ALREADY PENDING, and that is not
/// decoration: re-registering replaces the job by name, so an event storm that
/// pushed `due` out to `now + refresh` every time would keep moving the tick
/// away from itself and a busy machine's lamps would never be re-armed at all.
/// The lease is what every caller refreshes; the schedule is left where the
/// last tick put it.
pub(crate) fn schedule_lights_tick(
    state: &Path,
    lights: &pns::config::Lights,
    now: u64,
    lease_secs: u64,
) {
    let pending =
        match pns::daemon::peek(&pns::daemon::spool_dir(state).join(LIGHTS_JOB), LIGHTS_JOB) {
            pns::daemon::Peeked::Job(job) => Some(job.due),
            _ => None,
        };
    let due = pending
        .filter(|due| *due > now)
        .unwrap_or_else(|| now.saturating_add(lights.refresh_secs));
    let job = pns::daemon::Job {
        id: LIGHTS_JOB.to_string(),
        due,
        // AT LEAST AS FAR AS THE DUE SECOND, because a lease that ended before
        // its own job's first run is a record `validate_shape` refuses, and a
        // refused registration is a lamp that never re-arms with nothing said
        // anywhere. It bites for any refresh interval longer than the ordinary
        // lease, which the config permits up to a day.
        until: due.max(now.saturating_add(lease_secs)),
        every: Some(lights.refresh_secs),
        unless_marker: None,
        args: vec!["lights".to_string(), "tick".to_string()],
    };
    // The failure is DROPPED here and nowhere else: see the doc comment.
    let _ = pns::daemon::schedule(state, &job, now);
}
