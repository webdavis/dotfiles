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
    clear_remind(&payload.session_id);
    let reply = turn_reply(payload);
    // A STATE THE SUMMARIZER READ OFF THE TURN IS A GUESS, and says so, so the
    // submit path can withhold a blocked marker a live loop makes wrong. An
    // empty reply states nothing to read, so `done` there is not a guess.
    let (state, detail, guessed) = match reply.is_empty() {
        true => ("done".to_string(), String::new(), false),
        false => {
            let (state, detail) = summarize(&reply);
            (state, detail, true)
        }
    };
    hook_event(
        &pns_domain::EventArgs {
            agent: agent.to_string(),
            state,
            detail,
            guessed,
            pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
            long_running: pns_domain::pulse::session_was_long(
                elapsed,
                Some(pns_domain::pulse::DEFAULT_LONG_SESSION_SECS),
            ),
            ..attribution(payload, agent)
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
/// NO SUMMARIZER AND NO TRANSCRIPT. The summarizer is a model call on the one
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
    clear_remind(&payload.session_id);
    hook_event(
        &pns_domain::EventArgs {
            agent: agent.to_string(),
            state: "failed".to_string(),
            detail: payload.message.clone(),
            pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
            long_running: pns_domain::pulse::session_was_long(
                elapsed,
                Some(pns_domain::pulse::DEFAULT_LONG_SESSION_SECS),
            ),
            ..attribution(payload, agent)
        },
        &system_probes(),
        payload,
        Attempt::First,
    );
}
/// The project a working directory names on its own: its last segment.
///
/// THE FALLBACK, NOT THE ANSWER. `sender::attribution` asks git for the
/// repository first, because a linked worktree's directory is named for its
/// branch; this is what answers outside a repository, where the directory is
/// all there is.
pub(crate) fn project_of(cwd: &str) -> String {
    cwd.rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or_default()
        .to_string()
}
/// The project a checkout is about: the repository git answered, or the
/// directory's own name when git answered none.
///
/// ONE RULE, TWO CALLERS. Session attribution and the recap both ask which
/// project a working directory is about, and two spellings of the answer would
/// let a recap land in a channel named for a worktree's branch slug while the
/// session events from that same directory land in the repository's.
pub(crate) fn named_project(repository: &str, cwd: &str) -> String {
    match repository.is_empty() {
        true => project_of(cwd),
        false => repository.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE MUTANT THIS PINS: the repository ignored in favour of the
    /// directory, which names a linked worktree after its branch slug and
    /// sends its recap to a channel nobody mapped.
    #[test]
    fn the_repository_names_the_project_and_the_directory_only_answers_without_one() {
        assert_eq!(
            named_project("dotfiles", "/Users/x/.herdr/worktrees/dotfiles/feat-y"),
            "dotfiles"
        );
        assert_eq!(named_project("", "/Users/x/workspaces/homelab/"), "homelab");
        assert_eq!(named_project("", ""), "");
    }
}
