use super::*;

/// The documented shape of one listing entry, fields trimmed to the ones this
/// reads. Every value is invented; no live call was made to produce it.
const LISTING: &str = r#"[
  {
    "id": "20111",
    "unread": true,
    "reason": "ci_activity",
    "updated_at": "2026-09-14T15:16:27Z",
    "subject": {
      "title": "lint",
      "url": "https://api.github.com/repos/webdavis/dotfiles/check-suites/4471",
      "latest_comment_url": null,
      "type": "CheckSuite"
    },
    "repository": {
      "id": 1296269,
      "full_name": "webdavis/dotfiles",
      "html_url": "https://github.com/webdavis/dotfiles",
      "private": true
    }
  },
  {
    "id": "20112",
    "reason": "review_requested",
    "updated_at": "2026-09-14T15:10:00Z",
    "subject": {
      "title": "feat: the poll",
      "url": "https://api.github.com/repos/webdavis/dotfiles/pulls/689",
      "type": "PullRequest"
    },
    "repository": { "full_name": "webdavis/dotfiles", "html_url": "https://github.com/webdavis/dotfiles" }
  }
]"#;

#[test]
fn a_listing_carries_every_thread_in_the_order_it_stated_them() {
    let threads = notification_threads(LISTING);
    assert_eq!(threads.len(), 2);
    assert_eq!(threads[0].id, "20111");
    assert_eq!(threads[0].reason, "ci_activity");
    assert_eq!(threads[0].subject_type, "CheckSuite");
    assert_eq!(threads[0].subject_title, "lint");
    assert_eq!(
        threads[0].subject_url,
        "https://api.github.com/repos/webdavis/dotfiles/check-suites/4471"
    );
    assert_eq!(threads[0].repo_full_name, "webdavis/dotfiles");
    assert_eq!(
        threads[0].repo_html_url,
        "https://github.com/webdavis/dotfiles"
    );
    assert_eq!(threads[0].updated_at, 1_789_398_987);
    assert_eq!(
        threads[1].id, "20112",
        "newest first, as the API states them"
    );
}

#[test]
fn a_null_subject_url_is_an_empty_field_rather_than_a_dropped_thread() {
    // A `CheckSuite` has historically carried a null `subject.url`, which the
    // policy answers with a fallback identity: losing the whole thread here
    // would make that fallback unreachable.
    let threads = notification_threads(
        r#"[{"id":"1","reason":"ci_activity","updated_at":"2026-09-14T15:16:27Z",
             "subject":{"title":"lint","url":null,"type":"CheckSuite"},
             "repository":{"full_name":"a/b","html_url":"https://github.com/a/b"}}]"#,
    );
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].subject_url, "");
}

#[test]
fn an_entry_with_no_id_is_skipped_and_the_rest_of_the_listing_survives() {
    // THE MUTANT THIS PINS: one malformed entry failing the whole listing,
    // which would cost the other nineteen notifications.
    let threads = notification_threads(
        r#"[{"reason":"ci_activity"},
            {"id":"2","reason":"mention","updated_at":"2026-09-14T15:16:27Z",
             "subject":{"title":"t","url":"","type":"Issue"},
             "repository":{"full_name":"a/b","html_url":"https://github.com/a/b"}}]"#,
    );
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].id, "2");
}

#[test]
fn a_numeric_id_reads_as_its_digits() {
    let threads = notification_threads(r#"[{"id":20111}]"#);
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].id, "20111");
}

#[test]
fn a_body_that_is_not_a_listing_is_no_threads_rather_than_a_panic() {
    for body in [
        "",
        "not json",
        "{}",
        "null",
        r#"{"message":"Bad credentials"}"#,
        "[1,2]",
    ] {
        assert!(notification_threads(body).is_empty(), "case {body:?}");
    }
}

// --- the instant, which is the inverse of the clock adapter's own ----------

#[test]
fn an_instant_reads_as_the_epoch_second_it_names() {
    for (instant, epoch) in [
        ("1970-01-01T00:00:00Z", 0),
        ("1970-01-01T00:00:01Z", 1),
        ("2000-03-01T00:00:00Z", 951_868_800),
        ("2026-09-14T15:16:27Z", 1_789_398_987),
        ("2024-02-29T12:00:00Z", 1_709_208_000),
        ("2038-01-19T03:14:08Z", 2_147_483_648),
    ] {
        assert_eq!(epoch_from_instant(instant), Some(epoch), "case {instant}");
    }
}

#[test]
fn what_the_clock_adapter_renders_is_what_this_reads_back() {
    // THE ROUND TRIP IS THE PROPERTY, against the one function in this crate
    // that writes this format. A drift in either direction is red here.
    for epoch in [
        0_u64,
        1,
        951_868_800,
        1_709_208_000,
        1_789_398_987,
        4_102_444_800,
    ] {
        let rendered = crate::utc_timestamp(epoch).expect("the clock renders it");
        assert_eq!(epoch_from_instant(&rendered), Some(epoch), "case {epoch}");
    }
}

#[test]
fn anything_that_is_not_that_one_shape_is_no_instant_at_all() {
    // STRICT RATHER THAN LENIENT: a format change has to read as a zero the
    // caller can see, never as a plausible wrong instant.
    for instant in [
        "",
        "2026-09-14T15:16:27",
        "2026-09-14T15:16:27+02:00",
        "2026-09-14 15:16:27Z",
        "2026-09-14T15:16Z",
        "2026-09-14T15:16:27.500Z",
        "2026-9-14T15:16:27Z",
        "2026-13-01T00:00:00Z",
        "2026-00-01T00:00:00Z",
        "2026-01-00T00:00:00Z",
        "2026-01-01T24:00:00Z",
        "2026-01-01T00:60:00Z",
        "1969-12-31T23:59:59Z",
        "not-a-dateT00:00:00Z",
    ] {
        assert_eq!(epoch_from_instant(instant), None, "case {instant:?}");
    }
}
