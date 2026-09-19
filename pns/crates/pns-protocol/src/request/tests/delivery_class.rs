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
fn the_old_kind_and_class_spellings_are_refused_by_name_rather_than_ignored() {
    // The two fields this one replaced. A field nobody ever defined is
    // ignored, because a newer producer must not break an older pns; one this
    // envelope USED to honour is refused, because ignoring it is a page whose
    // class went nowhere and a producer that never learns.
    for (retired, word) in [("kind", "health"), ("class", "security")] {
        let mut value = minimal();
        value[retired] = json!(word);
        let refused = decode_value(&value).expect_err("a retired field cannot be ignored");
        assert_eq!(refused.request_id, Some(id("r-1")));
        let Rejection::Invalid(said) = refused.reason else {
            panic!("a retired field is invalid input: {refused:?}");
        };
        assert!(said.contains(retired), "{said}");
        assert!(said.contains("delivery_class"), "{said}");
    }
}
