use super::*;

#[test]
fn observation_is_silent_while_progress_and_legacy_notifications_keep_sound() {
    for state in [
        "observation",
        "progress",
        "model-switch",
        "quota",
        "config-change",
        "blocked",
        "done",
    ] {
        let banner = channel("com.term", None);
        let event = Event {
            state: state.into(),
            ..event_with_pane("wW:p1")
        };
        assert!(matches!(
            banner.deliver(&delivery_request(&event, ReportMode::Silent)),
            pns_domain::Delivery::Delivered(_)
        ));
        let calls = banner.runner.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].contains("-message \\a preview"));
        assert_eq!(
            calls[0].contains("-sound default"),
            state != "observation",
            "{state}: {}",
            calls[0]
        );
    }
}
