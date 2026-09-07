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
