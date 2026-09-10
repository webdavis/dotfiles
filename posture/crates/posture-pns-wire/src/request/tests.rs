use super::{Context, DeliveryScope, Interaction, Request, Session, Signal, decode};
use crate::envelope::Rejection;
use crate::identifiers::{Name, RequestId};
use serde_json::{Value, json};

mod classes;

const GOLDEN: &str = include_str!("../../fixtures/request-v1.json");

fn id(text: &str) -> RequestId {
    RequestId::new(text).unwrap()
}

fn name(text: &str) -> Name {
    Name::new(text).unwrap()
}

fn golden_request() -> Request {
    let mut request = Request::new(
        id("nvim-7f3a9c2e-0001"),
        name("nvim"),
        name("BufWritePost"),
        Signal::Succeeded,
    );
    request.session = Some(Session {
        id: name("s-2026-09-06-a"),
        turn: Some(3),
    });
    request.occurred_at = Some(1_788_782_400);
    request.elapsed_secs = Some(42);
    request.detail = "wrote 3 files".to_string();
    request.context = Context {
        project: Some("dotfiles".to_string()),
        branch: Some("main".to_string()),
        pane: Some("wW:p21".to_string()),
    };
    request.route = Some(name("alert"));
    let Value::Object(extensions) = json!({ "nvim": { "buffer": 12 } }) else {
        unreachable!("the literal above is an object");
    };
    request.extensions = extensions;
    request
}

/// The smallest request that decodes: every optional field absent.
fn minimal() -> Value {
    json!({
        "schema": "pns.request/1",
        "request_id": "r-1",
        "producer": "shell",
        "event": "command-finished",
        "signal": { "kind": "failed" }
    })
}

fn decode_value(value: &Value) -> Result<super::Decoded, super::Rejected> {
    decode(value.to_string().as_bytes())
}

#[test]
fn the_golden_version_one_request_decodes_to_exactly_the_struct_it_spells() {
    let decoded = decode(GOLDEN.as_bytes()).unwrap();
    assert_eq!(decoded.request, golden_request());
    assert!(decoded.ignored.is_empty(), "{:?}", decoded.ignored);
}

#[test]
fn a_request_round_trips_through_encode_and_decode_unchanged() {
    let request = golden_request();
    let decoded = decode(request.encode().unwrap().as_bytes()).unwrap();
    assert_eq!(decoded.request, request);
    // Also pins the known-field list: a field the struct writes but the list
    // omits would surface here as an "ignored" field of our own making.
    assert!(decoded.ignored.is_empty(), "{:?}", decoded.ignored);
}

#[test]
fn encode_writes_the_version_one_request_schema() {
    let wire: Value = serde_json::from_str(&golden_request().encode().unwrap()).unwrap();
    assert_eq!(wire["schema"], json!("pns.request/1"));
}

#[test]
fn every_signal_kind_is_tagged_and_no_other_word_is_a_signal() {
    let kinds = [
        ("succeeded", Signal::Succeeded),
        ("failed", Signal::Failed),
        ("needs_attention", Signal::NeedsAttention),
        ("approval_requested", Signal::ApprovalRequested),
        ("resolved", Signal::Resolved),
        ("observation", Signal::Observation),
        ("progress", Signal::Progress),
    ];
    for (word, signal) in kinds {
        let mut value = minimal();
        value["signal"] = json!({ "kind": word });
        let request = decode_value(&value).unwrap().request;
        assert_eq!(request.signal, signal, "{word}");
        let encoded: Value = serde_json::from_str(&request.encode().unwrap()).unwrap();
        assert_eq!(encoded["signal"], json!({ "kind": word }));
    }
    let mut value = minimal();
    value["signal"] = json!({ "kind": "done" });
    let rejected = decode_value(&value).unwrap_err();
    assert!(
        matches!(rejected.reason, Rejection::Invalid(_)),
        "{:?}",
        rejected.reason
    );
    assert_eq!(rejected.reason.code(), "field_invalid");
    assert_eq!(rejected.request_id, Some(id("r-1")));
}

#[test]
fn a_request_missing_its_signal_is_invalid_not_defaulted() {
    let mut value = minimal();
    value.as_object_mut().unwrap().remove("signal");
    let rejected = decode_value(&value).unwrap_err();
    match rejected.reason {
        Rejection::Invalid(message) => assert!(message.contains("signal"), "{message}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn absent_optional_fields_take_their_documented_defaults() {
    let request = decode_value(&minimal()).unwrap().request;
    assert_eq!(request.scope, DeliveryScope::Automatic);
    assert_eq!(request.interaction, Interaction::None);
    assert_eq!(request.detail, "");
    assert_eq!(request.context, Context::default());
    assert!(request.extensions.is_empty());
    assert_eq!(request.session, None);
    assert_eq!(request.route, None);
    assert_eq!(request.occurred_at, None);
    assert_eq!(request.elapsed_secs, None);
}

#[test]
fn an_unknown_top_level_field_is_ignored_and_named_rather_than_refused() {
    let mut value = minimal();
    value["detial"] = json!("a typo");
    value["zzz"] = json!(1);
    let decoded = decode_value(&value).unwrap();
    assert_eq!(
        decoded.ignored,
        vec!["detial".to_string(), "zzz".to_string()]
    );
    assert_eq!(decoded.request.detail, "");
}

#[test]
fn an_unknown_field_inside_extensions_is_kept_verbatim() {
    let mut value = minimal();
    value["extensions"] = json!({ "vendor": { "anything": [1, "two", null] } });
    let request = decode_value(&value).unwrap().request;
    assert_eq!(
        request.extensions.get("vendor"),
        Some(&json!({ "anything": [1, "two", null] }))
    );
}

#[test]
fn hostile_text_inside_the_caps_is_carried_verbatim_because_sanitizing_is_the_domains_job() {
    let hostile = "\u{1b}[31mred\u{7}\u{200b} \"quoted\" \\ back";
    let mut value = minimal();
    value["detail"] = json!(hostile);
    let request = decode_value(&value).unwrap().request;
    assert_eq!(request.detail, hostile);
}

#[test]
fn the_delivery_scope_is_one_word_and_the_words_are_exactly_three() {
    let words = [
        ("automatic", DeliveryScope::Automatic),
        ("local_only", DeliveryScope::LocalOnly),
        ("remote_only", DeliveryScope::RemoteOnly),
    ];
    for (word, scope) in words {
        let mut value = minimal();
        value["scope"] = json!(word);
        let request = decode_value(&value).unwrap().request;
        assert_eq!(request.scope, scope, "{word}");
        let encoded: Value = serde_json::from_str(&request.encode().unwrap()).unwrap();
        assert_eq!(encoded["scope"], json!(word));
    }
    let mut value = minimal();
    value["scope"] = json!("both");
    assert!(matches!(
        decode_value(&value).unwrap_err().reason,
        Rejection::Invalid(_)
    ));
}

#[test]
fn the_interaction_requirement_is_tagged_and_the_words_are_exactly_two() {
    let mut value = minimal();
    value["interaction"] = json!({ "kind": "await_decision" });
    let request = decode_value(&value).unwrap().request;
    assert_eq!(request.interaction, Interaction::AwaitDecision);
    let encoded: Value = serde_json::from_str(&request.encode().unwrap()).unwrap();
    assert_eq!(encoded["interaction"], json!({ "kind": "await_decision" }));
    value["interaction"] = json!({ "kind": "none" });
    let request = decode_value(&value).unwrap().request;
    assert_eq!(request.interaction, Interaction::None);
    let encoded: Value = serde_json::from_str(&request.encode().unwrap()).unwrap();
    assert_eq!(encoded["interaction"], json!({ "kind": "none" }));
    value["interaction"] = json!({ "kind": "wait" });
    assert!(matches!(
        decode_value(&value).unwrap_err().reason,
        Rejection::Invalid(_)
    ));
}

#[test]
fn an_invalid_identifier_anywhere_in_the_request_is_refused_as_invalid() {
    let mut value = minimal();
    value["request_id"] = json!("has space");
    let rejected = decode_value(&value).unwrap_err();
    assert!(
        matches!(rejected.reason, Rejection::Invalid(_)),
        "{:?}",
        rejected.reason
    );
    assert_eq!(rejected.request_id, None);
    let mut value = minimal();
    value["producer"] = json!("");
    assert!(matches!(
        decode_value(&value).unwrap_err().reason,
        Rejection::Invalid(_)
    ));
    let mut value = minimal();
    value["route"] = json!("a\nb");
    assert!(matches!(
        decode_value(&value).unwrap_err().reason,
        Rejection::Invalid(_)
    ));
}

#[test]
fn a_negative_or_fractional_time_is_invalid() {
    let mut value = minimal();
    value["occurred_at"] = json!(-1);
    assert!(matches!(
        decode_value(&value).unwrap_err().reason,
        Rejection::Invalid(_)
    ));
    let mut value = minimal();
    value["elapsed_secs"] = json!(1.5);
    assert!(matches!(
        decode_value(&value).unwrap_err().reason,
        Rejection::Invalid(_)
    ));
}
