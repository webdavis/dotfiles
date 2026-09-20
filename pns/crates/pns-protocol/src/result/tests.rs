use super::{DeliveryOutcome, DestinationOutcome, ResultEnvelope, Status, decode};
use crate::envelope::{Rejected, Rejection};
use crate::identifiers::{Name, RequestId};
use serde_json::{Value, json};

const GOLDEN: &str = include_str!("../../fixtures/result-v1.json");

fn id(text: &str) -> RequestId {
    RequestId::new(text).unwrap()
}

fn name(text: &str) -> Name {
    Name::new(text).unwrap()
}

fn golden_result() -> ResultEnvelope {
    ResultEnvelope {
        request_id: Some(id("nvim-7f3a9c2e-0001")),
        status: Status::Partial,
        ledger_sequence: Some("123".to_string()),
        destinations: vec![
            DestinationOutcome {
                name: name("banner"),
                outcome: DeliveryOutcome::Delivered,
                route: None,
                note: None,
                retry_at: None,
            },
            DestinationOutcome {
                name: name("hermes"),
                outcome: DeliveryOutcome::Failed,
                route: Some(name("priority")),
                note: Some("post FAILED HTTP 401".to_string()),
                retry_at: Some(1_758_153_600),
            },
        ],
        diagnostics: vec!["ignored_field:detial".to_string()],
    }
}

#[test]
fn the_golden_version_one_result_decodes_to_exactly_the_struct_it_spells() {
    assert_eq!(decode(GOLDEN.as_bytes()).unwrap(), golden_result());
}

#[test]
fn a_result_round_trips_through_encode_and_decode_unchanged() {
    let result = golden_result();
    assert_eq!(decode(result.encode().unwrap().as_bytes()).unwrap(), result);
}

#[test]
fn encode_writes_the_version_one_result_schema() {
    let wire: Value = serde_json::from_str(&golden_result().encode().unwrap()).unwrap();
    assert_eq!(wire["schema"], json!("pns.result/1"));
}

#[test]
fn a_rejection_becomes_a_rejected_result_carrying_the_recovered_id_and_the_code() {
    let rejected = Rejected {
        request_id: Some(id("r-1")),
        reason: Rejection::MajorUnsupported(2),
    };
    let result = ResultEnvelope::rejected(&rejected);
    assert_eq!(result.request_id, Some(id("r-1")));
    assert_eq!(result.status, Status::Rejected);
    assert_eq!(result.diagnostics, vec!["major_unsupported".to_string()]);
    assert!(result.destinations.is_empty());
    assert_eq!(result.ledger_sequence, None);
    // Encodable even with no id, which is the malformed-bytes case.
    let anonymous = ResultEnvelope::rejected(&Rejected {
        request_id: None,
        reason: Rejection::Malformed("x".to_string()),
    });
    let wire: Value = serde_json::from_str(&anonymous.encode().unwrap()).unwrap();
    assert_eq!(wire["request_id"], Value::Null);
    assert_eq!(wire["diagnostics"], json!(["malformed_json"]));
}

#[test]
fn diagnostics_are_bounded_at_the_item_cap_when_encoded() {
    let mut result = golden_result();
    // Advisory codes keep their first 64 entries. Delivery facts stay intact.
    for count in [63, 64, 65] {
        result.diagnostics = (0..count).map(|index| format!("code_{index}")).collect();
        let decoded = decode(result.encode().unwrap().as_bytes()).unwrap();
        let mut expected = result.clone();
        expected.diagnostics = (0..count.min(64))
            .map(|index| format!("code_{index}"))
            .collect();
        assert_eq!(decoded, expected, "{count} diagnostics");
        assert_eq!(
            result.diagnostics.len(),
            count,
            "encoding must not mutate the caller"
        );
    }
}

#[test]
fn every_status_and_outcome_word_is_pinned_as_a_bare_string() {
    let mut result = golden_result();
    for (status, word) in [
        (Status::Delivered, "delivered"),
        (Status::Partial, "partial"),
        (Status::Undelivered, "undelivered"),
        (Status::Rejected, "rejected"),
    ] {
        result.status = status;
        let wire: Value = serde_json::from_str(&result.encode().unwrap()).unwrap();
        assert_eq!(wire["status"], json!(word));
        assert_eq!(decode(wire.to_string().as_bytes()).unwrap().status, status);
    }
    for (outcome, word) in [
        (DeliveryOutcome::Delivered, "delivered"),
        (DeliveryOutcome::Failed, "failed"),
        (DeliveryOutcome::Silent, "silent"),
        (DeliveryOutcome::Unlaunched, "unlaunched"),
        (DeliveryOutcome::Unknown, "unknown"),
    ] {
        result.destinations[0].outcome = outcome;
        let wire: Value = serde_json::from_str(&result.encode().unwrap()).unwrap();
        assert_eq!(wire["destinations"][0]["outcome"], json!(word));
        assert_eq!(
            decode(wire.to_string().as_bytes()).unwrap().destinations[0].outcome,
            outcome
        );
    }
}

#[test]
fn the_ledger_row_and_each_destination_name_are_the_only_identifying_fields() {
    let wire: Value = serde_json::from_str(&golden_result().encode().unwrap()).unwrap();
    assert_eq!(wire["ledger_sequence"], json!("123"));
    assert_eq!(wire["destinations"][0]["name"], json!("banner"));
    assert_eq!(wire.get("decision_id"), None);
    assert_eq!(wire.get("interaction"), None);
    assert_eq!(wire["destinations"][0].get("destination"), None);
}

#[test]
fn an_absent_note_route_or_retry_time_is_omitted_rather_than_written_as_null() {
    let wire: Value = serde_json::from_str(&golden_result().encode().unwrap()).unwrap();
    for field in ["note", "route", "retry_at"] {
        assert_eq!(wire["destinations"][0].get(field), None, "{field}");
    }
    assert_eq!(
        wire["destinations"][1]["note"],
        json!("post FAILED HTTP 401")
    );
    assert_eq!(wire["destinations"][1]["route"], json!("priority"));
    assert_eq!(wire["destinations"][1]["retry_at"], json!(1_758_153_600u64));
}

#[test]
fn a_result_with_the_wrong_schema_is_refused_like_a_request() {
    let request_text = include_str!("../../fixtures/request-v1.json");
    let rejected = decode(request_text.as_bytes()).unwrap_err();
    assert_eq!(
        rejected.reason,
        Rejection::SchemaUnknown("pns.request/1".to_string())
    );
    let rejected = decode(br#"{"schema":"pns.result/2","request_id":"r-1"}"#).unwrap_err();
    assert_eq!(rejected.reason, Rejection::MajorUnsupported(2));
    assert_eq!(rejected.request_id, Some(id("r-1")));
}
