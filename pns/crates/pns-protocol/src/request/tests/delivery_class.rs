use super::*;

#[test]
fn a_delivery_class_round_trips_but_absence_keeps_the_original_request_bytes() {
    let original = decode_value(&minimal()).unwrap().request.encode().unwrap();
    assert_eq!(
        original,
        r#"{"schema":"pns.request/1","request_id":"r-1","producer":"shell","session":null,"state":"failed","elapsed":null,"detail":"","project":null,"branch":null,"pane":null,"scope":"automatic","route":null,"extensions":{}}"#
    );
    for class in ["security".to_string(), "other-class".into(), "x".repeat(64)] {
        let mut value = minimal();
        value["delivery_class"] = json!(class);
        let decoded = decode_value(&value).unwrap();
        assert!(
            decoded.ignored.is_empty(),
            "a delivery class is owned request metadata"
        );
        let encoded: Value = serde_json::from_str(&decoded.request.encode().unwrap()).unwrap();
        assert_eq!(encoded["delivery_class"], value["delivery_class"]);
    }
    for class in [
        json!(""),
        json!("x".repeat(65)),
        json!("bad\nclass"),
        json!(3),
        json!([]),
    ] {
        let mut value = minimal();
        value["delivery_class"] = class;
        let refused =
            decode_value(&value).expect_err("an invalid delivery class cannot be ignored");
        assert_eq!(refused.request_id, Some(id("r-1")));
    }
    let mut value = minimal();
    value["delivery_class"] = Value::Null;
    assert_eq!(
        decode_value(&value).unwrap().request.encode().unwrap(),
        original
    );
}

#[test]
fn the_old_kind_and_class_spellings_are_ignored_and_named_rather_than_refused() {
    // The two fields this one replaced. They decode as ordinary unknown
    // fields now, the same as a typo, so an older producer is told its word
    // went nowhere instead of having its page dropped.
    let mut value = minimal();
    value["kind"] = json!("health");
    value["class"] = json!("security");
    let decoded = decode_value(&value).unwrap();
    assert_eq!(
        decoded.ignored,
        vec!["class".to_string(), "kind".to_string()]
    );
    assert_eq!(decoded.request.delivery_class, None);
}
