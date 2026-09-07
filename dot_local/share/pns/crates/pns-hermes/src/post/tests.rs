use super::*;

mod post_fixture;

#[test]
fn a_malformed_url_is_never_attempted_which_is_its_own_outcome() {
    assert_eq!(
        UreqSignedPost.post("http://[::1", "{}", "sig", Some(Duration::from_secs(2))),
        PostOutcome::NoStatus
    );
}

#[test]
fn a_closed_port_is_no_response() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/hook", listener.local_addr().unwrap());
    drop(listener);
    assert_eq!(
        UreqSignedPost.post(&url, "{}", "sig", Some(Duration::from_secs(2))),
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
        let outcome = UreqSignedPost.post(&url, r#"{"signed":true}"#, "sig", Some(DEADLINE));
        server
            .join()
            .unwrap()
            .expect("the signed request must arrive");
        outcome
    });
    assert!(decoy.accept().is_err(), "the signed body must stay home");
    assert_eq!(outcome, PostOutcome::Status(307));
}
