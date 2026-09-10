use super::*;

#[test]
fn an_empty_key_signs_nothing_rather_than_signing_with_an_empty_secret() {
    // The not-set-up case. Signing under an empty secret would produce a
    // perfectly valid signature the gateway would reject, and the caller could
    // not tell that from a key that is simply wrong.
    assert!(sign("", "{}").is_none());
}

#[test]
fn a_signature_is_lowercase_hex_of_the_expected_length() {
    let signature = sign("secret", "{}").expect("a key was given");
    assert_eq!(signature.len(), 64, "sha256 is 32 bytes");
    assert!(
        signature
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character)),
        "{signature}"
    );
}

#[test]
fn the_signature_is_the_documented_hmac_sha256_vector() {
    // A KNOWN ANSWER, not a round trip. Both ends of this are independent
    // programs, so a test that only checked self-consistency would pass while
    // the gateway rejected every record.
    assert_eq!(
        sign("key", "The quick brown fox jumps over the lazy dog").as_deref(),
        Some("f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8")
    );
}

#[test]
fn the_body_changes_the_signature() {
    assert_ne!(sign("k", "a"), sign("k", "b"));
}

#[test]
fn the_key_changes_the_signature() {
    assert_ne!(sign("a", "body"), sign("b", "body"));
}

#[test]
fn every_two_hundred_status_is_delivered_and_nothing_either_side_of_it_is() {
    assert!(delivered(PostOutcome::Status(200)));
    assert!(delivered(PostOutcome::Status(299)));
    assert!(!delivered(PostOutcome::Status(199)));
    assert!(!delivered(PostOutcome::Status(300)));
    assert!(!delivered(PostOutcome::Status(500)));
}

#[test]
fn a_request_that_never_answered_is_not_delivered() {
    assert!(!delivered(PostOutcome::NoResponse));
    assert!(!delivered(PostOutcome::NoStatus));
}

#[test]
fn a_delivered_status_reads_as_posted_and_a_rejected_one_as_failed() {
    assert_eq!(outcome_line(PostOutcome::Status(202)), "posted HTTP 202");
    assert_eq!(
        outcome_line(PostOutcome::Status(401)),
        "post FAILED HTTP 401"
    );
}

#[test]
fn the_two_no_status_cases_read_differently_because_they_are_different_jobs() {
    // One is a config to fix and the other is a service to start, so a reader
    // who cannot tell them apart is sent to the wrong one half the time.
    let never_made = outcome_line(PostOutcome::NoStatus);
    let no_answer = outcome_line(PostOutcome::NoResponse);
    assert_ne!(never_made, no_answer);
    assert!(never_made.contains("[records] url"), "{never_made}");
    assert!(no_answer.contains("gateway"), "{no_answer}");
}

#[test]
fn no_sentence_here_names_a_tool_uu_does_not_run() {
    // These sentences were inherited from pns, which spoke of curl and named
    // its own config key. uu runs no curl and has no [plugins.hermes].
    for outcome in [
        PostOutcome::Status(500),
        PostOutcome::NoStatus,
        PostOutcome::NoResponse,
    ] {
        let line = outcome_line(outcome);
        assert!(!line.contains("curl"), "{line}");
        assert!(!line.contains("plugins.hermes"), "{line}");
    }
}
