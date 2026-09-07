use super::*;

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
    use super::super::super::post_fixture::{DEADLINE, serve};
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

// --- delivery ------------------------------------------------------------

#[test]
fn a_key_posts_once_with_the_signature_of_the_exact_body_bytes() {
    let channel = channel_with_settings("key = \"key\"\n", PostOutcome::Status(200));
    assert_eq!(
        channel.deliver(&event(), ReportMode::Silent),
        Delivery::Delivered("posted HTTP 200".to_string()),
        "the channel reports what happened; the leg's mode decides who hears it"
    );
    let posts = channel.post.posts.borrow();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].0, "http://127.0.0.1:9/test");
    assert_eq!(
        Some(posts[0].2.as_str()),
        super::sign("key", &posts[0].1).as_deref(),
        "the signature covers the body that was actually posted"
    );
    assert_eq!(
        posts[0].3,
        Some(Duration::from_secs(10)),
        "async carries the ten second deadline"
    );
}

#[test]
fn sync_carries_the_validated_sync_deadline() {
    let channel = channel_with_settings("key = \"key\"\n", PostOutcome::Status(200));
    channel.deliver(&event(), ReportMode::ReportOutcome);
    assert_eq!(
        channel.post.posts.borrow()[0].3,
        Some(Duration::from_secs(5))
    );
}

#[test]
fn no_key_means_no_post_in_either_mode_and_the_verdict_is_a_failure() {
    for mode in [ReportMode::Silent, ReportMode::ReportOutcome] {
        let channel = channel_with_settings("", PostOutcome::Status(200));
        assert_eq!(
            channel.deliver(&event(), mode),
            Delivery::Failed(super::skipped_line()),
            "not set up is reported in both modes; only sync prints it"
        );
        assert!(channel.post.posts.borrow().is_empty());
    }
}

// --- the verdict ---------------------------------------------------------

#[test]
fn a_2xx_is_delivered_and_every_other_answer_is_failed_carrying_its_own_sentence() {
    // THE VERDICT IS READABLE WITHOUT READING ENGLISH. A caller that had to
    // decide "did this work" by looking for the word FAILED inside the
    // sentence is a predicate keyed on message text, which is a defect this
    // repo has already paid for once.
    for (outcome, expected) in [
        (
            PostOutcome::Status(200),
            Delivery::Delivered("posted HTTP 200".to_string()),
        ),
        (
            PostOutcome::Status(204),
            Delivery::Delivered("posted HTTP 204".to_string()),
        ),
        (
            PostOutcome::Status(401),
            Delivery::Failed("post FAILED HTTP 401".to_string()),
        ),
        (
            // A redirect is the final answer here, so it is not a delivery.
            PostOutcome::Status(301),
            Delivery::Failed("post FAILED HTTP 301".to_string()),
        ),
        (
            PostOutcome::NoResponse,
            Delivery::Failed(
                "post FAILED HTTP 000 (no response; is the hermes gateway up?)".to_string(),
            ),
        ),
        (
            PostOutcome::NoStatus,
            Delivery::Failed("post FAILED (curl reported no HTTP status at all)".to_string()),
        ),
    ] {
        let channel = channel_with_settings("key = \"key\"\n", outcome);
        assert_eq!(
            channel.deliver(&event(), ReportMode::ReportOutcome),
            expected,
            "case: {outcome:?}"
        );
    }
}
