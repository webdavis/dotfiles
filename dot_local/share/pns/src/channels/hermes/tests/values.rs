use super::*;

// --- the body ------------------------------------------------------------

#[test]
fn the_body_carries_the_full_message_because_discord_has_no_ceiling() {
    let body = hermes_body(&event());
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["agent"], "claude");
    assert_eq!(parsed["state"], "done");
    assert_eq!(parsed["project"], "dotfiles");
    assert_eq!(parsed["detail"], "the full message");
    assert_eq!(parsed.as_object().unwrap().len(), 4);
}

// --- the signature -------------------------------------------------------

#[test]
fn the_signature_matches_the_published_hmac_sha256_vector() {
    // RFC-known vector: HMAC-SHA256("key", "The quick brown fox jumps
    // over the lazy dog").
    assert_eq!(
        sign("key", "The quick brown fox jumps over the lazy dog").as_deref(),
        Some("f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8")
    );
}

#[test]
fn an_empty_key_signs_nothing_which_is_the_not_set_up_case() {
    assert_eq!(sign("", "anything"), None);
}

// --- the sync voice ------------------------------------------------------

#[test]
fn sync_outcomes_are_spelled_exactly_as_the_bash_spells_them() {
    // Minus the `pns: ` prefix, which now belongs to the print site: the
    // PRINTED line is still byte for byte the bash's, and
    // `tests/native.rs` plus the dispatch suite pin that end of it.
    assert_eq!(outcome_line(PostOutcome::Status(200)), "posted HTTP 200");
    assert_eq!(outcome_line(PostOutcome::Status(204)), "posted HTTP 204");
    assert_eq!(
        outcome_line(PostOutcome::Status(404)),
        "post FAILED HTTP 404"
    );
    assert_eq!(
        outcome_line(PostOutcome::NoResponse),
        "post FAILED HTTP 000 (no response; is the hermes gateway up?)"
    );
}

#[test]
fn the_no_key_line_names_the_config_key_the_operator_must_fix() {
    assert_eq!(
        skipped_line(),
        "post SKIPPED -- no hermes key in the config ([plugins.hermes] key); nothing was sent"
    );
}

// --- the deadline --------------------------------------------------------

#[test]
fn the_sync_deadline_validates_and_defaults_to_five() {
    assert_eq!(remote_deadline(None), Some(Duration::from_secs(5)));
    assert_eq!(
        remote_deadline(Some("garbage")),
        Some(Duration::from_secs(5))
    );
    assert_eq!(remote_deadline(Some("012")), Some(Duration::from_secs(5)));
    assert_eq!(remote_deadline(Some("30")), Some(Duration::from_secs(30)));
}

#[test]
fn an_explicit_zero_deadline_is_no_deadline_like_curls_dash_m_zero() {
    assert_eq!(remote_deadline(Some("0")), None);
}

#[test]
fn an_absurd_deadline_clamps_to_a_day_instead_of_panicking_the_edge() {
    assert_eq!(
        remote_deadline(Some("9223372036854775807")),
        Some(Duration::from_secs(86_400))
    );
}

#[test]
fn a_redirect_is_the_final_answer_so_3xx_reads_failed() {
    assert_eq!(
        outcome_line(PostOutcome::Status(301)),
        "post FAILED HTTP 301"
    );
}

#[test]
fn the_never_attempted_case_has_its_own_bash_wording() {
    assert_eq!(
        outcome_line(PostOutcome::NoStatus),
        "post FAILED (curl reported no HTTP status at all)"
    );
}

#[test]
fn the_empty_and_unicode_bodies_match_openssls_own_hmac() {
    assert_eq!(
        sign("key", "").as_deref(),
        Some("5d5d139563c95b5967b9bd9a8c9b233a9dedb45072794cd232dc1b74832607d0")
    );
    assert_eq!(
        sign("key", "\u{17c}\u{f3}\u{142}\u{107} \u{fc}ber \u{1f6a8}").as_deref(),
        Some("2ce40f95a8377ebe61896f8eeb03cd9150b1ed7fbf16c47483ee58f86976d6c5")
    );
}

#[test]
fn the_key_never_rides_in_the_body_the_url_or_the_signature() {
    let channel = channel_with_settings("key = \"sekrit-key-9\"\n", PostOutcome::Status(200));
    channel.deliver(&event(), ReportMode::Silent);
    let posts = channel.post.posts.borrow();
    assert!(!posts[0].0.contains("sekrit-key-9"));
    assert!(!posts[0].1.contains("sekrit-key-9"));
    assert!(!posts[0].2.contains("sekrit-key-9"));
}

#[test]
fn the_default_url_is_the_local_gateway_route() {
    assert_eq!(DEFAULT_HERMES_URL, "http://127.0.0.1:8644/webhooks/pns");
}
