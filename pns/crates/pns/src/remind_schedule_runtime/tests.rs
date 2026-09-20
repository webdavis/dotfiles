use super::Remind;
use crate::legacy::remind_switch;

/// The switch a request's JSON `remind` field states, decoded through the
/// protocol the way `pns submit` decodes it.
fn from_json(remind: &str) -> Option<Remind> {
    let request = format!(
        r#"{{"schema":"pns.request/1","request_id":"r-1","producer":"claude",
        "state":"blocked","remind":{remind}}}"#
    );
    pns_protocol::decode_request(request.as_bytes())
        .expect("the request above is version 1")
        .request
        .remind
}

/// The switch a hook's argv states.
fn from_flag(flag: &str) -> Option<Remind> {
    remind_switch(&[flag.to_string()]).expect("the flag above is spelled right")
}

/// THE TWO PATHS HAND ONE RESOLUTION ONE VALUE. `remind_delay` reads config
/// and a clock, so what is pinned here is its INPUT: a request and the
/// equivalent flag resolve alike because they arrive as the same switch.
#[test]
fn remind_true_is_the_configured_delay_the_way_the_bare_flag_is() {
    assert_eq!(from_json("true"), from_flag("--remind"));
    assert_eq!(from_json("true"), Some(Remind::Configured));
}

#[test]
fn a_remind_duration_overrides_the_delay_the_way_the_valued_flag_does() {
    assert_eq!(from_json("\"5m\""), from_flag("--remind=5m"));
    assert_eq!(
        from_json("\"5m\""),
        Some(Remind::After(std::time::Duration::from_secs(300)))
    );
}

#[test]
fn remind_false_disarms_the_way_no_remind_does() {
    assert_eq!(from_json("false"), from_flag("--no-remind"));
    assert_eq!(from_json("false"), Some(Remind::Off));
}

#[test]
fn a_request_that_says_nothing_about_the_reminder_falls_through_like_a_bare_hook() {
    assert_eq!(from_json_absent(), remind_switch(&[]).unwrap());
    assert_eq!(from_json_absent(), None);
}

fn from_json_absent() -> Option<Remind> {
    pns_protocol::decode_request(
        br#"{"schema":"pns.request/1","request_id":"r-1","producer":"claude","state":"blocked"}"#,
    )
    .expect("the request above is version 1")
    .request
    .remind
}
