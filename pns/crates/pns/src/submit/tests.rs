use super::*;
use pns_protocol::{
    DeliveryOutcome, DestinationOutcome, InteractionResult, MAX_BYTES, MAX_ITEMS, Name, RequestId,
    decode_result,
};
use std::cell::Cell;

const REQUEST: &[u8] = br#"{"schema":"pns.request/1","request_id":"posture-occurrence","producer":"posture","event":"heartbeat","signal":{"kind":"observation"},"session":{"id":"session","turn":9},"occurred_at":42,"elapsed_secs":7,"detail":"private body","context":{"project":"repo","branch":"topic","pane":"pane"},"scope":"remote_only","route":"posture","interaction":{"kind":"none"},"extensions":{"posture":{"count":2}},"future":true}"#;
fn args() -> Vec<String> {
    vec!["--json".into()]
}
fn receipt(status: Status) -> ResultEnvelope {
    ResultEnvelope {
        request_id: Some(RequestId::new("posture-occurrence").unwrap()),
        status,
        decision_id: Some("d-17".into()),
        interaction: Some(InteractionResult::NoOpinion),
        destinations: vec![DestinationOutcome {
            destination: Name::new("hermes").unwrap(),
            outcome: DeliveryOutcome::Failed,
            note: Some("transport unavailable".into()),
        }],
        diagnostics: vec!["ledger_committed".into()],
    }
}
#[test]
fn one_valid_request_reaches_the_callback_with_every_decoded_field_intact() {
    let calls = Cell::new(0);
    let mut output = Vec::new();
    let status = run(&args(), REQUEST, &mut output, |decoded| {
        calls.set(calls.get() + 1);
        assert_eq!(decoded, pns_protocol::decode_request(REQUEST).unwrap());
        assert_eq!(decoded.ignored, ["future"]);
        receipt(Status::Accepted)
    })
    .unwrap();
    assert_eq!(calls.get(), 1, "a valid request must be submitted once");
    assert_eq!(status, Status::Accepted);
    assert_eq!(decode_result(&output).unwrap(), receipt(Status::Accepted));
}
#[test]
fn decoder_refusals_remain_correlated_without_submitting_or_echoing_private_text() {
    let invalid = String::from_utf8(REQUEST.to_vec())
        .unwrap()
        .replace("observation", "unknown_signal");
    let mut output = Vec::new();
    let status = run(&args(), invalid.as_bytes(), &mut output, |_| {
        panic!("invalid input must not reach delivery")
    })
    .unwrap();
    let decoded = decode_result(&output).unwrap();
    assert_eq!(status, Status::Rejected);
    assert_eq!(
        decoded.request_id,
        Some(RequestId::new("posture-occurrence").unwrap())
    );
    assert_eq!(decoded.diagnostics, ["field_invalid"]);
    assert!(decoded.destinations.is_empty());
    assert!(!String::from_utf8(output).unwrap().contains("private body"));
}
#[test]
fn malformed_and_multiple_envelopes_are_refused_before_the_callback() {
    for input in [b"{".as_slice(), [REQUEST, REQUEST].concat().as_slice()] {
        let mut output = Vec::new();
        run(&args(), input, &mut output, |_| panic!("not one request")).unwrap();
        let result = decode_result(&output).unwrap();
        assert_eq!(result.status, Status::Rejected);
        assert_eq!(result.request_id, None);
        assert_eq!(result.diagnostics, ["malformed_json"]);
    }
}
#[test]
fn the_byte_ceiling_accepts_its_edge_and_reads_only_one_byte_beyond_it() {
    for size in [MAX_BYTES - 1, MAX_BYTES, MAX_BYTES + 2] {
        let mut bytes = REQUEST.to_vec();
        bytes.resize(size, b' ');
        let mut input = bytes.as_slice();
        let mut output = Vec::new();
        let calls = Cell::new(0);
        let status = run(&args(), &mut input, &mut output, |_| {
            calls.set(calls.get() + 1);
            receipt(Status::Accepted)
        })
        .unwrap();
        assert_eq!(size - input.len(), size.min(MAX_BYTES + 1));
        assert_eq!(calls.get(), usize::from(size <= MAX_BYTES));
        assert_eq!(
            status,
            if size <= MAX_BYTES {
                Status::Accepted
            } else {
                Status::Rejected
            }
        );
        if size > MAX_BYTES {
            assert_eq!(
                decode_result(&output).unwrap().diagnostics,
                ["bytes_over_cap"]
            );
        }
    }
}
struct Unreadable<'a>(&'a [u8]);
impl Read for Unreadable<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self.0.is_empty() {
            Err(io::Error::other("private input failure"))
        } else {
            self.0.read(output)
        }
    }
}
#[test]
fn a_read_failure_is_a_nonsecret_refusal_and_never_submits_a_partial_request() {
    let mut output = Vec::new();
    let status = run(&args(), Unreadable(REQUEST), &mut output, |_| {
        panic!("unreadable input")
    })
    .unwrap();
    assert_eq!(status, Status::Rejected);
    let result = decode_result(&output).unwrap();
    assert_eq!(result.request_id, None);
    assert_eq!(result.diagnostics, ["input_unreadable"]);
    assert!(
        !String::from_utf8(output)
            .unwrap()
            .contains("private input failure")
    );
}
#[test]
fn any_argument_shape_other_than_json_is_refused_before_reading() {
    for args in [
        vec![],
        vec!["--json".into(), "extra".into()],
        vec!["--other".into()],
    ] {
        let mut output = Vec::new();
        let status = run(&args, Unreadable(&[]), &mut output, |_| {
            panic!("invalid arguments")
        })
        .unwrap();
        assert_eq!(status, Status::Rejected);
        assert_eq!(
            decode_result(&output).unwrap().diagnostics,
            ["submit_usage"]
        );
    }
}
mod output;
