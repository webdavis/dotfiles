mod tests {
    use super::super::*;

    const SECRET: &str = "a-webhook-secret";
    const BODY: &str = "{\"action\":\"completed\"}";

    /// One delivery as GitHub sends it, signed with `secret`.
    fn delivery_request(secret: &str, body: &str) -> Vec<u8> {
        request(
            &[
                ("X-GitHub-Event", "workflow_run"),
                ("X-GitHub-Delivery", "72d3162e-cc78-11e3-81ab-4c9367dc0958"),
                (
                    "X-Hub-Signature-256",
                    &format!("sha256={}", digest(secret, body)),
                ),
            ],
            body,
        )
    }

    /// The raw request bytes for these headers and this body.
    fn request(headers: &[(&str, &str)], body: &str) -> Vec<u8> {
        let mut raw = format!("POST {WEBHOOK_PATH} HTTP/1.1\r\nHost: 127.0.0.1\r\n");
        for (name, value) in headers {
            raw.push_str(&format!("{name}: {value}\r\n"));
        }
        raw.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
        let mut bytes = raw.into_bytes();
        bytes.extend_from_slice(body.as_bytes());
        bytes
    }

    /// The hex digest GitHub would send, computed the way the documentation
    /// states it: an HMAC-SHA256 hex digest of the request body.
    fn digest(secret: &str, body: &str) -> String {
        use hmac::{Hmac, KeyInit, Mac};
        let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).expect("key");
        mac.update(body.as_bytes());
        mac.finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    #[test]
    fn a_delivery_signed_with_the_configured_secret_is_verified() {
        assert_eq!(
            delivery(&delivery_request(SECRET, BODY), SECRET),
            Delivery::Verified
        );
        assert_eq!(Delivery::Verified.status(), 204);
    }

    #[test]
    fn a_delivery_signed_with_another_secret_or_over_another_body_is_refused() {
        // THE MUTANT THIS PINS: a signature check that reads the header and
        // believes it. The second case is the replayed digest: the right
        // secret, a body somebody else wrote.
        let forged = delivery_request("not-the-secret", BODY);
        assert!(matches!(delivery(&forged, SECRET), Delivery::Refused(_)));
        let mut replayed = delivery_request(SECRET, BODY);
        let tail = replayed.len() - 1;
        replayed[tail] = b' ';
        assert!(matches!(delivery(&replayed, SECRET), Delivery::Refused(_)));
    }

    #[test]
    fn a_request_with_no_signature_header_is_refused_and_so_is_an_unusable_one() {
        for headers in [
            vec![
                ("X-GitHub-Event", "workflow_run"),
                ("X-GitHub-Delivery", "72d3162e"),
            ],
            vec![
                ("X-GitHub-Event", "workflow_run"),
                ("X-GitHub-Delivery", "72d3162e"),
                ("X-Hub-Signature-256", "sha256="),
            ],
            vec![
                ("X-GitHub-Event", "workflow_run"),
                ("X-GitHub-Delivery", "72d3162e"),
                ("X-Hub-Signature-256", "sha1=abcd"),
            ],
            vec![
                ("X-GitHub-Event", "workflow_run"),
                ("X-GitHub-Delivery", "72d3162e"),
                ("X-Hub-Signature-256", "sha256=zzzz"),
            ],
        ] {
            let refused = delivery(&request(&headers, BODY), SECRET);
            assert!(
                matches!(refused, Delivery::Refused(_)),
                "{headers:?} was accepted"
            );
            assert_eq!(refused.status(), 403);
        }
    }

    #[test]
    fn a_request_that_is_not_a_github_delivery_at_all_is_refused_before_the_secret() {
        // A scanner walking the hostname, and a delivery to a path this
        // receiver does not serve. Both refused, both with the one answer.
        let signed = format!("sha256={}", digest(SECRET, BODY));
        assert!(matches!(
            delivery(&request(&[("X-Hub-Signature-256", &signed)], BODY), SECRET),
            Delivery::Refused("not a GitHub delivery")
        ));
        let mut raw = delivery_request(SECRET, BODY);
        let path = String::from_utf8_lossy(&raw).replace(WEBHOOK_PATH, "/webhooks/other");
        raw = path.into_bytes();
        assert!(matches!(
            delivery(&raw, SECRET),
            Delivery::Refused("not the webhook path")
        ));
        let getter = String::from_utf8_lossy(&delivery_request(SECRET, BODY))
            .replace("POST ", "GET ")
            .into_bytes();
        assert!(matches!(
            delivery(&getter, SECRET),
            Delivery::Refused("not a POST")
        ));
        assert!(matches!(delivery(b"", SECRET), Delivery::Refused(_)));
    }

    #[test]
    fn a_receiver_with_no_secret_verifies_nothing_however_the_request_is_signed() {
        // THE MUTANT THIS PINS: an empty key that hashes to something and
        // matches a payload signed the same way, which would make a
        // misconfigured receiver accept whatever the internet sent it.
        assert!(matches!(
            delivery(&delivery_request("", BODY), ""),
            Delivery::Refused(_)
        ));
    }

    #[test]
    fn a_body_over_the_ceiling_is_refused_and_a_stated_length_over_it_is_read_off_the_head() {
        let huge = "x".repeat(WEBHOOK_BODY_MAX + 1);
        assert!(matches!(
            delivery(&delivery_request(SECRET, &huge), SECRET),
            Delivery::Refused("body over the ceiling")
        ));
        let head = "POST / HTTP/1.1\r\nContent-Length: 12\r\n\r\n";
        assert_eq!(content_length(head), Some(12));
        assert_eq!(content_length("POST / HTTP/1.1\r\n\r\n"), None);
    }
}
