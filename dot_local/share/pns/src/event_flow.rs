use crate::*;

mod records;
mod submit;
use records::EventRecords;
pub(crate) use submit::submit_mode;

/// Whether this is the event's FIRST delivery, a NUDGE about one already
/// recorded, or an OBSERVATION.
///
/// ONE ARGUMENT RATHER THAN A SECOND EVENT PATH. A nudge is an ordinary event
/// in every respect an operator can see (the mute, the named Focus modes, the
/// quiet window, the surface and visibility plan, fresh probes taken in the
/// nudge's own process); what it is not is a second OCCURRENCE, and the
/// contiguous tail of `run_event` is what records occurrences.
///
/// AN OBSERVATION IS THE SAME KIND OF NON-OCCURRENCE, for a different reason:
/// it is a harness telling pns about something that happened rather than a
/// turn needing the operator's attention, so it changes no workflow or marker
/// state and is routed marker-neutral through the same tail a nudge skips.
/// It is still recorded as a decision (`record_decision` runs before the
/// guard for every attempt), just with `nag=no`.
///
/// AN OBSERVATION SHAPED LIKE A `PermissionRequest` IS TOO LATE TO GATE HERE.
/// `blocking_event` forwards to moshi and arms the nag before `run_event`
/// ever runs, so this guard cannot undo either one; a caller on that path
/// must refuse the observation at the top of `blocking_event` itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Attempt {
    First,
    Nudge,
    Observation,
}
/// One notification, end to end: decide, render, dispatch. THE one event path,
/// whether the event came from argv or from a harness hook.
///
/// THE PAYLOAD RIDES BESIDE THE EVENT RATHER THAN INSIDE IT, and the split is
/// the point: `EventArgs` is the ARGV contract, and argv has no spelling for a
/// session id, a permission mode, a subagent id or a raw tool name. Every one
/// of those arrives in a harness payload or not at all, so the hook arms pass
/// what they were given and every other caller passes `HookPayload::default()`,
/// which is honestly no identity rather than fields nothing can fill. The
/// lamps' needs marker and the decision line are its readers.
pub(crate) fn run_event(
    event: &pns::args::EventArgs,
    probes: &SystemProbes<SystemCommandRunner>,
    payload: &HookPayload,
    attempt: Attempt,
) {
    run_event_pulsing(
        event,
        probes,
        payload,
        attempt,
        &|table, lights, behaviour, presence| {
            fire_pulse_unless_quiet(table, lights, behaviour, presence);
        },
    );
}
/// Where this event's pulse ends up. THE REAL PULSE IN PRODUCTION and a
/// recorder in the one test that is about the ORDERING of this path rather
/// than about a lamp.
///
/// A SEAM AND NOT AN ABSTRACTION. There is exactly one implementation besides
/// the real pulse, it exists because the readings the pulse is handed are
/// taken hundreds of lines above it, and no other reading on this path is
/// injectable. A probe set is already the seam for everything else.
type PulseSink<'a> = &'a dyn Fn(
    Option<toml::Table>,
    Option<&pns::config::Lights>,
    pns::config::Behaviour,
    Option<&pns::presence_policy::Snapshot>,
);
mod execution;
fn run_event_pulsing(
    event: &pns::args::EventArgs,
    probes: &SystemProbes<SystemCommandRunner>,
    payload: &HookPayload,
    attempt: Attempt,
    pulse: PulseSink<'_>,
) {
    let _ = execution::execute(event, probes, payload, attempt, pulse, None);
}

#[cfg(test)]
#[path = "event_flow/tests.rs"]
mod event_flow_tests;
