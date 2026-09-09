use super::*;

// --- delivery ------------------------------------------------------------

#[test]
fn a_key_posts_once_with_the_signature_of_the_exact_body_bytes() {
    let channel = channel_with_settings("key = \"key\"\n", PostOutcome::Status(200));
    assert_eq!(
        channel.deliver(&delivery_request(&event(), ReportMode::Silent)),
        Delivery::Delivered("posted HTTP 200".to_string()),
        "the channel reports what happened; the leg's mode decides who hears it"
    );
    let posts = channel.post.posts.lock().unwrap();
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
    channel.deliver(&delivery_request(&event(), ReportMode::ReportOutcome));
    assert_eq!(
        channel.post.posts.lock().unwrap()[0].3,
        Some(Duration::from_secs(5))
    );
}

#[test]
fn no_key_means_no_post_in_either_mode_and_the_verdict_is_a_failure() {
    for mode in [ReportMode::Silent, ReportMode::ReportOutcome] {
        let channel = channel_with_settings("", PostOutcome::Status(200));
        assert_eq!(
            channel.deliver(&delivery_request(&event(), mode)),
            Delivery::Failed(super::skipped_line()),
            "not set up is reported in both modes; only sync prints it"
        );
        assert!(channel.post.posts.lock().unwrap().is_empty());
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
            Delivery::Rejected {
                status: 401,
                detail: "post FAILED HTTP 401".to_string(),
            },
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
            channel.deliver(&delivery_request(&event(), ReportMode::ReportOutcome)),
            expected,
            "case: {outcome:?}"
        );
    }
}

#[test]
fn only_the_four_established_http_statuses_are_terminal_failures() {
    for status in [
        199, 200, 299, 300, 400, 401, 402, 403, 404, 405, 408, 409, 412, 413, 414, 422, 429, 500,
        599,
    ] {
        let channel = channel_with_settings("key = \"key\"\n", PostOutcome::Status(status));
        let result = channel.deliver(&delivery_request(&event(), ReportMode::ReportOutcome));
        match status {
            200..=299 => assert!(matches!(result, Delivery::Delivered(_)), "status {status}"),
            401 | 403 | 404 | 413 => assert!(
                matches!(result, Delivery::Rejected { status: code, .. } if code == status),
                "status {status}"
            ),
            _ => assert!(matches!(result, Delivery::Failed(_)), "status {status}"),
        }
    }
}
