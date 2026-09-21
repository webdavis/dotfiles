use super::{DeliveryScope, Remind, RequestEnvelope, State, decode};
use crate::envelope::Rejection;
use crate::identifiers::{Name, RequestId};
use serde_json::{Value, json};
use std::time::Duration;

mod delivery_class;

const GOLDEN: &str = include_str!("../../fixtures/request-v1.json");

fn id(text: &str) -> RequestId {
    RequestId::new(text).unwrap()
}

fn name(text: &str) -> Name {
    Name::new(text).unwrap()
}

fn golden_request() -> RequestEnvelope {
    let mut request = RequestEnvelope::new(id("nvim-7f3a9c2e-0001"), name("nvim"), State::Done);
    request.session = Some(name("s-2026-09-06-a"));
    request.elapsed = Some(Duration::from_secs(42));
    request.detail = "wrote 3 files".to_string();
    request.project = Some("dotfiles".to_string());
    request.branch = Some("main".to_string());
    request.pane = Some("wW:p21".to_string());
    request.route = Some(name("alert"));
    request.delivery_class = Some(name("health"));
    request.remind = Some(Remind::After(Duration::from_secs(300)));
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
        "state": "failed"
    })
}

fn decode_value(value: &Value) -> Result<super::DecodedRequest, super::Rejected> {
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
    // omits would refuse its own encoded request here.
    assert!(decoded.ignored.is_empty(), "{:?}", decoded.ignored);
}

#[test]
fn encode_writes_the_version_one_request_schema() {
    let wire: Value = serde_json::from_str(&golden_request().encode().unwrap()).unwrap();
    assert_eq!(wire["schema"], json!("pns.request/1"));
}

#[test]
fn the_state_is_one_plain_word_and_the_words_are_exactly_six() {
    let words = [
        ("done", State::Done),
        ("failed", State::Failed),
        ("blocked", State::Blocked),
        ("resolved", State::Resolved),
        ("observation", State::Observation),
        ("progress", State::Progress),
    ];
    assert_eq!(words.len(), State::WORDS.len());
    for (word, state) in words {
        assert_eq!(State::from_word(word), Some(state));
        assert_eq!(state.as_str(), word);
        let mut value = minimal();
        value["state"] = json!(word);
        let request = decode_value(&value).unwrap().request;
        assert_eq!(request.state, state, "{word}");
        let encoded: Value = serde_json::from_str(&request.encode().unwrap()).unwrap();
        assert_eq!(encoded["state"], json!(word));
    }
    // The wrapper's own words are gone with it, and so is the wrapper.
    for retired in ["succeeded", "needs_attention", "approval_requested"] {
        assert_eq!(State::from_word(retired), None, "{retired}");
        let mut value = minimal();
        value["state"] = json!(retired);
        let rejected = decode_value(&value).unwrap_err();
        assert!(
            matches!(rejected.reason, Rejection::Invalid(_)),
            "{:?}",
            rejected.reason
        );
        assert_eq!(rejected.reason.code(), "field_invalid");
        assert_eq!(rejected.request_id, Some(id("r-1")));
    }
}

#[test]
fn a_request_missing_its_state_is_invalid_not_defaulted() {
    let mut value = minimal();
    value.as_object_mut().unwrap().remove("state");
    let rejected = decode_value(&value).unwrap_err();
    match rejected.reason {
        Rejection::Invalid(message) => assert!(message.contains("state"), "{message}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn observation_and_progress_are_the_quiet_states_and_nothing_else_is() {
    for state in [State::Observation, State::Progress] {
        assert!(state.quiet(), "{}", state.as_str());
    }
    for state in [State::Done, State::Failed, State::Blocked, State::Resolved] {
        assert!(!state.quiet(), "{}", state.as_str());
    }
}

#[test]
fn absent_optional_fields_take_their_documented_defaults() {
    let request = decode_value(&minimal()).unwrap().request;
    assert_eq!(request.scope, DeliveryScope::Automatic);
    assert_eq!(request.detail, "");
    assert_eq!(request.project, None);
    assert_eq!(request.branch, None);
    assert_eq!(request.pane, None);
    assert!(request.extensions.is_empty());
    assert_eq!(request.session, None);
    assert_eq!(request.route, None);
    assert_eq!(request.elapsed, None);
}

/// A field pns would drop is a producer saying something that goes nowhere,
/// so it is refused and named, exactly as the flag path refuses an unknown
/// flag. The refusal names the FIRST unknown field rather than listing them.
#[test]
fn an_unknown_top_level_field_is_refused_and_named() {
    let mut value = minimal();
    value["detial"] = json!("a typo");
    value["zzz"] = json!(1);
    let refusal = decode_value(&value).expect_err("an unknown field is refused");
    assert_eq!(
        refusal.reason,
        Rejection::Invalid("`detial` is not a field pns takes".to_string())
    );
    assert_eq!(
        refusal.request_id.map(|id| id.as_str().to_string()),
        Some("r-1".to_string()),
        "the refusal stays correlated"
    );
}

/// A field carrying no value at all is still bad input: `state` is required,
/// and a request without it is refused before anything is delivered.
#[test]
fn a_required_field_left_out_is_refused() {
    let mut value = minimal();
    value.as_object_mut().unwrap().remove("state");
    assert!(matches!(
        decode_value(&value).unwrap_err().reason,
        Rejection::Invalid(_)
    ));
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
fn event_occurred_at_and_interaction_are_refused_like_any_other_unknown_field() {
    // These fields changed nothing: `event` and `occurred_at` were stored and
    // never read, and `interaction` always answered "no opinion". Each is an
    // ordinary unknown field now, the same as a typo.
    for field in ["event", "occurred_at", "interaction"] {
        let mut value = minimal();
        value[field] = json!("whatever it said");
        assert_eq!(
            decode_value(&value)
                .expect_err("a retired name is refused")
                .reason,
            Rejection::Invalid(format!("`{field}` is not a field pns takes")),
        );
    }
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
fn elapsed_is_a_duration_with_a_unit_and_a_bare_number_is_refused() {
    for (text, duration) in [
        ("0s", Duration::ZERO),
        ("90s", Duration::from_secs(90)),
        ("5m", Duration::from_secs(300)),
        ("2h", Duration::from_secs(7_200)),
    ] {
        let mut value = minimal();
        value["elapsed"] = json!(text);
        let request = decode_value(&value).unwrap().request;
        assert_eq!(request.elapsed, Some(duration), "{text}");
    }
    // A BARE NUMBER IS THE REFUSAL THIS FIELD EXISTS FOR: one reader takes
    // `90` as seconds and the next as minutes.
    for word in [json!("90"), json!(90), json!(1.5), json!("90 s"), json!("")] {
        let mut value = minimal();
        value["elapsed"] = word.clone();
        let refused = decode_value(&value).expect_err("a bare number cannot be a duration");
        assert_eq!(refused.reason.code(), "field_invalid", "{word}");
    }
    // Past the range rather than clamped into it.
    let mut value = minimal();
    value["elapsed"] = json!("721h");
    assert_eq!(
        decode_value(&value).unwrap_err().reason.code(),
        "field_invalid"
    );
}

#[test]
fn a_request_naming_no_delivery_class_keeps_the_original_version_one_bytes() {
    // THE FIXTURE BYTES ARE A CONTRACT posture's own copy of this envelope
    // pins too, so an additive field must not move them.
    let original = decode_value(&minimal()).unwrap().request.encode().unwrap();
    assert_eq!(
        original,
        r#"{"schema":"pns.request/1","request_id":"r-1","producer":"shell","state":"failed","detail":"","scope":"automatic","extensions":{}}"#,
        "an absent delivery class moved the canonical bytes"
    );
    assert_eq!(
        decode_value(&minimal()).unwrap().request.delivery_class,
        None
    );
    let mut value = minimal();
    value["delivery_class"] = Value::Null;
    assert_eq!(
        decode_value(&value).unwrap().request.encode().unwrap(),
        original,
        "a null delivery class is an absent delivery class"
    );
}

#[test]
fn an_absent_optional_request_field_is_omitted_rather_than_written_as_null() {
    let request = decode_value(&minimal()).unwrap().request;
    assert_eq!(request.session, None);
    assert_eq!(request.elapsed, None);
    let text = request.encode().unwrap();
    let wire: Value = serde_json::from_str(&text).unwrap();
    for field in [
        "session",
        "elapsed",
        "project",
        "branch",
        "pane",
        "route",
        "delivery_class",
        "remind",
    ] {
        assert_eq!(wire.get(field), None, "{field}");
    }
    assert!(!text.contains("null"), "{text}");
    // A field written as null decodes as absent and comes back omitted.
    let mut nulled = minimal();
    for field in ["session", "elapsed", "project", "branch", "pane", "route"] {
        nulled[field] = Value::Null;
    }
    assert_eq!(
        decode_value(&nulled).unwrap().request.encode().unwrap(),
        text
    );
}

#[test]
fn remind_is_a_boolean_or_a_duration_and_anything_else_names_the_field() {
    for (value, remind) in [
        (json!(true), Remind::Configured),
        (json!(false), Remind::Off),
        (json!("30s"), Remind::After(Duration::from_secs(30))),
        (json!("5m"), Remind::After(Duration::from_secs(300))),
        (json!("1h"), Remind::After(Duration::from_secs(3_600))),
    ] {
        let mut request = minimal();
        request["remind"] = value.clone();
        assert_eq!(
            decode_value(&request).unwrap().request.remind,
            Some(remind),
            "{value}"
        );
    }
    // A DURATION OUTSIDE THE ONE RANGE EVERY SPELLING SHARES, a bare number
    // and a shape that is neither word are each refused, by name.
    for value in [
        json!("29s"),
        json!("2h"),
        json!("300"),
        json!(300),
        json!([]),
    ] {
        let mut request = minimal();
        request["remind"] = value.clone();
        let refused = decode_value(&request).expect_err("a reminder is a boolean or a duration");
        let Rejection::Invalid(sentence) = &refused.reason else {
            panic!("{value} was refused as {:?}", refused.reason);
        };
        assert!(sentence.contains("remind"), "{value}: {sentence}");
    }
}

#[test]
fn a_request_saying_nothing_about_the_reminder_keeps_the_original_version_one_bytes() {
    let original = decode_value(&minimal()).unwrap().request.encode().unwrap();
    assert!(!original.contains("remind"), "{original}");
    assert_eq!(decode_value(&minimal()).unwrap().request.remind, None);
}
