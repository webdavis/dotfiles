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

    /// One full round trip through `answer` on a real loopback connection:
    /// send `request`, return the response bytes and how many times the
    /// doorbell rang. THE REAL SEAM, unlike the two tests above: this drives
    /// `read_request`'s framing (the header ceiling, the stated-length
    /// ceiling, a partial body) as well as the verdict, none of which a
    /// direct call to `pns_adapters::delivery` ever touches.
    fn round_trip(secret: &str, request: Vec<u8>) -> (String, u32) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("local_addr");
        let sender = std::thread::spawn(move || {
            let mut client = TcpStream::connect(addr).expect("connect");
            client.write_all(&request).expect("write");
            client.shutdown(std::net::Shutdown::Write).ok();
            let mut response = Vec::new();
            client.read_to_end(&mut response).ok();
            response
        });
        let (stream, _) = listener.accept().expect("accept");
        let mut rung = 0;
        answer(stream, secret, &mut || rung += 1);
        let response = sender.join().expect("sender thread panicked");
        (String::from_utf8_lossy(&response).into_owned(), rung)
    }

    #[test]
    fn a_signed_delivery_over_a_real_connection_is_answered_204_with_no_content_length() {
        let (response, rung) = round_trip(SECRET, delivery(SECRET));
        assert!(
            response.starts_with("HTTP/1.1 204 No Content\r\n"),
            "unexpected status line: {response}"
        );
        // RFC 9110 forbids Content-Length on a 204.
        assert!(
            !response.contains("Content-Length"),
            "a 204 must carry no Content-Length: {response}"
        );
        assert_eq!(rung, 1, "the doorbell rang {rung} times");
    }

    #[test]
    fn a_stated_length_over_the_ceiling_is_refused_before_the_body_is_read() {
        // THE MUTANT THIS PINS: reading to end of file instead of stopping at
        // the stated length would hang against a client that never sends the
        // body it claimed, exactly what this request does.
        let head = format!(
            "POST {} HTTP/1.1\r\nX-GitHub-Event: workflow_run\r\n\
             X-GitHub-Delivery: 1\r\nX-Hub-Signature-256: sha256=00\r\n\
             Content-Length: {}\r\n\r\n",
            pns_adapters::WEBHOOK_PATH,
            pns_adapters::WEBHOOK_BODY_MAX + 1
        );
        let (response, rung) = round_trip(SECRET, head.into_bytes());
        assert!(
            response.starts_with("HTTP/1.1 403 Forbidden\r\n"),
            "unexpected status line: {response}"
        );
        assert_eq!(rung, 0, "an oversized Content-Length rang the doorbell");
    }

    #[test]
    fn a_request_with_no_content_length_is_refused_rather_than_treated_as_an_empty_body() {
        let request = format!(
            "POST {} HTTP/1.1\r\nX-GitHub-Event: workflow_run\r\n\
             X-GitHub-Delivery: 1\r\nX-Hub-Signature-256: sha256=00\r\n\r\n",
            pns_adapters::WEBHOOK_PATH
        );
        let (response, rung) = round_trip(SECRET, request.into_bytes());
        assert!(
            response.starts_with("HTTP/1.1 403 Forbidden\r\n"),
            "unexpected status line: {response}"
        );
        assert_eq!(rung, 0, "a request with no stated length rang the doorbell");
    }

    #[test]
    fn a_header_block_over_the_ceiling_is_refused() {
        // No \r\n\r\n anywhere in this, so head_end() never fires and the
        // ceiling is the only thing that stops this from reading forever.
        let request = vec![b'a'; HEAD_MAX + 1024];
        let (response, rung) = round_trip(SECRET, request);
        assert!(
            response.starts_with("HTTP/1.1 403 Forbidden\r\n"),
            "unexpected status line: {response}"
        );
        assert_eq!(rung, 0, "an oversized header block rang the doorbell");
    }
}
