use super::*;

// --- the body ------------------------------------------------------------

#[test]
fn the_body_carries_the_full_message_because_discord_has_no_ceiling() {
    let body = hermes_body(&event(), "original-42");
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["agent"], "claude");
    assert_eq!(parsed["state"], "done");
    assert_eq!(parsed["project"], "dotfiles");
    assert_eq!(parsed["detail"], "the full message");
    assert_eq!(parsed.as_object().unwrap().len(), 9);
}

#[test]
fn every_header_key_is_present_on_an_event_that_knows_no_session() {
    // THE GATEWAY'S RENDERER HAS NO CONDITIONALS: a `{key}` the posted body
    // omits renders as the literal text `{key}` in the channel
    // (`gateway/platforms/webhook.py`, read 2026-09-14), so a key pns has
    // nothing to say for still has to be there. `thread_id` is empty on
    // every post until hermes can hand back a thread it created.
    let body = hermes_body(&Event::default(), "original-42");
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    for key in ["header", "subheader", "body", "thread_id"] {
        assert!(parsed.get(key).is_some(), "{key} must be present");
    }
    assert_eq!(parsed["thread_id"], "");
    assert_eq!(parsed["body"], "");
    assert_eq!(parsed["header"], "done");
    assert_eq!(parsed["subheader"], "pns");
}

#[test]
fn the_two_composed_lines_ride_beside_the_bare_body() {
    let composed = Event {
        branch: "feat/sender-header".to_string(),
        state: "blocked".to_string(),
        session: "a1b2c3d4-dead-beef".to_string(),
        session_title: "arm posture alert".to_string(),
        detail: "Bash(git push) needs approval".to_string(),
        ..event()
    };
    let parsed: serde_json::Value =
        serde_json::from_str(&hermes_body(&composed, "original-42")).unwrap();
    assert_eq!(parsed["header"], "dotfiles · feat/sender-header · blocked");
    assert_eq!(parsed["subheader"], "claude · a1b2 · arm posture alert");
    assert_eq!(
        parsed["body"], "Bash(git push) needs approval",
        "the body is the BARE detail: the branch the message prefixes is in the header now"
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
fn the_key_never_rides_in_the_body_the_url_or_the_signature() {
    let channel = channel_with_settings("key = \"sekrit-key-9\"\n", PostOutcome::Status(200));
    channel.deliver(&delivery_request(&event(), ReportMode::Silent));
    let posts = channel.post.posts.lock().unwrap();
    assert!(!posts[0].0.contains("sekrit-key-9"));
    assert!(!posts[0].1.contains("sekrit-key-9"));
    assert!(!posts[0].2.contains("sekrit-key-9"));
}

#[test]
fn the_default_url_is_the_local_gateway_route() {
    assert_eq!(DEFAULT_HERMES_URL, "http://127.0.0.1:8644/webhooks/pns");
}
