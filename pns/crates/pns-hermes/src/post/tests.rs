use super::*;

mod post_fixture;

#[test]
fn a_malformed_url_is_never_attempted_which_is_its_own_outcome() {
    assert_eq!(
        UreqSignedPost.post(
            "http://[::1",
            "{}",
            "sig",
            None,
            Some(Duration::from_secs(2))
        ),
        PostOutcome::NoStatus
    );
}

#[test]
fn a_closed_port_is_no_response() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/hook", listener.local_addr().unwrap());
    drop(listener);
    assert_eq!(
        UreqSignedPost.post(&url, "{}", "sig", None, Some(Duration::from_secs(2))),
        PostOutcome::NoResponse
    );
}

#[test]
fn a_redirecting_gateway_is_the_final_answer_and_the_signed_body_stays_home() {
    use self::post_fixture::{DEADLINE, serve};
    let decoy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let decoy_addr = decoy.local_addr().unwrap();
    decoy.set_nonblocking(true).unwrap();
    let redirector = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/hook", redirector.local_addr().unwrap());
    let response = format!(
        "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{decoy_addr}/\r\nContent-Length: 0\r\n\r\n"
    );
    let outcome = std::thread::scope(|scope| {
        let server = scope.spawn(|| serve(redirector, &response, br#"{"signed":true}"#));
        let outcome = UreqSignedPost.post(&url, r#"{"signed":true}"#, "sig", None, Some(DEADLINE));
        server
            .join()
            .unwrap()
            .expect("the signed request must arrive");
        outcome
    });
    assert!(decoy.accept().is_err(), "the signed body must stay home");
    assert_eq!(outcome, PostOutcome::Status(307));
}

#[test]
fn the_original_idempotency_key_reaches_the_wire_without_changing_the_signed_body() {
    use self::post_fixture::{DEADLINE, serve};
    for key in [Some("producer:event/1"), None] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/hook", listener.local_addr().unwrap());
        let body = r#"{"request_id":"producer:event/1","detail":"original"}"#;
        let signature = sign("fixture-key", body).unwrap();
        let response = "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n";
        let request = std::thread::scope(|scope| {
            let server = scope.spawn(|| serve(listener, response, body.as_bytes()));
            let outcome = UreqSignedPost.post(&url, body, &signature, key, Some(DEADLINE));
            let request = server.join().unwrap().unwrap();
            assert_eq!(outcome, PostOutcome::Status(204));
            request
        });
        let request = String::from_utf8(request).unwrap();
        let (headers, sent_body) = request.split_once("\r\n\r\n").unwrap();
        let header = |name: &str| {
            headers
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find_map(|(field, value)| field.eq_ignore_ascii_case(name).then(|| value.trim()))
        };
        assert_eq!(header("Idempotency-Key"), key);
        assert_eq!(header("X-Webhook-Signature"), Some(signature.as_str()));
        assert_eq!(sent_body, body);
    }
}
