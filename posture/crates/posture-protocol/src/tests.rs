//! Every coercion here was captured by RUNNING the jq this replaces against a
//! fixture spool, not read off its source.

use super::*;

fn record(detector: &str, identity: &str) -> DigestRecord {
    DigestRecord {
        timestamp: Some("2026-09-09T01:00:00Z".into()),
        detector: Some(detector.into()),
        category: Some("persistence".into()),
        identity: Some(identity.into()),
        action: Some("added".into()),
        summary: Some(format!("{detector} {identity}")),
    }
}

#[test]
fn a_record_encodes_to_one_line_with_its_six_fields_in_declared_order() {
    assert_eq!(
        encode(&record("persistence_launchd", "com.evil.agent")),
        r#"{"timestamp":"2026-09-09T01:00:00Z","detector":"persistence_launchd","category":"persistence","identity":"com.evil.agent","action":"added","summary":"persistence_launchd com.evil.agent"}"#
    );
}

#[test]
fn a_record_survives_the_round_trip_it_was_written_for() {
    let original = record("persistence_launchd", "com.evil.agent");
    assert_eq!(decode_spool(&encode(&original)), vec![original]);
}

#[test]
fn a_field_the_record_does_not_carry_encodes_as_null_and_decodes_back_to_nothing() {
    let sparse = DigestRecord {
        detector: Some("one".into()),
        ..DigestRecord::default()
    };
    let line = encode(&sparse);
    assert!(line.contains(r#""timestamp":null"#), "{line}");
    assert_eq!(decode_spool(&line), vec![sparse]);
}

#[test]
fn every_line_of_a_spool_becomes_its_own_record() {
    let spool = format!(
        "{}\n{}\n",
        encode(&record("alpha", "one")),
        encode(&record("beta", "two"))
    );
    let decoded = decode_spool(&spool);
    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded[0].detector.as_deref(), Some("alpha"));
    assert_eq!(decoded[1].detector.as_deref(), Some("beta"));
}

#[test]
fn a_torn_line_costs_one_finding_and_never_the_batch() {
    // The alerter can be killed mid-append at any moment, so a half-written
    // line is a normal event rather than a corrupt spool.
    let spool = format!(
        "{}\n{{\"detector\":\"tor\n{}\n",
        encode(&record("alpha", "one")),
        encode(&record("beta", "two"))
    );
    let decoded = decode_spool(&spool);
    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded[1].detector.as_deref(), Some("beta"));
}

#[test]
fn a_valid_json_line_that_is_not_an_object_costs_one_finding_and_never_the_batch() {
    // THE ONE PLACE THIS DIFFERS FROM THE RENDERER IT REPLACES. There, a bare
    // `42` reached group_by as a scalar, jq errored, and the whole day's digest
    // rendered empty and was never sent.
    let spool = format!("{}\n42\n[1,2]\n\"bare\"\n", encode(&record("alpha", "one")));
    let decoded = decode_spool(&spool);
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].detector.as_deref(), Some("alpha"));
}

#[test]
fn blank_and_whitespace_only_lines_are_not_records() {
    let spool = format!("\n   \n\t\n{}\n\n", encode(&record("alpha", "one")));
    assert_eq!(decode_spool(&spool).len(), 1);
}

#[test]
fn an_empty_spool_holds_no_records() {
    assert!(decode_spool("").is_empty());
    assert!(decode_spool("\n\n").is_empty());
}

#[test]
fn a_number_written_where_a_string_belonged_becomes_what_it_says() {
    // Captured: jq rendered `42` and `4.5` for these, so the finding is still
    // reported rather than dropped for a wrong-typed field.
    let decoded = decode_spool(r#"{"detector":"one","identity":42,"summary":4.5}"#);
    assert_eq!(decoded[0].identity.as_deref(), Some("42"));
    assert_eq!(decoded[0].summary.as_deref(), Some("4.5"));
}

#[test]
fn a_true_becomes_its_text_and_a_false_counts_as_not_carried() {
    // Captured: jq rendered `true` for the first and `?` for the second,
    // because its `//` fallback takes false as well as null. Nothing writes a
    // boolean field, so the quirk is preserved rather than quietly corrected.
    let decoded = decode_spool(r#"{"detector":"one","identity":true,"summary":false}"#);
    assert_eq!(decoded[0].identity.as_deref(), Some("true"));
    assert_eq!(decoded[0].summary, None);
}

#[test]
fn an_empty_string_is_a_value_and_not_an_absence() {
    // Captured: jq rendered an empty span rather than `?`, because an empty
    // string is truthy there.
    let decoded = decode_spool(r#"{"detector":"one","identity":"","summary":"s"}"#);
    assert_eq!(decoded[0].identity.as_deref(), Some(""));
}

#[test]
fn a_composite_written_where_a_string_belonged_becomes_its_compact_json() {
    // Captured: jq's tostring rendered `[1,2]`.
    let decoded = decode_spool(r#"{"detector":"one","identity":"i","summary":[1,2]}"#);
    assert_eq!(decoded[0].summary.as_deref(), Some("[1,2]"));
}

#[test]
fn a_field_the_line_never_mentions_is_not_carried() {
    let decoded = decode_spool(r#"{"detector":"one"}"#);
    assert_eq!(decoded[0].detector.as_deref(), Some("one"));
    assert_eq!(decoded[0].identity, None);
    assert_eq!(decoded[0].timestamp, None);
}

#[test]
fn a_field_the_record_does_not_know_is_ignored_rather_than_refused() {
    // A line an older or newer writer produced still yields its six fields.
    // There is no version to check, so tolerating the extra IS the
    // compatibility story.
    let decoded = decode_spool(r#"{"detector":"one","identity":"i","sha256":"deadbeef"}"#);
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].identity.as_deref(), Some("i"));
}

#[test]
fn a_line_carrying_a_newline_in_a_value_is_still_one_record() {
    // JSON escapes it, so the spool's one-line-per-record rule holds even when
    // a path an attacker chose contains a newline.
    let crafted = DigestRecord {
        identity: Some("evil\nline".into()),
        ..record("one", "unused")
    };
    let line = encode(&crafted);
    assert_eq!(line.lines().count(), 1, "{line}");
    assert_eq!(decode_spool(&line), vec![crafted]);
}
