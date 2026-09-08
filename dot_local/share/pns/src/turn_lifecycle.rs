use crate::*;

pub(crate) fn start_of_turn(payload: &HookPayload) {
    pns_adapters::turn_markers::start_of_turn(&state_dir(), &payload.session_id, now_secs());
}

/// The Stop hook: what the turn said, and whether it ran long enough to earn
/// the lights.
pub(crate) fn end_of_turn(payload: &HookPayload, agent: &str) {
    // FIRST, before anything slow: see consume_turn_marker.
    let elapsed = pns_adapters::turn_markers::consume_turn_marker(
        &state_dir(),
        &payload.session_id,
        now_secs,
    );
    // AND THE FREE CLEARING SIGNAL WITH IT. A turn cannot end while one of its
    // own approvals is unanswered, so a turn end proves resolution. It costs one
    // function call, no hook declaration and no apply, and it is the backstop
    // for a batch payload over the 1MB cap, an operator who escaped the prompt
    // instead of answering it, and the window between this merge and the apply
    // that installs the PostToolBatch entry.
    clear_nag(&payload.session_id);
    let reply = turn_reply(payload);
    let (state, detail) = match reply.is_empty() {
        true => ("done".to_string(), String::new()),
        false => condense(&reply),
    };
    run_event(
        &pns::args::EventArgs {
            agent: agent.to_string(),
            state,
            project: project_of(&payload.cwd),
            branch: git_branch(&payload.cwd),
            detail,
            pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
            long_running: pns::pulse::session_was_long(elapsed, Some(pulse_threshold_secs())),
            ..Default::default()
        },
        &system_probes(),
        payload,
        Attempt::First,
    );
}
/// The StopFailure hook: a turn that died on an API error reports itself,
/// where it used to report nothing at all.
///
/// THE MARKER IS CLAIMED HERE for the same reason `end_of_turn` claims it, and
/// this is the arm that used to leak it: StopFailure fires INSTEAD of Stop, so
/// a dead turn left its marker on disk, the next prompt found one and declined
/// to rewrite the clock, and the turn after that was measured from the dead
/// turn's start. `long_running` is what raises the mobile watch card and the
/// pulse, so one API error promoted later short turns to the long-running tier
/// for the rest of the session.
///
/// NO CONDENSER AND NO TRANSCRIPT. The condenser is a model call on the one
/// path where a model call has just failed, the reply's fallback re-reads the
/// transcript in a bounded loop of sleeps, and neither recovers the news: the
/// harness states it as a plain string that is never empty. The payload's
/// partial `last_assistant_message` is dropped for the same reason, since the
/// question at a dead pane is why it stopped rather than what it had said.
pub(crate) fn failed_turn(payload: &HookPayload, agent: &str) {
    let elapsed = pns_adapters::turn_markers::consume_turn_marker(
        &state_dir(),
        &payload.session_id,
        now_secs,
    );
    // The same free clear `end_of_turn` takes, for the same reason: StopFailure
    // fires INSTEAD of Stop, so without it a dead turn leaves its approval armed.
    clear_nag(&payload.session_id);
    run_event(
        &pns::args::EventArgs {
            agent: agent.to_string(),
            state: "failed".to_string(),
            project: project_of(&payload.cwd),
            branch: git_branch(&payload.cwd),
            detail: payload.message.clone(),
            pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
            long_running: pns::pulse::session_was_long(elapsed, Some(pulse_threshold_secs())),
            ..Default::default()
        },
        &system_probes(),
        payload,
        Attempt::First,
    );
}
/// The project an event belongs to: the last segment of the working directory.
pub(crate) fn project_of(cwd: &str) -> String {
    cwd.rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or_default()
        .to_string()
}
/// How long a turn must run to earn the lights.
fn pulse_threshold_secs() -> u64 {
    std::env::var("PNS_PULSE_THRESHOLD_SECS")
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(pns::pulse::DEFAULT_LONG_SESSION_SECS)
}
