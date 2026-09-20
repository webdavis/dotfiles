//! The golden documents are the point of these tests. They are the same two
//! files the engine on the other side of the producer API pins, so a change to
//! either reading that moves the bytes fails here rather than in a delivery.

use super::*;
use serde_json::{Value, json};

const REQUEST: &str = include_str!("../../fixtures/request-v1.json");
const RESULT: &str = include_str!("../../fixtures/result-v1.json");

fn value(text: &str) -> Value {
    serde_json::from_str(text).expect("a golden document is JSON")
}

#[test]
fn the_golden_request_survives_a_decode_and_re_encode_field_for_field() {
    let request = Request::decode(REQUEST.as_bytes()).expect("the golden request");
    assert_eq!(value(&request.encode().unwrap()), value(REQUEST));
}

#[test]
fn the_golden_result_survives_a_decode_and_re_encode_field_for_field() {
    let result = decode_result(RESULT.as_bytes()).expect("the golden result");
    assert_eq!(value(&result.encode().unwrap()), value(RESULT));
}

#[test]
fn the_golden_result_decodes_to_the_fields_posture_acts_on() {
    let result = decode_result(RESULT.as_bytes()).unwrap();
    assert_eq!(
        result.request_id.as_ref().map(RequestId::as_str),
        Some("nvim-7f3a9c2e-0001")
    );
    assert_eq!(result.status, Status::Partial);
    assert_eq!(result.diagnostics, vec!["ignored_field:detial".to_string()]);
    assert_eq!(result.destinations.len(), 2);
    assert_eq!(result.destinations[0].outcome, DeliveryOutcome::Delivered);
    assert_eq!(result.ledger_sequence.as_deref(), Some("123"));
    assert_eq!(result.destinations[0].name.as_str(), "macos-banner");
    let failed = &result.destinations[1];
    assert_eq!(failed.outcome, DeliveryOutcome::Failed);
    assert_eq!(failed.route.as_ref().map(Name::as_str), Some("priority"));
    assert_eq!(failed.note.as_deref(), Some("post FAILED HTTP 401"));
    assert_eq!(failed.retry_at, Some(1_758_153_600));
    // A leg on a destination's own default route states neither a route nor
    // a retry time, rather than writing them as null.
    assert_eq!(result.destinations[0].route, None);
    assert_eq!(result.destinations[0].retry_at, None);
}

#[test]
fn an_engine_that_never_learned_a_legs_answer_reads_unknown() {
    let mut unresolved = value(RESULT);
    unresolved["destinations"][1]["outcome"] = json!("unknown");
    let result = decode_result(unresolved.to_string().as_bytes()).expect("an unknown leg");
    assert_eq!(result.destinations[1].outcome, DeliveryOutcome::Unknown);
}

#[test]
fn a_destination_still_naming_itself_with_the_retired_field_is_refused() {
    let mut stale = value(RESULT);
    let leg = stale["destinations"][0].as_object_mut().unwrap();
    let name = leg.remove("name").unwrap();
    leg.insert("destination".to_string(), name);
    assert!(decode_result(stale.to_string().as_bytes()).is_err());
}

#[test]
fn an_unknown_field_in_a_known_major_is_ignored_on_both_envelopes() {
    for text in [REQUEST, RESULT] {
        let mut extended = value(text);
        extended["future"] = json!({ "field": true });
        let extended = extended.to_string();
        let same = if text == REQUEST {
            value(
                &Request::decode(extended.as_bytes())
                    .unwrap()
                    .encode()
                    .unwrap(),
            )
        } else {
            value(
                &decode_result(extended.as_bytes())
                    .unwrap()
                    .encode()
                    .unwrap(),
            )
        };
        assert_eq!(same, value(text));
    }
}

#[test]
fn a_schema_this_build_does_not_speak_is_malformed_on_both_envelopes() {
    for (text, name) in [(REQUEST, "pns.request"), (RESULT, "pns.result")] {
        for spelling in ["/0", "/2", ""] {
            let other = text.replace(&format!("\"{name}/1\""), &format!("\"{name}{spelling}\""));
            let refused = if text == REQUEST {
                Request::decode(other.as_bytes()).is_err()
            } else {
                decode_result(other.as_bytes()).is_err()
            };
            assert!(refused, "{name}{spelling}");
        }
    }
}

#[test]
fn a_result_missing_its_status_is_malformed_rather_than_defaulted() {
    let mut without = value(RESULT);
    without.as_object_mut().unwrap().remove("status");
    assert!(decode_result(without.to_string().as_bytes()).is_err());
}

#[test]
fn every_required_request_field_must_be_present() {
    for field in ["request_id", "producer", "state"] {
        let mut without = value(REQUEST);
        without.as_object_mut().unwrap().remove(field);
        assert!(
            Request::decode(without.to_string().as_bytes()).is_err(),
            "missing {field}"
        );
    }
}

#[test]
fn bytes_over_the_envelope_cap_are_refused_before_they_are_parsed() {
    let bytes = vec![b'?'; MAX_BYTES + 1];
    assert_eq!(Request::decode(&bytes), Err(Malformed));
    assert_eq!(decode_result(&bytes), Err(Malformed));
}

#[test]
fn request_encoding_refuses_detail_over_the_text_cap() {
    let mut request = Request::decode(REQUEST.as_bytes()).unwrap();
    for chars in [MAX_TEXT_CHARS - 1, MAX_TEXT_CHARS] {
        request.detail = "é".repeat(chars);
        assert!(request.encode().is_ok(), "{chars} characters");
    }
    request.detail.push('é');
    assert_eq!(request.encode(), Err(Oversized));
}

#[test]
fn request_encoding_refuses_the_total_bytes_even_when_each_field_fits() {
    let mut request = Request::decode(REQUEST.as_bytes()).unwrap();
    request.detail = "x".repeat(MAX_TEXT_CHARS);
    for index in 0..7 {
        request
            .extensions
            .insert(format!("k{index}"), json!("x".repeat(MAX_TEXT_CHARS)));
    }
    assert!(request.encode().is_ok());
    request
        .extensions
        .insert("over".to_string(), json!("x".repeat(MAX_TEXT_CHARS)));
    assert_eq!(request.encode(), Err(Oversized));
}

#[test]
fn identifiers_that_break_their_own_rules_are_refused_on_the_way_in() {
    let mut hostile = value(REQUEST);
    hostile["request_id"] = json!("has a space");
    assert!(Request::decode(hostile.to_string().as_bytes()).is_err());
    let mut hostile = value(REQUEST);
    hostile["producer"] = json!("");
    assert!(Request::decode(hostile.to_string().as_bytes()).is_err());
}

#[test]
fn an_unknown_or_wrapped_status_or_outcome_word_is_refused_rather_than_guessed() {
    for field in ["status", "outcome", "wrapped_status", "wrapped_outcome"] {
        let mut hostile = value(RESULT);
        match field {
            "status" => hostile["status"] = json!("unknown"),
            "outcome" => hostile["destinations"][0]["outcome"] = json!("unreported"),
            "wrapped_status" => hostile["status"] = json!({ "kind": "partial" }),
            _ => hostile["destinations"][0]["outcome"] = json!({ "kind": "delivered" }),
        }
        assert!(
            decode_result(hostile.to_string().as_bytes()).is_err(),
            "unknown {field}"
        );
    }
}
