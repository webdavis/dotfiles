use super::double::*;
use super::request::API_BASE;
use super::*;
use pns_domain::channel_map::ChannelMap;
use pns_domain::retry::{DeadletterReason, FailureClass, RetryLimits};

#[test]
fn a_table_with_no_token_refuses_by_name_and_posts_nothing() {
    // THE MUTANT THIS PINS: the not-set-up guard dropped, so an unarmed
    // channel posts `Bot ` and earns a 401 on every event instead of one
    // sentence naming the key to write.
    for (channel, key) in [
        (
            DiscordChannel {
                post: Recorder::answering(TransportOutcome::Status(200)),
                token: None,
                channels: channels(&[("default", "9001")]),
                route: String::new(),
                default_route: DEFAULT_ROUTE.to_string(),
                threads: Box::new(Remembered::default()),
            },
            "[plugins.log] bot_token",
        ),
        (
            armed_on("", ChannelMap::new(), TransportOutcome::Status(200)),
            "[plugins.log.channels] default",
        ),
    ] {
        let Delivery::Failed(line) = delivered_by(&channel) else {
            panic!("an unarmed channel fails rather than delivering");
        };
        assert!(line.contains(key), "names the key to write: {line}");
        assert!(
            channel.post.seen.lock().unwrap().is_empty(),
            "nothing was sent"
        );
    }
}

#[test]
fn a_401_dead_letters_on_its_first_attempt_while_429_and_5xx_stay_retryable() {
    // THE WHOLE RETRY POLICY, read off the shared rule rather than restated
    // here: a refusal will not fix itself, and a rate limit or a bad gateway
    // will. A destination that classified for itself is exactly what this
    // engine keeps out of the transports.
    let limits = RetryLimits::default();
    for (status, permanent) in [
        (401, true),
        (403, true),
        (404, true),
        (429, false),
        (500, false),
        (502, false),
    ] {
        let channel = armed(TransportOutcome::Status(status));
        let Delivery::Rejected { status: seen, .. } = delivered_by(&channel) else {
            panic!("a refused post is Rejected with its status, not a bare failure");
        };
        assert_eq!(seen, status);
        let class = TransportOutcome::Status(seen).class();
        assert_eq!(class.is_permanent(), permanent, "status {status}");
        assert_eq!(
            limits.verdict(TransportOutcome::Status(seen), 0, 0, 0)
                == Some(DeadletterReason::Permanent),
            permanent,
            "status {status} on its first attempt"
        );
    }
    // And a dead network is the destination's to report and the ledger's to
    // keep retrying.
    assert_eq!(
        TransportOutcome::NoResponse.class(),
        FailureClass::Temporary
    );
}

#[test]
fn the_token_appears_in_no_rendered_line_whatever_happened() {
    // THE MUTANT THIS PINS: a failure line built from the request rather than
    // the status, which is how a credential reaches a log and a failure page.
    for answer in [
        TransportOutcome::Status(200),
        TransportOutcome::Status(401),
        TransportOutcome::Status(429),
        TransportOutcome::NoResponse,
        TransportOutcome::NoStatus,
    ] {
        let channel = armed(answer);
        let line = match delivered_by(&channel) {
            Delivery::Delivered(line) | Delivery::Failed(line) => line,
            Delivery::Rejected { detail, .. } => detail,
            other => panic!("unexpected verdict: {other:?}"),
        };
        assert!(!line.contains(TOKEN), "the token rode a line out: {line}");
        assert!(!line.contains("Bot "), "nor the header shape: {line}");
    }
}

#[test]
fn the_composed_request_carries_the_bot_authorization_the_user_agent_and_no_mentions() {
    let channel = armed(TransportOutcome::Status(200));
    delivered_by(&channel);
    let seen = channel.post.seen.lock().unwrap();
    let sent = seen.first().expect("one post went out");
    assert_eq!(sent.url, format!("{API_BASE}/channels/9001/messages"));
    let header = |name: &str| {
        sent.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
            .unwrap_or_else(|| panic!("no {name} header"))
    };
    assert_eq!(header("Authorization"), format!("Bot {TOKEN}"));
    assert_eq!(header("Content-Type"), "application/json");
    // MANDATORY: Discord blocks a call without a valid one, and the form is
    // theirs, so both halves are pinned rather than merely non-empty.
    let agent = header("User-Agent");
    assert!(agent.starts_with("DiscordBot ("), "{agent}");
    assert!(agent.contains("github.com/webdavis/dotfiles"), "{agent}");
    let parsed: serde_json::Value = serde_json::from_str(&sent.body).expect("the body is JSON");
    // AN AGENT QUOTING `@everyone` MUST NOT PAGE THE GUILD, which the default
    // for a regular message would do.
    assert_eq!(parsed["allowed_mentions"]["parse"], serde_json::json!([]));
    let content = parsed["content"].as_str().expect("a content string");
    assert!(content.starts_with("**dotfiles"), "{content}");
    assert!(
        content.contains("Bash(git push) needs approval"),
        "{content}"
    );
}

#[test]
fn a_body_past_the_budget_sheds_whole_lines_from_the_end_and_says_how_many() {
    // THE MUTANT THIS PINS: no budget at all, so one long stack trace earns a
    // permanent 400 from Discord and dead-letters the event.
    let mut event = event();
    event.detail = (0..500)
        .map(|line| format!("line {line} of a very long paste"))
        .collect::<Vec<_>>()
        .join("\n");
    let content = content(&event);
    assert!(content.chars().count() <= MAX_CONTENT_CHARS, "{content}");
    assert!(
        content.starts_with("**dotfiles"),
        "the header is never shed"
    );
    assert!(content.contains("more lines dropped)"), "{content}");
}

#[test]
fn a_single_oversized_line_is_clipped_to_its_start_and_the_note_is_singular() {
    // THE MUTANT THIS PINS: the whole oversized line dropped rather than
    // clipped, which turns one giant paste into a bare header and subheader
    // with none of the paste's own start visible, and a note that reads "1
    // more lines dropped" instead of "1 more line dropped".
    let mut event = event();
    event.detail = "x".repeat(5_000);
    let content = content(&event);
    assert!(content.chars().count() <= MAX_CONTENT_CHARS, "{content}");
    assert!(
        content.contains("xxxxx"),
        "the paste's start survives: {content}"
    );
    assert!(content.contains("(1 more line dropped)"), "{content}");
}

#[test]
fn the_event_picks_its_channel_and_the_route_picks_it_first() {
    // THE MUTANT THIS PINS: one channel for every event, which is the whole
    // map ignored, and a critical page posted to whichever channel the
    // catch-all names.
    let map = channels(&[
        ("default", "catch-all"),
        ("pns-events", "engine"),
        ("priority", "pages"),
        ("dotfiles", "dotfiles-dev"),
    ]);
    let mut nothing_mapped = event();
    nothing_mapped.project = "netpulse".to_string();
    let mut no_project = event();
    no_project.project = String::new();
    for (route, event, expected) in [
        ("", event(), "dotfiles-dev"),
        ("priority", event(), "pages"),
        ("", nothing_mapped, "catch-all"),
        ("", no_project, "engine"),
    ] {
        let channel = armed_on(route, map.clone(), TransportOutcome::Status(200));
        assert!(
            matches!(delivered_about(&channel, &event), Delivery::Delivered(_)),
            "route {route:?} delivered"
        );
        assert_eq!(posted_to(&channel), expected, "route {route:?}");
    }
}

#[test]
fn a_github_event_and_a_session_event_about_one_repository_reach_one_channel() {
    // THE MUTANT THIS PINS: a second map, or a second lookup order, for the
    // source that spells a repository `owner/name`. This is why
    // `[plugins.github.channels]` is deleted rather than filled in: GitHub
    // only ever knows `webdavis/dotfiles` and a session only ever knows
    // `dotfiles`, and both are the same repository, so both are one channel.
    let map = channels(&[("default", "catch-all"), ("dotfiles", "dotfiles-dev")]);
    let mut from_github = event();
    from_github.project = "webdavis/dotfiles".to_string();
    from_github.branch = "lint".to_string();
    from_github.state = "failed".to_string();
    for subject in [event(), from_github] {
        let channel = armed_on("", map.clone(), TransportOutcome::Status(200));
        assert!(
            matches!(delivered_about(&channel, &subject), Delivery::Delivered(_)),
            "project {:?} delivered",
            subject.project
        );
        assert_eq!(
            posted_to(&channel),
            "dotfiles-dev",
            "project {:?}",
            subject.project
        );
    }
}
