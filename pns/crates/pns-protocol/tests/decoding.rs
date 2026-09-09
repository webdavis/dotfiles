use pns_protocol::{Rejection, Violation, decode_request, decode_result};
use serde_json::{Value, json};

const REQUEST: &str = include_str!("../fixtures/request-v1.json");
const RESULT: &str = include_str!("../fixtures/result-v1.json");

#[test]
fn conflicting_schema_fields_are_refused_before_a_version_is_chosen() {
    let duplicate = REQUEST.replacen(
        "\"schema\":",
        "\"schema\": \"pns.request/2\", \"schema\":",
        1,
    );
    assert!(matches!(
        decode_request(duplicate.as_bytes()).unwrap_err().reason,
        Rejection::Malformed(_)
    ));
}

#[test]
fn conflicting_request_ids_are_refused_instead_of_choosing_one() {
    let duplicate = REQUEST.replacen(
        "\"request_id\":",
        "\"request_id\": \"other\", \"request_id\":",
        1,
    );
    let rejected = decode_request(duplicate.as_bytes()).unwrap_err();
    assert!(matches!(rejected.reason, Rejection::Malformed(_)));
    assert_eq!(rejected.request_id, None);
}

#[test]
fn duplicate_fields_inside_extensions_are_refused_too() {
    let duplicate = REQUEST.replace("\"buffer\": 12", "\"buffer\": 12, \"buffer\": 13");
    assert!(matches!(
        decode_request(duplicate.as_bytes()).unwrap_err().reason,
        Rejection::Malformed(_)
    ));
}

#[test]
fn unknown_result_fields_are_ignored_within_the_known_major() {
    let mut value: Value = serde_json::from_str(RESULT).unwrap();
    value["future"] = json!({ "field": true });
    assert_eq!(
        decode_result(value.to_string().as_bytes()).unwrap(),
        decode_result(RESULT.as_bytes()).unwrap()
    );
}

#[test]
fn an_unknown_lower_major_is_refused_for_both_envelopes() {
    for (text, kind) in [(REQUEST, "request"), (RESULT, "result")] {
        let value = text.replace(&format!("pns.{kind}/1"), &format!("pns.{kind}/0"));
        let rejected = if kind == "request" {
            decode_request(value.as_bytes()).unwrap_err()
        } else {
            decode_result(value.as_bytes()).unwrap_err()
        };
        assert_eq!(rejected.reason, Rejection::MajorUnsupported(0));
    }
}

#[test]
fn every_json_value_survives_uninterpreted_extensions() {
    let mut value: Value = serde_json::from_str(REQUEST).unwrap();
    value["extensions"] = json!({
        "empty": null, "boolean": true, "negative": -1, "unsigned": u64::MAX,
        "fraction": 1.25, "text": "é", "array": [false, { "text": "quoted\"" }]
    });
    let request = decode_request(value.to_string().as_bytes())
        .unwrap()
        .request;
    assert_eq!(json!(request.extensions), value["extensions"]);
}

#[test]
fn unknown_fields_cannot_bypass_the_shared_decode_bounds() {
    let mut request: Value = serde_json::from_str(REQUEST).unwrap();
    request["future"] = json!("x".repeat(8_001));
    assert_eq!(
        decode_request(request.to_string().as_bytes())
            .unwrap_err()
            .reason,
        Rejection::Bound(Violation::Text { chars: 8_001 })
    );
    let mut result: Value = serde_json::from_str(RESULT).unwrap();
    result["future"] = json!("x".repeat(8_001));
    assert_eq!(
        decode_result(result.to_string().as_bytes())
            .unwrap_err()
            .reason,
        Rejection::Bound(Violation::Text { chars: 8_001 })
    );
}

#[test]
fn every_required_request_field_must_be_present() {
    for field in ["request_id", "producer", "event", "signal"] {
        let mut value: Value = serde_json::from_str(REQUEST).unwrap();
        value.as_object_mut().unwrap().remove(field);
        assert!(
            matches!(
                decode_request(value.to_string().as_bytes())
                    .unwrap_err()
                    .reason,
                Rejection::Invalid(_)
            ),
            "missing {field}"
        );
    }
}

#[test]
fn a_result_must_state_its_status() {
    let mut value: Value = serde_json::from_str(RESULT).unwrap();
    value.as_object_mut().unwrap().remove("status");
    assert!(matches!(
        decode_result(value.to_string().as_bytes())
            .unwrap_err()
            .reason,
        Rejection::Invalid(_)
    ));
}

#[test]
fn unknown_result_status_outcome_and_interaction_words_are_refused() {
    for field in ["status", "outcome", "interaction"] {
        let mut value: Value = serde_json::from_str(RESULT).unwrap();
        match field {
            "status" => value["status"] = json!("unknown"),
            "outcome" => value["destinations"][0]["outcome"] = json!("unknown"),
            _ => value["interaction"] = json!({ "kind": "unknown" }),
        }
        assert!(
            matches!(
                decode_result(value.to_string().as_bytes())
                    .unwrap_err()
                    .reason,
                Rejection::Invalid(_)
            ),
            "unknown {field}"
        );
    }
}
