use super::{attributed, named, session_label};
use crate::runtime_test_support::scratch;
use pns_adapters::{HookPayload, SqliteStore};

fn store() -> SqliteStore {
    SqliteStore::new(scratch("sender"))
}

#[test]
fn an_event_with_no_prompt_of_its_own_carries_the_title_the_first_prompt_stored() {
    // The Stop hook's payload names no prompt and Claude Code sends no
    // session title on it, so the label can only come from the store.
    let store = store();
    let payload = HookPayload {
        session_id: "s1".to_string(),
        prompt: "arm posture alert and retire the Bash alerter".to_string(),
        ..HookPayload::default()
    };
    named(&store, &payload, "claude", &session_label(&payload));
    let event = attributed(
        &store,
        &HookPayload {
            session_id: "s1".to_string(),
            ..HookPayload::default()
        },
        "claude",
    );
    assert_eq!(
        event.session_title,
        "arm posture alert and retire the Bash alerter"
    );
    assert_eq!(event.session, "s1");
}

#[test]
fn a_session_id_that_could_not_name_a_file_names_no_session_either() {
    let event = attributed(
        &store(),
        &HookPayload {
            session_id: "../../etc/passwd".to_string(),
            ..HookPayload::default()
        },
        "claude",
    );
    assert_eq!(
        (event.session.as_str(), event.session_title.as_str()),
        ("", "")
    );
}

#[test]
fn the_harnesss_own_session_title_outranks_the_first_prompt() {
    assert_eq!(
        session_label(&HookPayload {
            session_title: "posture alert cutover".to_string(),
            prompt: "arm posture alert and retire the Bash alerter".to_string(),
            ..HookPayload::default()
        }),
        "posture alert cutover"
    );
}

#[test]
fn a_long_first_prompt_is_cut_to_the_headers_own_line_budget() {
    let label = session_label(&HookPayload {
        prompt: "a".repeat(200),
        ..HookPayload::default()
    });
    assert_eq!(label.chars().count(), 60);
    assert!(label.ends_with('…'), "a cut line says it was cut: {label}");
}
