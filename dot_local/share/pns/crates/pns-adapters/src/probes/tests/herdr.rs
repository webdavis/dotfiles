use super::*;

#[test]
fn a_zoom_onto_a_sibling_hides_the_origin_that_pane_current_would_call_focused() {
    // THE D4 LIVE FAILURE. `herdr pane current` is CALLER-RELATIVE: the
    // hook runs inside the origin pane, so it is told the origin pane is
    // the current one, the origin therefore equals the focused pane, and
    // every desk event self-suppresses. The session was zoomed onto
    // wW:p3R the whole time.
    let view = viewer(answers(WORKSPACE_LIST, "wW:p3K", LAYOUT_ZOOMED_ON_SIBLING))
        .session_view("wW:p3K")
        .expect("a readable view");
    assert_eq!(view.focused_pane, "wW:p3R");
    assert_eq!(visibility("wW:p3K", &view), Visibility::Hidden);
}

#[test]
fn the_view_asks_the_session_what_is_focused_and_never_asks_for_the_current_pane() {
    // The two calls that carry no caller context: the focused workspace's
    // active tab, and the ORIGIN tab's own layout, addressed by the pane
    // id the event itself carried.
    let probes = viewer(answers(WORKSPACE_LIST, "wW:p3K", LAYOUT_ZOOMED_ON_SIBLING));
    probes.session_view("wW:p3K").expect("a readable view");
    assert_eq!(
        probes.runner.calls.lock().unwrap().as_slice(),
        &[
            "herdr workspace list".to_string(),
            "herdr pane layout --pane wW:p3K".to_string(),
        ]
    );
}

#[test]
fn the_zoomed_pane_itself_stays_visible() {
    let view = viewer(answers(WORKSPACE_LIST, "wW:p3K", LAYOUT_ZOOMED_ON_ORIGIN))
        .session_view("wW:p3K")
        .expect("a readable view");
    assert_eq!(visibility("wW:p3K", &view), Visibility::Visible);
}

#[test]
fn an_unzoomed_sibling_is_visible_beside_the_focused_pane() {
    let view = viewer(answers(WORKSPACE_LIST, "wW:p3R", LAYOUT_UNZOOMED))
        .session_view("wW:p3R")
        .expect("a readable view");
    assert_eq!(visibility("wW:p3R", &view), Visibility::Visible);
}

#[test]
fn a_pane_on_another_tab_is_hidden_however_that_tab_is_arranged() {
    let view = viewer(answers(WORKSPACE_LIST, "wW:p10", LAYOUT_OTHER_TAB))
        .session_view("wW:p10")
        .expect("a readable view");
    assert_eq!(view.origin_tab, "wW:tF");
    assert_eq!(visibility("wW:p10", &view), Visibility::Hidden);
}

#[test]
fn the_focused_workspace_decides_the_tab_not_the_first_one_listed() {
    // wV is listed first and its active tab holds the origin, but the
    // operator is looking at wW. Reading the first workspace instead of
    // the focused one would call this Visible.
    let view = viewer(answers(
        WORKSPACE_LIST_SECOND_FOCUSED,
        "wV:p1",
        LAYOUT_OTHER_WORKSPACE,
    ))
    .session_view("wV:p1")
    .expect("a readable view");
    assert_eq!(view.focused_tab, "wW:t9");
    assert_eq!(visibility("wV:p1", &view), Visibility::Hidden);
}

#[test]
fn a_session_with_no_focused_workspace_is_unreadable_rather_than_a_guess() {
    assert!(
        viewer(answers(
            WORKSPACE_LIST_NONE_FOCUSED,
            "wW:p3K",
            LAYOUT_UNZOOMED
        ))
        .session_view("wW:p3K")
        .is_none()
    );
}

#[test]
fn any_herdr_call_failing_leaves_the_view_unreadable_rather_than_guessing() {
    // Unknown never suppresses, so a multiplexer that cannot answer costs
    // a spare notification rather than a lost one.
    for dropped in ["herdr workspace list", "herdr pane layout --pane wW:p3K"] {
        let scripted = answers(WORKSPACE_LIST, "wW:p3K", LAYOUT_UNZOOMED)
            .into_iter()
            .filter(|(call, _)| call != dropped)
            .collect();
        assert!(
            viewer(scripted).session_view("wW:p3K").is_none(),
            "case: {dropped} unanswered"
        );
    }
}

#[test]
fn an_answer_this_parser_does_not_recognise_is_unreadable_too() {
    assert_eq!(parse_focused_tab("not json"), None);
    assert_eq!(parse_focused_tab(r#"{"result":{"workspaces":[]}}"#), None);
    // A focused workspace with no active tab names no tab, and inventing
    // one would suppress against a tab that is not on screen.
    assert_eq!(
        parse_focused_tab(r#"{"result":{"workspaces":[{"focused":true}]}}"#),
        None
    );
    assert!(parse_layout("not json").is_none());
    // A layout missing the zoom flag or either id is a shape we do not
    // know: refusing beats assuming a tab is unzoomed and suppressing.
    assert!(
        parse_layout(r#"{"result":{"layout":{"focused_pane_id":"wW:p3K","tab_id":"wW:t9"}}}"#)
            .is_none()
    );
    assert!(parse_layout(r#"{"result":{"layout":{"zoomed":false}}}"#).is_none());
}
