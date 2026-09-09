use pns_protocol::{
    DeliveryOutcome, DestinationOutcome, Name, Rejected, Rejection, Request, RequestId,
    ResultEnvelope, Signal, Violation, decode_request, decode_result,
};
use serde_json::{Value, json};

fn request() -> Request {
    Request::new(
        RequestId::new("r-1").unwrap(),
        Name::new("shell").unwrap(),
        Name::new("finished").unwrap(),
        Signal::Succeeded,
    )
}

fn result() -> ResultEnvelope {
    ResultEnvelope::rejected(&Rejected {
        request_id: Some(RequestId::new("r-1").unwrap()),
        reason: Rejection::MajorUnsupported(2),
    })
}

#[test]
fn a_consumer_constructs_and_round_trips_a_request_through_the_public_boundary() {
    let request = request();
    let wire = request.encode().unwrap();
    assert_eq!(decode_request(wire.as_bytes()).unwrap().request, request);
}

#[test]
fn a_consumer_constructs_and_round_trips_a_result_through_the_public_boundary() {
    let result = result();
    let wire = result.encode().unwrap();
    assert_eq!(decode_result(wire.as_bytes()).unwrap(), result);
}

#[test]
fn request_encoding_refuses_text_over_the_wire_cap() {
    let mut request = request();
    for chars in [7_999, 8_000] {
        request.detail = "é".repeat(chars);
        assert!(request.encode().is_ok(), "{chars} characters");
    }
    request.detail.push('é');
    assert_eq!(
        request.encode().unwrap_err().reason,
        Rejection::Bound(Violation::Text { chars: 8_001 })
    );
}

#[test]
fn result_encoding_refuses_a_destination_note_over_the_wire_cap() {
    let mut result = result();
    result.destinations.push(DestinationOutcome {
        destination: Name::new("banner").unwrap(),
        outcome: DeliveryOutcome::Failed,
        note: Some("x".repeat(8_000)),
    });
    assert!(result.encode().is_ok());
    result.destinations[0].note.as_mut().unwrap().push('x');
    assert_eq!(
        result.encode().unwrap_err().reason,
        Rejection::Bound(Violation::Text { chars: 8_001 })
    );
}

#[test]
fn result_encoding_refuses_too_many_destinations() {
    let mut result = result();
    let destination = DestinationOutcome {
        destination: Name::new("banner").unwrap(),
        outcome: DeliveryOutcome::Delivered,
        note: None,
    };
    for count in [63, 64] {
        result.destinations = vec![destination.clone(); count];
        assert!(result.encode().is_ok(), "{count} destinations");
    }
    result.destinations.push(destination);
    assert_eq!(
        result.encode().unwrap_err().reason,
        Rejection::Bound(Violation::Items { count: 65 })
    );
}

#[test]
fn request_encoding_refuses_too_many_extension_fields() {
    let mut request = request();
    for index in 0..64 {
        request.extensions.insert(format!("k{index}"), Value::Null);
    }
    assert!(request.encode().is_ok());
    request.extensions.insert("over".to_string(), Value::Null);
    assert_eq!(
        request.encode().unwrap_err().reason,
        Rejection::Bound(Violation::Fields { count: 65 })
    );
}

#[test]
fn request_encoding_refuses_extensions_beyond_the_depth_cap() {
    let mut request = request();
    let mut nested = json!({});
    // Request and extensions occupy two of the eight container levels.
    for _ in 0..5 {
        nested = json!({ "next": nested });
    }
    request
        .extensions
        .insert("nested".to_string(), nested.clone());
    assert!(request.encode().is_ok());
    request
        .extensions
        .insert("nested".to_string(), json!({ "next": nested }));
    assert_eq!(
        request.encode().unwrap_err().reason,
        Rejection::Bound(Violation::Depth { depth: 9 })
    );
}

#[test]
fn request_encoding_refuses_the_total_bytes_even_when_each_field_fits() {
    let mut request = request();
    for index in 0..8 {
        request
            .extensions
            .insert(format!("k{index}"), json!("x".repeat(8_000)));
    }
    assert!(request.encode().is_ok());
    request
        .extensions
        .insert("over".to_string(), json!("x".repeat(8_000)));
    assert!(matches!(
        request.encode().unwrap_err().reason,
        Rejection::Bound(Violation::Bytes { bytes }) if bytes > 65_536
    ));
}
