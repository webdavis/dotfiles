//! Who an event is from: which checkout, which branch, which session, and
//! what that session was asked to do.
//!
//! THE HEADER'S FIELDS AND NOTHING ELSE. The composition lives in
//! `pns_domain::render`, the row lives in the sessions store, and this module
//! is the composition root's wiring between the two: one git read and one row
//! touch per event, both bounded, neither able to hold a notification.

use crate::*;
use pns_adapters::{SessionNote, SqliteStore, git_checkout};

/// The sender's own fields, as an `EventArgs` for the caller to fill in with
/// its state, detail and pane.
///
/// THE PROJECT IS THE REPOSITORY. A linked worktree's directory is named for
/// its branch, so the last segment of `cwd` answered the branch slug where
/// the operator's question is about the repository; git's common directory is
/// what tells them apart, and the cwd's own name is the fallback outside a
/// repository.
pub(crate) fn attribution(payload: &HookPayload, agent: &str) -> pns_domain::EventArgs {
    attributed(&sessions(), payload, agent)
}

/// The prompt hook's own write: the FIRST prompt of a session is what names
/// it, so a later prompt does not relabel the events that came before.
///
/// NO GIT AND NO NETWORK, because this runs inside the synchronous prompt
/// hook: one local row write, and the checkout is left to the first event
/// that actually reads it.
pub(crate) fn name_session(payload: &HookPayload, agent: &str) {
    named(&sessions(), payload, agent);
}

fn sessions() -> SqliteStore {
    SqliteStore::new(state_dir())
}

fn attributed(store: &SqliteStore, payload: &HookPayload, agent: &str) -> pns_domain::EventArgs {
    let checkout = git_checkout(&payload.cwd);
    let project = match checkout.repository.is_empty() {
        true => project_of(&payload.cwd),
        false => checkout.repository,
    };
    let session = tracked(&payload.session_id).unwrap_or_default();
    let title = noted(
        store,
        &SessionNote {
            id: session,
            harness: agent,
            project: &project,
            branch: &checkout.branch,
            title: "",
            now: stamp(),
        },
    );
    pns_domain::EventArgs {
        session: session.to_string(),
        session_title: title,
        project,
        branch: checkout.branch,
        ..Default::default()
    }
}

fn named(store: &SqliteStore, payload: &HookPayload, agent: &str) -> String {
    noted(
        store,
        &SessionNote {
            id: tracked(&payload.session_id).unwrap_or_default(),
            harness: agent,
            project: "",
            branch: "",
            title: &session_label(payload),
            now: stamp(),
        },
    )
}

/// Record the session and answer the title it is known by. An id pns cannot
/// track names no row at all.
fn noted(store: &SqliteStore, note: &SessionNote<'_>) -> String {
    if note.id.is_empty() {
        return String::new();
    }
    match store.note_session(note) {
        Ok(title) => title,
        Err(error) => {
            eprintln!("pns: state error (the session could not be recorded: {error}); no title");
            String::new()
        }
    }
}

/// A clock that answers nothing still records the session, because the name
/// is what the header needs; the stale-block escalation reads its own column
/// rather than these stamps.
fn stamp() -> u64 {
    now_secs().unwrap_or_default()
}

/// The session id, when it is one pns tracks at all.
///
/// THE SAME PREDICATE THAT GOVERNS A MARKER FILE. An id that cannot name a
/// file is not a session this engine follows, and the prompt hook's own test
/// pins that such an id writes nothing whatever under the state directory.
fn tracked(session_id: &str) -> Option<&str> {
    pns_domain::safety::session_id_is_safe(session_id).then_some(session_id)
}

/// What names a session on the header's second line: the harness's own title
/// when it sent one, else the prompt that started the session, cut to the
/// line budget. Both arrive already flattened, so neither can become two
/// Discord lines.
fn session_label(payload: &HookPayload) -> String {
    let stated = [&payload.session_title, &payload.prompt]
        .into_iter()
        .find(|line| !line.is_empty())
        .map(String::as_str)
        .unwrap_or_default();
    render::clipped(stated, render::SESSION_TITLE_MAX_CHARS)
}

#[cfg(test)]
#[path = "sender/tests.rs"]
mod sender_tests;
