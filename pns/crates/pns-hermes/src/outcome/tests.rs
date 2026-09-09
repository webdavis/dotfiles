use crate::{PostOutcome, outcome_line, sign, skipped_line};

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
        "post SKIPPED, no hermes key in the config ([plugins.hermes] key); nothing was sent"
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
