use super::*;

fn event(at: u64, project: &str, session: &str, state: &str) -> Event {
    Event {
        at,
        agent: "claude".to_string(),
        state: state.to_string(),
        project: project.to_string(),
        branch: "main".to_string(),
        session: session.to_string(),
        ..Event::default()
    }
}

#[test]
fn events_group_by_project_then_session_in_first_seen_order() {
    let grouped = by_project(&[
        event(10, "dotfiles", "s1", "prompt"),
        event(20, "pns", "s2", "prompt"),
        event(30, "dotfiles", "s1", "done"),
        event(40, "dotfiles", "s3", "blocked"),
    ]);
    assert_eq!(
        grouped
            .iter()
            .map(|p| p.project.as_str())
            .collect::<Vec<_>>(),
        ["dotfiles", "pns"]
    );
    assert_eq!(
        grouped[0]
            .sessions
            .iter()
            .map(|s| s.session.as_str())
            .collect::<Vec<_>>(),
        ["s1", "s3"]
    );
}

#[test]
fn a_sessions_duration_is_its_first_event_to_its_last_inside_the_window() {
    let grouped = by_project(&[
        event(100, "dotfiles", "s1", "prompt"),
        event(8_140, "dotfiles", "s1", "done"),
    ]);
    assert_eq!(grouped[0].sessions[0].duration_secs, 8_040);
    assert_eq!(grouped[0].sessions[0].last_state, "done");
}

#[test]
fn a_single_event_session_ran_no_measurable_time() {
    let grouped = by_project(&[event(100, "dotfiles", "s1", "blocked")]);
    assert_eq!(grouped[0].sessions[0].duration_secs, 0);
}

#[test]
fn a_later_event_states_the_title_branch_and_model_and_an_empty_one_does_not() {
    let mut named = event(20, "dotfiles", "s1", "done");
    named.session_title = "the recap engine".to_string();
    named.model = "opus".to_string();
    named.branch = String::new();
    let grouped = by_project(&[event(10, "dotfiles", "s1", "prompt"), named]);
    let session = &grouped[0].sessions[0];
    assert_eq!(session.title, "the recap engine");
    assert_eq!(session.model, "opus");
    // The branch the first event knew survives an event that says nothing.
    assert_eq!(session.branch, "main");
}

#[test]
fn a_harness_that_sends_no_session_id_is_still_one_session() {
    let grouped = by_project(&[
        event(10, "dotfiles", "", "prompt"),
        event(20, "dotfiles", "", "done"),
    ]);
    assert_eq!(grouped[0].sessions.len(), 1);
    assert_eq!(grouped[0].sessions[0].events.len(), 2);
}
