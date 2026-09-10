use super::*;

/// A real entry off this machine's ring, trimmed of its epoch.
const DESK_VISIBLE: &str = "shell/done mode=none agent=none tool=none surface=Desk \
     visibility=Visible session_visibility=Visible desk_age=0 phone_age=11760 tap_age=33085 \
     locked=no fresh_window=120 long_running=no nag=no local_only=no remote_only=no pane=present \
     pane_dropped=no watch_card=no muted=no focus=no skip_phone=no force_phone=no idle_invalid=no \
     desk_invalid=no phone_invalid=no plan=banner:no,card:no,pulse:no legs=hermes:delivered";

const DESK_HIDDEN: &str = "claude/config-change mode=none agent=none tool=none surface=Desk \
     visibility=Hidden session_visibility=Hidden desk_age=2 phone_age=11713 tap_age=33038 \
     locked=no fresh_window=120 long_running=no nag=no local_only=no remote_only=no pane=present \
     pane_dropped=no watch_card=no muted=no focus=no skip_phone=no force_phone=no idle_invalid=no \
     desk_invalid=no phone_invalid=no plan=banner:yes,card:no,pulse:no \
     legs=macos-banner:delivered,hermes:delivered";

#[test]
fn the_event_is_carried_through_as_the_ring_wrote_it() {
    assert_eq!(summarize(DESK_VISIBLE).event, "shell/done");
    assert_eq!(summarize(DESK_HIDDEN).event, "claude/config-change");
}

#[test]
fn delivered_legs_are_named_as_what_actually_happened() {
    assert_eq!(summarize(DESK_VISIBLE).outcome, "reached hermes");
    assert_eq!(
        summarize(DESK_HIDDEN).outcome,
        "reached macos-banner and hermes"
    );
}

#[test]
fn a_desk_reading_says_whether_the_pane_was_in_view() {
    // The two differ by one field and by whether a banner fired, so a reader
    // scanning the log needs them to read differently too.
    assert_eq!(
        summarize(DESK_VISIBLE).because.as_deref(),
        Some("you were at the desk with the pane in view")
    );
    assert_eq!(
        summarize(DESK_HIDDEN).because.as_deref(),
        Some("you were at the desk, but the pane was hidden")
    );
}

#[test]
fn a_decision_that_sent_nothing_says_so_rather_than_going_quiet() {
    // The commonest question this log is opened to answer.
    let summary = summarize("shell/done surface=Away plan=banner:no,card:no,pulse:no legs=-");
    assert_eq!(summary.outcome, "nothing sent");
    assert_eq!(
        summary.because.as_deref(),
        Some("you were away from the Mac")
    );
}

#[test]
fn a_plan_with_no_legs_is_reported_as_planned_rather_than_as_reached() {
    // A plan is an intention. Calling it a delivery would report a card that
    // never landed as one that did.
    let summary = summarize("claude/blocked surface=Away plan=banner:yes,card:yes,pulse:no");
    assert_eq!(summary.outcome, "planned a banner and a phone card");
}

#[test]
fn an_operator_mute_outranks_every_other_reason() {
    // muted and focus and Away are all true here; only the switch the operator
    // themselves flipped is worth naming, because it is the one they can undo.
    let summary = summarize("shell/done surface=Away muted=yes focus=yes legs=-");
    assert_eq!(summary.because.as_deref(), Some("`pns quiet` was running"));
}

#[test]
fn focus_outranks_where_you_were() {
    let summary = summarize("shell/done surface=Away focus=yes legs=-");
    assert_eq!(
        summary.because.as_deref(),
        Some("a Focus mode pns respects was on")
    );
}

#[test]
fn a_scoped_event_names_the_scope_rather_than_the_surface() {
    assert_eq!(
        summarize("shell/done surface=Desk local_only=yes")
            .because
            .as_deref(),
        Some("the event asked for this Mac only")
    );
    assert_eq!(
        summarize("shell/done surface=Desk remote_only=yes")
            .because
            .as_deref(),
        Some("the event asked for the phone only")
    );
}

#[test]
fn a_nag_says_it_is_a_repeat() {
    assert_eq!(
        summarize("claude/asked surface=Away nag=yes")
            .because
            .as_deref(),
        Some("it was a repeat of an approval nobody answered")
    );
}

#[test]
fn an_entry_naming_no_surface_states_no_reason_rather_than_inventing_one() {
    // A future writer that drops a field must make this say LESS, never
    // something untrue.
    assert!(
        summarize("shell/done plan=banner:no,card:no,pulse:no")
            .because
            .is_none()
    );
}

#[test]
fn an_unrecognized_yes_no_value_is_read_as_absent_rather_than_as_false() {
    // `muted=maybe` is not evidence that nothing was muted, so it must not
    // silently take the un-muted branch and name a different reason.
    let summary = summarize("shell/done surface=Desk visibility=Visible muted=maybe");
    assert_eq!(
        summary.because.as_deref(),
        Some("you were at the desk with the pane in view")
    );
}

#[test]
fn a_body_with_no_fields_at_all_still_names_its_event() {
    let summary = summarize("shell/done");
    assert_eq!(summary.event, "shell/done");
    assert_eq!(summary.outcome, "nothing sent");
    assert!(summary.because.is_none());
}

#[test]
fn three_delivered_legs_read_as_a_sentence_rather_than_a_csv() {
    assert_eq!(
        summarize("x/y legs=a:delivered,b:delivered,c:delivered").outcome,
        "reached a, b and c"
    );
}

#[test]
fn a_desk_entry_with_no_visibility_field_claims_nothing_about_the_pane() {
    // Reading an absent field as "not Visible" would print "but the pane was
    // hidden" over an entry that never said so, which is the report inventing
    // the very thing the reader came to check.
    assert_eq!(
        summarize("shell/done surface=Desk").because.as_deref(),
        Some("you were at the desk")
    );
}
