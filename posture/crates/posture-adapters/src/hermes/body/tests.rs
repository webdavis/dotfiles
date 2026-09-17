use super::*;

fn alert() -> Alert {
    Alert {
        occurrence_id: Some("occurrence-7".into()),
        event: "page",
        signal: AlertSignal::NeedsAttention,
        severity: Some(Severity::Critical),
        occurred_at: Some(1730000000),
        title: "Security finding".into(),
        detail: "line one\nline two".into(),
    }
}

#[test]
fn the_body_serves_the_flat_placeholders_and_the_nested_alert_object() {
    let body = encode(&alert(), &Name::new("priority").unwrap());
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["agent"], "posture");
    assert_eq!(parsed["state"], "critical");
    assert_eq!(parsed["project"], "page");
    assert_eq!(parsed["detail"], "Security finding\nline one\nline two");
    assert_eq!(parsed["route"], "priority");
    assert_eq!(parsed["alert"]["title"], "Security finding");
    assert_eq!(parsed["alert"]["detail"], "line one\nline two");
}

#[test]
fn the_body_serves_the_header_subheader_and_body_placeholders_too() {
    let parsed: serde_json::Value =
        serde_json::from_str(&encode(&alert(), &Name::new("priority").unwrap())).unwrap();
    assert_eq!(parsed["header"], "Security finding");
    assert_eq!(parsed["subheader"], "posture \u{b7} critical");
    assert_eq!(parsed["body"], "line one\nline two");
}

#[test]
fn no_placeholder_any_gateway_route_names_is_left_out_of_either_body() {
    let route = Name::new("posture-pages").unwrap();
    let storm = encode_storm(&["first finding".into(), "second finding".into()], &route);
    for body in [encode(&alert(), &route), storm] {
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        for key in [
            "agent",
            "state",
            "project",
            "detail",
            "header",
            "subheader",
            "body",
        ] {
            assert!(parsed[key].is_string(), "{key} is missing from {body}");
        }
        assert!(parsed["alert"]["title"].is_string());
        assert!(parsed["alert"]["detail"].is_string());
    }
}

#[test]
fn the_combined_body_states_how_many_findings_the_hour_holds_and_lists_them() {
    let parsed: serde_json::Value = serde_json::from_str(&encode_storm(
        &["first finding".into(), "second finding".into()],
        &Name::new("explain").unwrap(),
    ))
    .unwrap();
    assert_eq!(parsed["header"], "2 distinct critical findings in one hour");
    assert_eq!(parsed["subheader"], "posture \u{b7} storm");
    assert_eq!(parsed["body"], "- first finding\n- second finding");
    assert_eq!(parsed["state"], "critical");
    assert_eq!(parsed["route"], "explain");
}

#[test]
fn an_untiered_page_states_what_its_submission_is_rather_than_a_tier() {
    for (signal, word) in [
        (AlertSignal::NeedsAttention, "needs-attention"),
        (AlertSignal::Observation, "observation"),
    ] {
        let alert = Alert {
            severity: None,
            signal,
            ..alert()
        };
        let parsed: serde_json::Value =
            serde_json::from_str(&encode(&alert, &Name::new("posture-pages").unwrap())).unwrap();
        assert_eq!(parsed["state"], word);
    }
}

#[test]
fn a_detail_holding_json_syntax_is_encoded_rather_than_glued_into_the_body() {
    let alert = Alert {
        detail: "plugin \"a\": {broken}\nnext".into(),
        ..alert()
    };
    let parsed: serde_json::Value =
        serde_json::from_str(&encode(&alert, &Name::new("posture-pages").unwrap())).unwrap();
    assert_eq!(parsed["alert"]["detail"], "plugin \"a\": {broken}\nnext");
}
