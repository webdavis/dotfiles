use super::{Rejection, open};
use crate::bounds::{MAX_BYTES, MAX_FIELDS, Violation};
use crate::identifiers::{RequestId, SchemaId};

fn request_schema() -> SchemaId {
    SchemaId {
        name: "pns.request".to_string(),
        major: 1,
    }
}

fn id(text: &str) -> RequestId {
    RequestId::new(text).unwrap()
}

#[test]
fn an_unknown_major_version_is_refused_by_number() {
    let text = r#"{"schema":"pns.request/2","request_id":"r-1"}"#;
    let rejected = open(text.as_bytes(), &request_schema()).unwrap_err();
    assert_eq!(rejected.reason, Rejection::MajorUnsupported(2));
    assert_eq!(rejected.reason.code(), "major_unsupported");
}

#[test]
fn the_request_id_survives_a_rejection_so_the_result_can_still_be_correlated() {
    let text = r#"{"schema":"pns.request/2","request_id":"r-1"}"#;
    let rejected = open(text.as_bytes(), &request_schema()).unwrap_err();
    assert_eq!(rejected.request_id, Some(id("r-1")));
    // An id that is itself invalid is not recovered: nothing downstream may
    // carry a value the identifier rules refuse.
    let text = r#"{"schema":"pns.request/2","request_id":"has space"}"#;
    let rejected = open(text.as_bytes(), &request_schema()).unwrap_err();
    assert_eq!(rejected.request_id, None);
}

#[test]
fn a_missing_or_non_string_schema_is_refused_as_missing() {
    for text in [
        r#"{"request_id":"r-1"}"#,
        r#"{"schema":1,"request_id":"r-1"}"#,
    ] {
        let rejected = open(text.as_bytes(), &request_schema()).unwrap_err();
        assert_eq!(rejected.reason, Rejection::SchemaMissing, "{text}");
        assert_eq!(rejected.reason.code(), "schema_missing");
        assert_eq!(rejected.request_id, Some(id("r-1")));
    }
}

#[test]
fn a_schema_naming_another_envelope_or_no_envelope_at_all_is_refused_as_unknown() {
    for schema in ["pns.result/1", "nonsense", "pns.request/one"] {
        let text = format!(r#"{{"schema":"{schema}","request_id":"r-1"}}"#);
        let rejected = open(text.as_bytes(), &request_schema()).unwrap_err();
        assert_eq!(
            rejected.reason,
            Rejection::SchemaUnknown(schema.to_string())
        );
        assert_eq!(rejected.reason.code(), "schema_unknown");
    }
}

#[test]
fn bytes_that_are_not_a_json_object_are_refused_as_malformed_and_carry_no_id() {
    for text in ["{not json", "[]", "\"r-1\"", ""] {
        let rejected = open(text.as_bytes(), &request_schema()).unwrap_err();
        assert!(
            matches!(rejected.reason, Rejection::Malformed(_)),
            "{text:?}: {:?}",
            rejected.reason
        );
        assert_eq!(rejected.reason.code(), "malformed_json");
        assert_eq!(rejected.request_id, None);
    }
}

#[test]
fn the_byte_cap_is_checked_before_anything_is_parsed() {
    let mut text = String::from(r#"{"schema":"pns.request/1","request_id":"r-1"}"#);
    let padding = MAX_BYTES + 1 - text.len();
    text.push_str(&" ".repeat(padding));
    let rejected = open(text.as_bytes(), &request_schema()).unwrap_err();
    assert_eq!(
        rejected.reason,
        Rejection::Bound(Violation::Bytes {
            bytes: MAX_BYTES + 1
        })
    );
    assert_eq!(rejected.reason.code(), "bytes_over_cap");
}

#[test]
fn a_bound_violation_inside_the_object_is_refused_before_the_schema_is_read() {
    let mut fields = Vec::new();
    for index in 0..=MAX_FIELDS {
        fields.push(format!(r#""k{index}":null"#));
    }
    let text = format!(r#"{{"schema":"pns.request/2",{}}}"#, fields.join(","));
    let rejected = open(text.as_bytes(), &request_schema()).unwrap_err();
    assert_eq!(
        rejected.reason,
        Rejection::Bound(Violation::Fields {
            count: MAX_FIELDS + 2
        })
    );
}

#[test]
fn an_envelope_that_passes_every_check_opens_with_its_object_and_its_id() {
    let text = r#"{"schema":"pns.request/1","request_id":"r-1","extra":true}"#;
    let opened = open(text.as_bytes(), &request_schema()).unwrap();
    assert_eq!(opened.request_id, Some(id("r-1")));
    assert_eq!(opened.value["extra"], serde_json::json!(true));
}
