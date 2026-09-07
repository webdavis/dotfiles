use crate::*;

/// The lamp-narrowing ring: one line per narrowing decision, `KEPT` deep,
/// beside `decisions`. Its own file rather than a field on the decision ring,
/// because the tick writes it too and the tick decides no event at all.
const PRESENCE_DECISIONS: &str = "presence-decisions";
/// The probe set for ONE invocation. Built here and shared, never per
/// consumer: see `SystemProbes`.
pub(crate) fn system_probes() -> SystemProbes<SystemCommandRunner> {
    let home = std::env::var("HOME").unwrap_or_default();
    SystemProbes::new(
        SystemCommandRunner,
        resolve_path(
            std::env::var("PNS_PHONE_MARKER_FILE").ok().as_deref(),
            &format!("{home}/.local/state/pns/phone-attention.marker"),
        )
        .to_string_lossy()
        .into_owned(),
    )
    // OFF `state_dir`, which is where the daemon publishes it and which
    // `PNS_STATE_DIR` already redirects, so the reading follows a sandboxed
    // run without a second environment knob of its own.
    .with_presence_path(
        state_dir()
            .join(pns::presence_file::STATE_FILE)
            .to_string_lossy()
            .into_owned(),
    )
}
/// What the room sensor reads right now, off ONE probe set: the line and the
/// clock it is aged against come from the same set, so the two cannot
/// straddle a boundary.
///
/// A TABLE THIS COULD NOT READ IS NO READING, never a room: the refusal was
/// already printed, and inventing a room out of settings nobody could parse
/// is the fail-open this whole reading is shaped to avoid.
pub(crate) fn presence_status(
    settings: Option<&pns::config::Presence>,
) -> pns::presence::PresenceStatus {
    match settings {
        Some(settings) => presence_reading(settings).0,
        None => pns::presence::PresenceStatus::Unknown(pns::presence::Unreadable::NoReading),
    }
}
/// The reading and the clock it was aged against, off ONE probe set.
///
/// THE CLOCK COMES BACK OUT with the verdict rather than being re-read by
/// whoever wants to stamp a line with it: a second read is a second moment.
fn presence_reading(
    settings: &pns::config::Presence,
) -> (pns::presence::PresenceStatus, Option<u64>) {
    let probes = system_probes();
    let now = probes.now_secs();
    (
        pns::presence::classify(
            probes
                .presence_line()
                .as_deref()
                .and_then(pns::presence_file::parse_presence_line),
            now,
            settings.stale_after_secs,
            &settings.rooms,
            &settings.exclude,
        ),
        now,
    )
}
/// What the room sensor and the machine's own clocks say, or `None` for a
/// machine that never armed the table.
///
/// `None` REACHES NO NEW CODE AT ALL: no narrowing, no record, and a lamp map
/// that behaves exactly as it did before this feature existed.
///
/// EVERY READING COMES OFF THE CALLER'S OWN PROBE SET, which is what makes
/// this one moment rather than several. `SystemProbes` memoizes the clock, the
/// idle counter, the screen lock and the presence line, so a caller that hands
/// its own set in cannot have the reading aged against one clock and the
/// decision made against another. Building a fresh set here is exactly the
/// boundary that let a reading fresh at 14 seconds be stale at 16.
pub(crate) fn presence_snapshot<R: pns::system::CommandRunner>(
    settings: Option<&pns::config::Presence>,
    probes: &SystemProbes<R>,
    desk_idle_secs: Option<u64>,
    screen_locked: Option<bool>,
    home: pns::home::HomePresence,
) -> Option<pns::presence_policy::Snapshot> {
    let settings = settings?;
    let now = probes.now_secs();
    Some(pns::presence_policy::Snapshot {
        status: pns::presence::classify(
            probes
                .presence_line()
                .as_deref()
                .and_then(pns::presence_file::parse_presence_line),
            now,
            settings.stale_after_secs,
            &settings.rooms,
            &settings.exclude,
        ),
        desk_idle_secs,
        screen_locked,
        home,
        desk_room: settings.desk_room.clone(),
        desk_stale_after_secs: settings.desk_stale_after_secs,
        now,
    })
}
/// What the router says about the phone, for the narrowing's away gate.
///
/// `Unknown` TODAY, AND THE GATE IS WHERE IT BELONGS RATHER THAN LIVE. Reading
/// it means two `UniFiRouter` calls at five seconds apiece
/// (`home::ROUTER_DEADLINE`), and both callers of the narrowing are behind a
/// lamp: the event path's pulse already spends a bridge deadline after every
/// channel has fired, and the tick's whole budget is one refresh interval it
/// also has to fit three bridge calls and a breath into. `Unknown` is the
/// fail-open direction the policy already states, so the cost of not asking is
/// exactly one gate: motion in a watched room still carries the lamps.
///
/// SO THE GATE IS DORMANT, and the residue is stated rather than left to be
/// re-derived: while this answers `Unknown`, an operator out of the house with
/// somebody else moving in a watched room narrows the lamps to that room.
/// Presence only ever narrows which lamp signals, and the operator is not in
/// the house to see any lamp, so that wrong narrow costs them nothing a full
/// write would have delivered; the signal that reaches them when they are away
/// is the phone, which no lamp decision touches. `presence_room::chosen` holds
/// the same reasoning at the gate itself.
///
/// ponytail: a stale-bounded reading published by a daemon poll, in
/// `presence poll`'s own shape, is what makes this live; it is filed as B102.
pub(crate) fn home_presence() -> pns::home::HomePresence {
    pns::home::HomePresence::Unknown
}
/// The routing narrowed to the room the operator is in, with the decision
/// appended to its ring.
///
/// THE RECORD IS FAIL-QUIET, in `record_decision`'s exact style and for its
/// exact reason: both callers run where a printed line about the state
/// directory would be a line in every hook's output or in a tick that runs
/// three times a minute forever.
pub(crate) fn narrow_to_presence(
    state: &Path,
    routing: pns::channels::hue::Routing,
    presence: Option<&pns::presence_policy::Snapshot>,
) -> pns::channels::hue::Routing {
    let Some(snapshot) = presence else {
        return routing;
    };
    let (narrowed, decision) = pns::presence_policy::narrow(routing, snapshot);
    let _ = append_ring_line(
        &state.join(PRESENCE_DECISIONS),
        &pns::presence_journal::entry(&pns::presence_journal::recorded(snapshot, &decision)),
        pns::decision_log::KEPT,
        RING_READ_MAX,
    );
    narrowed
}
/// The last narrowing this machine decided. `None` is a ring with nothing in
/// it, which is presence off or never yet consulted.
pub(crate) fn last_narrowing(state: &Path) -> Option<pns::presence_journal::Entry> {
    let contents =
        pns::system::readable_state_file(&state.join(PRESENCE_DECISIONS), RING_READ_MAX).ok()?;
    pns::presence_journal::last(&contents)
}

#[cfg(test)]
#[path = "presence_runtime/tests.rs"]
mod presence_runtime_tests;
