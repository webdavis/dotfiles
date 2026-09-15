use super::request::API_BASE;
use super::*;
use pns_domain::retry::{DeadletterReason, FailureClass, RetryLimits};
use std::sync::Mutex;

/// The seam standing in for discord.com: it answers whatever it was built
/// with and keeps the request it was handed, which is how the composed
/// authorization, User-Agent and body are asserted without a socket.
struct Sent {
    url: String,
    headers: Vec<(String, String)>,
    body: String,
}

struct Recorder {
    answer: DeliveryOutcome,
    seen: Mutex<Vec<Sent>>,
}

impl Recorder {
    fn answering(answer: DeliveryOutcome) -> Self {
        Self {
            answer,
            seen: Mutex::new(Vec::new()),
        }
    }
}

impl DiscordPost for Recorder {
    fn post(&self, request: &DiscordRequest) -> DeliveryOutcome {
        self.seen.lock().unwrap().push(Sent {
            url: request.url.clone(),
            headers: request.headers.clone(),
            body: request.body.clone(),
        });
        self.answer
    }
}

const TOKEN: &str = "MTIzNDU2.a-secret-bot-token";

fn event() -> Event {
    Event {
        agent: "claude".to_string(),
        state: "blocked".to_string(),
        project: "dotfiles".to_string(),
        branch: "feat/x".to_string(),
        detail: "Bash(git push) needs approval".to_string(),
        ..Event::default()
    }
}

fn delivered_by(channel: &DiscordChannel<Recorder>) -> Delivery {
    let event = event();
    channel.deliver(&DeliveryRequest {
        producer: "pns",
        request_id: Some("req-1"),
        producer_request: None,
        event: &event,
        route: "",
        mode: pns_domain::routing::ReportMode::ReportOutcome,
    })
}

fn armed(answer: DeliveryOutcome) -> DiscordChannel<Recorder> {
    DiscordChannel {
        post: Recorder::answering(answer),
        token: Some(TOKEN.to_string()),
        channel_id: Some("9001".to_string()),
    }
}

#[test]
fn a_table_with_no_token_refuses_by_name_and_posts_nothing() {
    // THE MUTANT THIS PINS: the not-set-up guard dropped, so an unarmed
    // channel posts `Bot ` and earns a 401 on every event instead of one
    // sentence naming the key to write.
    for (channel, key) in [
        (
            DiscordChannel {
                post: Recorder::answering(DeliveryOutcome::Status(200)),
                token: None,
                channel_id: Some("9001".to_string()),
            },
            "[plugins.discord] token",
        ),
        (
            DiscordChannel {
                post: Recorder::answering(DeliveryOutcome::Status(200)),
                token: Some(TOKEN.to_string()),
                channel_id: None,
            },
            "[plugins.discord.channels] default",
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
        let channel = armed(DeliveryOutcome::Status(status));
        let Delivery::Rejected { status: seen, .. } = delivered_by(&channel) else {
            panic!("a refused post is Rejected with its status, not a bare failure");
        };
        assert_eq!(seen, status);
        let class = DeliveryOutcome::Status(seen).class();
        assert_eq!(class.is_permanent(), permanent, "status {status}");
        assert_eq!(
            limits.verdict(DeliveryOutcome::Status(seen), 0, 0, 0)
                == Some(DeadletterReason::Permanent),
            permanent,
            "status {status} on its first attempt"
        );
    }
    // And a dead network is the destination's to report and the ledger's to
    // keep retrying.
    assert_eq!(DeliveryOutcome::NoResponse.class(), FailureClass::Temporary);
}

#[test]
fn the_token_appears_in_no_rendered_line_whatever_happened() {
    // THE MUTANT THIS PINS: a failure line built from the request rather than
    // the status, which is how a credential reaches a log and a failure page.
    for answer in [
        DeliveryOutcome::Status(200),
        DeliveryOutcome::Status(401),
        DeliveryOutcome::Status(429),
        DeliveryOutcome::NoResponse,
        DeliveryOutcome::NoStatus,
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
    let channel = armed(DeliveryOutcome::Status(200));
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
