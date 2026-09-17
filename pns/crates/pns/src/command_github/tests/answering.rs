mod tests {
    use super::super::*;

    const SECRET: &str = "a-webhook-secret";
    const BODY: &str = "{\"action\":\"completed\"}";

    /// One delivery as GitHub sends it, signed with `secret`.
    fn delivery(secret: &str) -> Vec<u8> {
        use hmac::{KeyInit, Mac};
        let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).expect("key");
        mac.update(BODY.as_bytes());
        let digest: String = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let head = format!(
            "POST {} HTTP/1.1\r\nX-GitHub-Event: workflow_run\r\n\
             X-GitHub-Delivery: 72d3162e\r\nX-Hub-Signature-256: sha256={digest}\r\n\
             Content-Length: {}\r\n\r\n",
            pns_adapters::WEBHOOK_PATH,
            BODY.len()
        );
        let mut raw = head.into_bytes();
        raw.extend_from_slice(BODY.as_bytes());
        raw
    }

    #[test]
    fn a_verified_delivery_rings_the_doorbell_once_and_is_answered_204() {
        let verdict = pns_adapters::delivery(&delivery(SECRET), SECRET);
        assert_eq!(verdict.status(), 204);
        let mut rung = 0;
        ring(verdict, &mut || rung += 1);
        assert_eq!(rung, 1, "the doorbell rang {rung} times");
    }

    #[test]
    fn an_unsigned_or_wrongly_signed_request_rings_nothing_and_is_answered_403() {
        // THE MUTANT THIS PINS: a receiver that polls on whatever arrives,
        // which would let a stranger drive this machine's GitHub requests by
        // sending it empty POSTs.
        for (name, request) in [
            ("wrongly signed", delivery("not-the-secret")),
            (
                "unsigned",
                b"POST /webhooks/github HTTP/1.1\r\n\r\n".to_vec(),
            ),
            ("empty", Vec::new()),
        ] {
            let verdict = pns_adapters::delivery(&request, SECRET);
            assert_eq!(verdict.status(), 403, "{name} was accepted");
            let mut rung = 0;
            ring(verdict, &mut || rung += 1);
            assert_eq!(rung, 0, "{name} rang the doorbell");
        }
    }
}
