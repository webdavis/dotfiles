use super::*;

#[test]
fn a_redirect_is_not_a_delivery_however_the_endpoint_dresses_it_up() {
    // `max_redirects(0)` hands a 3xx back as an Ok RESPONSE rather than an
    // error, so a bare `is_ok` read one as a card that landed: the check
    // printed Delivered for a phone that was never pushed to. THE 200 IS
    // THE OTHER HALF, over a real socket, so the answer cannot become a
    // blanket false and pass this by refusing everything.
    use crate::destinations::post_fixture::{DEADLINE, serve};
    for (status, delivered) in [("302 Found", false), ("200 OK", true)] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/hook", listener.local_addr().unwrap());
        let response = format!(
            "HTTP/1.1 {status}\r\nLocation: http://127.0.0.1:1/\r\nContent-Length: 0\r\n\r\n"
        );
        let result = std::thread::scope(|scope| {
            let server = scope.spawn(|| serve(listener, &response, b"{}"));
            let result = UreqPost { timeout: DEADLINE }.post_json(&url, "{}");
            server.join().unwrap().expect("the request must arrive");
            result
        });
        assert_eq!(result, delivered, "the endpoint answered {status}");
    }
}

#[test]
fn a_redirecting_endpoint_is_never_followed() {
    // The bash curl carried no -L: a compromised endpoint must not turn
    // the channel into a blind request against a target it names. The
    // decoy listener proves no second request happens.
    use crate::destinations::post_fixture::{DEADLINE, serve};
    let decoy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let decoy_addr = decoy.local_addr().unwrap();
    decoy.set_nonblocking(true).unwrap();
    let redirector = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/hook", redirector.local_addr().unwrap());
    let response = format!(
        "HTTP/1.1 302 Found\r\nLocation: http://{decoy_addr}/\r\nContent-Length: 0\r\n\r\n"
    );
    std::thread::scope(|scope| {
        let server = scope.spawn(|| serve(redirector, &response, b"{}"));
        UreqPost { timeout: DEADLINE }.post_json(&url, "{}");
        server.join().unwrap().expect("the request must arrive");
    });
    assert!(
        decoy.accept().is_err(),
        "the redirect target must never be contacted"
    );
}

#[test]
fn the_upload_carries_the_token_in_an_authorization_header_and_not_in_its_body() {
    // MEASURED WITH A BOGUS TOKEN, over a loopback socket, because the
    // placement is the whole reason this leg needed a ruling: the live route
    // reads the token ONLY from this header (2026-09-15), while the webhook
    // beside it reads it only from the body. A test that asserted the body
    // alone would pass on a request that shipped no credential at all.
    use crate::destinations::post_fixture::{DEADLINE, serve};
    let png = b"pretend-png-bytes";
    let body = crate::destinations::moshi::upload::multipart(png).expect("the body builds");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/upload", listener.local_addr().unwrap());
    let reply = r#"{"id":"abcde","code":"abcde1xy","expires_at":"2026-07-23T12:00:00.000Z"}"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{reply}",
        reply.len()
    );
    let (code, request) = std::thread::scope(|scope| {
        let server = scope.spawn(|| serve(listener, &response, &body));
        let code =
            UreqPost { timeout: DEADLINE }.upload_png(&url, "definitely-not-a-real-token", png);
        let request = server.join().unwrap().expect("the request must arrive");
        (code, request)
    });
    assert_eq!(
        code.as_deref(),
        Some("abcde1xy"),
        "the code was not read back"
    );
    let request = String::from_utf8_lossy(&request);
    let (headers, sent) = request
        .split_once("\r\n\r\n")
        .expect("a request with a body");
    assert!(
        headers.contains("authorization: Bearer definitely-not-a-real-token")
            || headers.contains("Authorization: Bearer definitely-not-a-real-token"),
        "no bearer header was sent: {headers}"
    );
    assert!(
        headers.contains("multipart/form-data; boundary="),
        "the upload was not sent as multipart: {headers}"
    );
    assert!(
        !sent.contains("definitely-not-a-real-token"),
        "the token was also put in the upload body"
    );
}

#[test]
fn an_upload_the_endpoint_refused_yields_no_code() {
    // A non-2xx must not be read as an upload, or the card would point at an
    // image host address nothing was ever filed under.
    use crate::destinations::post_fixture::{DEADLINE, serve};
    let png = b"pretend-png-bytes";
    let body = crate::destinations::moshi::upload::multipart(png).expect("the body builds");
    for status in ["401 Unauthorized", "302 Found", "500 Internal Server Error"] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/upload", listener.local_addr().unwrap());
        let response = format!(
            "HTTP/1.1 {status}\r\nLocation: http://127.0.0.1:1/\r\nContent-Length: 0\r\n\r\n"
        );
        let code = std::thread::scope(|scope| {
            let server = scope.spawn(|| serve(listener, &response, &body));
            let code = UreqPost { timeout: DEADLINE }.upload_png(&url, "bogus", png);
            server.join().unwrap().expect("the request must arrive");
            code
        });
        assert_eq!(code, None, "the endpoint answered {status}");
    }
}
