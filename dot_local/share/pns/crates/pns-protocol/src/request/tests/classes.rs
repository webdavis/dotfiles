use super::*;

#[test]
fn a_delivery_class_round_trips_but_absence_keeps_the_original_request_bytes() {
    let original = decode_value(&minimal()).unwrap().request.encode().unwrap();
    assert_eq!(
        original,
        r#"{"schema":"pns.request/1","request_id":"r-1","producer":"shell","session":null,"event":"command-finished","signal":{"kind":"failed"},"occurred_at":null,"elapsed_secs":null,"detail":"","context":{"project":null,"branch":null,"pane":null},"scope":"automatic","route":null,"interaction":{"kind":"none"},"extensions":{}}"#
    );
    for class in ["security".to_string(), "other-class".into(), "x".repeat(64)] {
        let mut value = minimal();
        value["class"] = json!(class);
        let decoded = decode_value(&value).unwrap();
        assert!(
            decoded.ignored.is_empty(),
            "class is owned request metadata"
        );
        let encoded: Value = serde_json::from_str(&decoded.request.encode().unwrap()).unwrap();
        assert_eq!(encoded["class"], value["class"]);
    }
    for class in [
        json!(""),
        json!("x".repeat(65)),
        json!("bad\nclass"),
        json!(3),
        json!([]),
    ] {
        let mut value = minimal();
        value["class"] = class;
        let refused = decode_value(&value).expect_err("invalid class cannot be ignored");
        assert_eq!(refused.request_id, Some(id("r-1")));
    }
    let mut value = minimal();
    value["class"] = Value::Null;
    assert_eq!(
        decode_value(&value).unwrap().request.encode().unwrap(),
        original
    );
}
