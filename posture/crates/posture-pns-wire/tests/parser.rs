use posture_pns_wire::{Rejection, Violation, decode_request, decode_result};

const REQUEST: &str = include_str!("../fixtures/request-v1.json");
const RESULT: &str = include_str!("../fixtures/result-v1.json");

#[test]
fn oversized_malformed_bytes_are_refused_before_json_parsing() {
    let bytes = vec![b'?'; 65_537];
    assert_eq!(
        decode_request(&bytes).unwrap_err().reason,
        Rejection::Bound(Violation::Bytes { bytes: 65_537 })
    );
}

#[test]
fn the_parser_stops_at_the_depth_cap_before_reading_deeper_values() {
    let text = format!("{}?", "[".repeat(9));
    assert_eq!(
        decode_request(text.as_bytes()).unwrap_err().reason,
        Rejection::Bound(Violation::Depth { depth: 9 })
    );
}

#[test]
fn a_second_top_level_value_is_refused() {
    let text = format!("{REQUEST} {{}}");
    assert!(matches!(
        decode_request(text.as_bytes()).unwrap_err().reason,
        Rejection::Malformed(_)
    ));
}

#[test]
fn a_repeated_schema_is_refused_even_when_both_values_agree() {
    let text = REQUEST.replacen(
        "\"schema\":",
        "\"schema\": \"pns.request/1\", \"schema\":",
        1,
    );
    assert!(matches!(
        decode_request(text.as_bytes()).unwrap_err().reason,
        Rejection::Malformed(_)
    ));
}

#[test]
fn repeated_result_fields_are_refused_too() {
    let text = RESULT.replacen("\"status\":", "\"status\": \"accepted\", \"status\":", 1);
    assert!(matches!(
        decode_result(text.as_bytes()).unwrap_err().reason,
        Rejection::Malformed(_)
    ));
}
