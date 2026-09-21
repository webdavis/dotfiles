//! One durable row per harness hook event, in the activity store.
//!
//! THE HOOKS ARE THE ONLY WRITERS. A producer submission and an argv event
//! reach `run_event` too, and neither is agent activity: the store answers
//! what the agents did in a window, so `hook_event` is the one call that
//! records and every other caller of the event path records nothing.
//!
//! AN UNTRACKED SESSION WRITES NOTHING, the same predicate a marker file is
//! held to: an id pns does not follow names no row, here or in `sessions`.

use crate::*;
use pns_adapters::{ActivityEvent, SqliteStore};

/// Record this event, reading the transcript once for what only it knows.
///
/// A STORE FAILURE COSTS ONE ROW. The caller is a hook whose contract is to
/// notify, so the refusal is said on stderr and the event carries on.
pub(crate) fn record(event: &pns_domain::EventArgs, payload: &HookPayload) {
    if !pns_domain::safety::session_id_is_safe(&payload.session_id) {
        return;
    }
    let facts =
        pns_adapters::session_facts(&crate::turn_text::transcript_tail(&payload.transcript_path));
    store(
        event,
        payload,
        session_title(&facts, &event.session_title),
        model(payload, &facts),
    );
}

/// Record the `resolved` event, with no transcript read: it fires once per
/// tool batch, where a 4MB read and parse on every call is no longer the
/// payload read and a parse the caller's own comment promises.
pub(crate) fn record_session_only(event: &pns_domain::EventArgs, payload: &HookPayload) {
    if !pns_domain::safety::session_id_is_safe(&payload.session_id) {
        return;
    }
    store(
        event,
        payload,
        event.session_title.clone(),
        payload.to_model.clone(),
    );
}

fn store(
    event: &pns_domain::EventArgs,
    payload: &HookPayload,
    session_title: String,
    model: String,
) {
    let row = ActivityEvent {
        at: now_secs().unwrap_or_default(),
        agent: event.agent.clone(),
        state: event.state.clone(),
        project: event.project.clone(),
        branch: event.branch.clone(),
        session: payload.session_id.clone(),
        session_title,
        pane: event.pane.clone(),
        workspace: std::env::var("HERDR_WORKSPACE_ID").unwrap_or_default(),
        model,
        title: pns_domain::render::title(&event.agent, &event.state, &event.project),
        detail: event.detail.clone(),
        transcript_path: payload.transcript_path.clone(),
    };
    if let Err(error) = SqliteStore::new(state_dir()).record_activity_event(&row) {
        eprintln!("pns: state error (this event could not be recorded: {error})");
    }
}

/// What names this session, in the order the recap design states: the name the
/// operator gave it, then the harness's own generated one, then what the
/// session was asked to do, which `sessions` already holds from the first
/// prompt.
///
/// THE TRANSCRIPT'S TEXT IS FLATTENED AND CUT, like every other line a card or
/// a page is built from: it is text pns did not write, and a title spanning two
/// lines becomes two Discord lines.
fn session_title(facts: &pns_adapters::SessionFacts, stored: &str) -> String {
    let stated = [&facts.custom_title, &facts.ai_title]
        .into_iter()
        .map(|title| pns_adapters::flattened(title))
        .find(|title| !title.is_empty())
        .unwrap_or_default();
    match stated.is_empty() {
        true => stored.to_string(),
        false => pns_domain::render::clipped(&stated, pns_domain::render::SESSION_TITLE_MAX_CHARS),
    }
}

/// The model this session is on: the switch event's own new model when this IS
/// one, else the newest assistant line of the transcript, which is where every
/// other event has to read it from.
fn model(payload: &HookPayload, facts: &pns_adapters::SessionFacts) -> String {
    match payload.to_model.is_empty() {
        true => facts.model.clone(),
        false => payload.to_model.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::session_title;
    use pns_adapters::SessionFacts;

    #[test]
    fn the_title_is_the_operators_name_then_the_generated_one_then_the_stored_one() {
        let facts = |custom: &str, ai: &str| SessionFacts {
            custom_title: custom.to_string(),
            ai_title: ai.to_string(),
            model: String::new(),
        };
        assert_eq!(
            session_title(
                &facts("the name I gave it", "generated"),
                "the first prompt"
            ),
            "the name I gave it"
        );
        assert_eq!(
            session_title(&facts("", "generated"), "the first prompt"),
            "generated"
        );
        assert_eq!(
            session_title(&facts("", ""), "the first prompt"),
            "the first prompt",
            "a Codex session states neither, so the sessions row is the answer"
        );
    }
}
