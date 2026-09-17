use super::*;
use crate::test_gateway::CaptureGateway;

#[test]
fn a_post_carries_its_signature_and_its_request_id_as_headers_the_gateway_reads() {
    let gateway = CaptureGateway::expecting(1);
    let outcome = UreqSignedPost.post(
        &format!("{}/posture-pages", gateway.base_url()),
        "{\"agent\":\"posture\"}",
        "cafef00d",
        "posture-0123456789abcdef",
        Some(Duration::from_secs(5)),
    );
    assert_eq!(outcome, PostOutcome::Status(204));
    let heads = gateway.requests();
    let head = heads.first().expect("one request reached the gateway");
    // A header name is case insensitive on the wire and the client picks its
    // own casing, so the match is made against one casing rather than the
    // spelling this call passed in.
    let head = head.to_lowercase();
    assert!(head.contains("post /webhooks/posture-pages"), "{head}");
    assert!(head.contains("x-webhook-signature: cafef00d"), "{head}");
    assert!(
        head.contains("x-request-id: posture-0123456789abcdef"),
        "the gateway falls back to a millisecond timestamp without it, and \
         then reads a retry of one page as a second page: {head}"
    );
}
