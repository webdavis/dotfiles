use crate::*;

/// The turn's start marker, so the Stop hook can measure the turn that just
/// finished rather than the whole session.
pub(crate) fn start_of_turn(payload: &HookPayload) {
    let Some(marker) = turn_marker(&payload.session_id) else {
        return;
    };
    if let Some(parent) = marker.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Only when none is there: a second prompt inside one turn must not
    // restart the clock.
    // NO CLOCK IS NO MARKER, never a marker at epoch zero: the same rule
    // `update_blocked_marker` states beside its own clock. A marker at zero
    // would measure the turn from 1970, so `consume_turn_marker` would call a
    // two-second turn long-running and it would earn the watch card and the
    // pulse; no marker measures nothing, and `session_was_long` reads that as
    // not long.
    if !marker.exists()
        && let Some(now) = now_secs()
    {
        let _ = std::fs::write(&marker, now.to_string());
    }
}
/// The turn's marker path, or None for a session id that cannot become a
/// filename. The id arrives in the harness payload, and `..` in it would
/// escape the state directory.
fn turn_marker(session_id: &str) -> Option<std::path::PathBuf> {
    if !pns::safety::session_id_is_safe(session_id) {
        return None;
    }
    Some(state_dir().join(format!("session-{session_id}.start")))
}
/// How long the finished turn ran, CLAIMING the marker first.
///
/// The claim is a rename, which is atomic: two Stops racing the same turn
/// cannot both read it and both pulse, because only one rename can succeed.
/// Reading first and unlinking after left that window open, and an unlink
/// that failed left the marker wedged for every later turn.
///
/// It runs BEFORE the reply and the condenser for the same reason. Stop is
/// asynchronous, so the next prompt can arrive while this one is still
/// condensing: with the marker still on disk that prompt writes nothing, and
/// this Stop then deletes the marker its successor was relying on. Claiming
/// up front also keeps the condenser's own latency out of the elapsed time it
/// is measuring.
///
/// The value is VALIDATED before it reaches arithmetic: a truncated write or
/// a hand edit must be a decision, not a crash.
fn consume_turn_marker(session_id: &str) -> Option<u64> {
    let marker = turn_marker(session_id)?;
    let claim = marker.with_extension(format!("claim.{}", std::process::id()));
    std::fs::rename(&marker, &claim).ok()?;
    let started = std::fs::read_to_string(&claim);
    let _ = std::fs::remove_file(&claim);
    let started: u64 = started.ok()?.trim().parse().ok()?;
    Some(now_secs()?.saturating_sub(started))
}
/// The Stop hook: what the turn said, and whether it ran long enough to earn
/// the lights.
pub(crate) fn end_of_turn(payload: &HookPayload, agent: &str) {
    // FIRST, before anything slow: see consume_turn_marker.
    let elapsed = consume_turn_marker(&payload.session_id);
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
    let elapsed = consume_turn_marker(&payload.session_id);
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
/// The branch the work happened on, or none. Bounded like every other spawn:
/// a wedged git must not hold a notification.
fn git_branch(cwd: &str) -> String {
    if cwd.is_empty() || !std::path::Path::new(cwd).is_dir() {
        return String::new();
    }
    let mut command = Command::new("git");
    command.args(["-C", cwd, "branch", "--show-current"]);
    run_bounded(command, None, GIT_DEADLINE, PROBE_READ_MAX)
        .map(|branch| branch.trim().to_string())
        .unwrap_or_default()
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
/// A branch lookup is a local read; anything slower than this is a wedged
/// repository, not an answer worth waiting for.
const GIT_DEADLINE: Duration = Duration::from_secs(5);
