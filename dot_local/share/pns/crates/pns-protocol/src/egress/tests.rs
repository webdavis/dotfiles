use super::{EgressEnvelope, EgressMode, RenderedEvent, decode};
use crate::{MAX_BYTES, MAX_TEXT_CHARS, Rejection, RequestId, Violation};
use serde_json::{Value, json};

const GOLDEN: &str = include_str!("../../fixtures/egress-v1.json");

fn event() -> EgressEnvelope {
    EgressEnvelope {
        request_id: RequestId::new("shell-42").unwrap(),
        body: RenderedEvent {
            agent: "shell".into(),
            branch: "work".into(),
            detail: "a \"quote\"\n\t雪\\".into(),
            message: "work: finished".into(),
            mode: EgressMode::Silent,
            pane: "w1:p2".into(),
            preview: "preview".into(),
            project: "dotfiles".into(),
            state: "done".into(),
            title: "shell · done · dotfiles".into(),
        },
    }
}

#[test]
fn egress_preserves_the_original_request_and_rendered_event_in_both_modes() {
    let mut event = event();
    assert_eq!(decode(GOLDEN.as_bytes()).unwrap(), event);
    assert_eq!(event.encode().unwrap(), GOLDEN.trim_end());
    for (mode, word) in [
        (EgressMode::Silent, "async"),
        (EgressMode::ReportOutcome, "sync"),
    ] {
        event.body.mode = mode;
        let bytes = event.encode().unwrap();
        let wire: Value = serde_json::from_str(&bytes).unwrap();
        assert_eq!(wire["schema"], "pns.egress/1");
        assert_eq!(wire["request_id"], "shell-42");
        assert_eq!(wire["body"]["mode"], word);
        assert_eq!(decode(bytes.as_bytes()).unwrap(), event);
    }
}

#[test]
fn egress_refuses_unknown_schemas_and_majors_with_the_original_id() {
    let golden: Value = serde_json::from_str(GOLDEN).unwrap();
    for (schema, reason) in [
        (json!("pns.egress/0"), Rejection::MajorUnsupported(0)),
        (json!("pns.egress/2"), Rejection::MajorUnsupported(2)),
        (
            json!("pns.request/1"),
            Rejection::SchemaUnknown("pns.request/1".into()),
        ),
        (
            json!("pns.egress/no"),
            Rejection::SchemaUnknown("pns.egress/no".into()),
        ),
        (Value::Null, Rejection::SchemaMissing),
    ] {
        let mut wire = golden.clone();
        wire["schema"] = schema;
        let rejected = decode(wire.to_string().as_bytes()).unwrap_err();
        assert_eq!(rejected.reason, reason);
        assert_eq!(rejected.request_id, Some(event().request_id));
    }
}

#[test]
fn egress_requires_the_id_and_every_typed_body_field() {
    let golden: Value = serde_json::from_str(GOLDEN).unwrap();
    for field in ["request_id", "body"] {
        let mut wire = golden.clone();
        wire.as_object_mut().unwrap().remove(field);
        assert!(
            matches!(
                decode(wire.to_string().as_bytes()).unwrap_err().reason,
                Rejection::Invalid(_)
            ),
            "{field}"
        );
    }
    for id in [json!(""), json!("two words"), json!("é"), json!(7)] {
        let mut wire = golden.clone();
        wire["request_id"] = id;
        let rejected = decode(wire.to_string().as_bytes()).unwrap_err();
        assert!(matches!(rejected.reason, Rejection::Invalid(_)));
        assert_eq!(rejected.request_id, None);
    }
    for field in [
        "agent", "branch", "detail", "message", "mode", "pane", "preview", "project", "state",
        "title",
    ] {
        for value in [None, Some(json!(false))] {
            let mut wire = golden.clone();
            let body = wire["body"].as_object_mut().unwrap();
            if let Some(value) = value {
                body.insert(field.into(), value);
            } else {
                body.remove(field);
            }
            let rejected = decode(wire.to_string().as_bytes()).unwrap_err();
            assert!(matches!(rejected.reason, Rejection::Invalid(_)), "{field}");
            assert_eq!(rejected.request_id, Some(event().request_id));
        }
    }
    let mut wire = golden;
    wire["body"]["mode"] = json!("silent");
    assert!(matches!(
        decode(wire.to_string().as_bytes()).unwrap_err().reason,
        Rejection::Invalid(_)
    ));
}

#[test]
fn egress_ignores_bounded_additive_fields_but_keeps_text_inert() {
    let mut wire: Value = serde_json::from_str(GOLDEN).unwrap();
    wire["new_field"] = json!({"command": "$(touch never)"});
    wire["body"]["new_field"] = json!(["ignored"]);
    assert_eq!(decode(wire.to_string().as_bytes()).unwrap(), event());
}

#[test]
fn egress_checks_body_text_and_whole_envelope_bytes_when_encoding_and_decoding() {
    let mut event = event();
    for chars in [MAX_TEXT_CHARS - 1, MAX_TEXT_CHARS] {
        event.body.detail = "é".repeat(chars);
        let encoded = event.encode().unwrap();
        assert_eq!(decode(encoded.as_bytes()).unwrap(), event);
    }
    event.body.detail.push('é');
    assert_eq!(
        event.encode().unwrap_err().reason,
        Rejection::Bound(Violation::Text {
            chars: MAX_TEXT_CHARS + 1
        })
    );
    let mut wire: Value = serde_json::from_str(GOLDEN).unwrap();
    wire["body"]["detail"] = json!(event.body.detail);
    assert_eq!(
        decode(wire.to_string().as_bytes()).unwrap_err().reason,
        Rejection::Bound(Violation::Text {
            chars: MAX_TEXT_CHARS + 1
        })
    );

    let body = &mut event.body;
    for text in [
        &mut body.agent,
        &mut body.branch,
        &mut body.detail,
        &mut body.message,
        &mut body.pane,
        &mut body.preview,
        &mut body.project,
        &mut body.state,
        &mut body.title,
    ] {
        *text = "x".repeat(MAX_TEXT_CHARS);
    }
    assert!(
        matches!(event.encode().unwrap_err().reason, Rejection::Bound(Violation::Bytes { bytes }) if bytes > MAX_BYTES)
    );
    let encoded =
        json!({"schema": "pns.egress/1", "request_id": "shell-42", "body": event.body}).to_string();
    assert!(
        matches!(decode(encoded.as_bytes()).unwrap_err().reason, Rejection::Bound(Violation::Bytes { bytes }) if bytes > MAX_BYTES)
    );
}

#[test]
fn egress_rejects_duplicate_keys_and_applies_bounds_to_unknown_fields() {
    let duplicate = GOLDEN.replace(
        "\"state\":\"done\"",
        "\"state\":\"done\",\"state\":\"done\"",
    );
    let rejected = decode(duplicate.as_bytes()).unwrap_err();
    assert!(matches!(rejected.reason, Rejection::Malformed(_)));
    assert_eq!(rejected.request_id, None);
    let mut wire: Value = serde_json::from_str(GOLDEN).unwrap();
    wire["new_field"] = json!(vec![Value::Null; 64]);
    assert_eq!(decode(wire.to_string().as_bytes()).unwrap(), event());
    wire["new_field"].as_array_mut().unwrap().push(Value::Null);
    assert_eq!(
        decode(wire.to_string().as_bytes()).unwrap_err().reason,
        Rejection::Bound(Violation::Items { count: 65 })
    );
}
