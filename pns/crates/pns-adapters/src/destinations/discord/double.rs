//! The seam standing in for discord.com, and the one standing in for the
//! store: both test modules under this destination share them, because a
//! second double would be a second thing to keep honest.

use super::*;
use pns_domain::channel_map::ChannelMap;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

/// This deployment's own default route name. TEST-LOCAL, because no route
/// name is pns's to know: it is the map key an event with no project lands on
/// and the one route the lookup does not try ahead of the project, and these
/// cases only need a name both halves agree on.
pub(super) const DEFAULT_ROUTE: &str = "pns-events";

/// One request the seam was handed, kept whole so the composed
/// authorization, User-Agent and body are asserted without a socket.
pub(super) struct Sent {
    pub(super) url: String,
    pub(super) headers: Vec<(String, String)>,
    pub(super) body: String,
}

/// Answers what it was armed with, in order where a script was given, and
/// keeps every request it saw.
pub(super) struct Recorder {
    answer: DeliveryOutcome,
    scripted: Mutex<VecDeque<(DeliveryOutcome, String)>>,
    pub(super) seen: Mutex<Vec<Sent>>,
}

impl Recorder {
    pub(super) fn answering(answer: DeliveryOutcome) -> Self {
        Self {
            answer,
            scripted: Mutex::new(VecDeque::new()),
            seen: Mutex::new(Vec::new()),
        }
    }

    /// One answer per call, in order, falling back to a bare 200 once the
    /// script runs out.
    pub(super) fn scripted(answers: Vec<(DeliveryOutcome, String)>) -> Self {
        Self {
            answer: DeliveryOutcome::Status(200),
            scripted: Mutex::new(answers.into()),
            seen: Mutex::new(Vec::new()),
        }
    }
}

impl DiscordPost for Recorder {
    fn post(&self, request: &DiscordRequest) -> DiscordReply {
        self.seen.lock().unwrap().push(Sent {
            url: request.url.clone(),
            headers: request.headers.clone(),
            body: request.body.clone(),
        });
        match self.scripted.lock().unwrap().pop_front() {
            Some((outcome, body)) => DiscordReply { outcome, body },
            None => DiscordReply {
                outcome: self.answer,
                body: String::new(),
            },
        }
    }
}

/// An object carrying just the id Discord's own message and channel objects
/// are read for.
pub(super) fn object(id: &str) -> String {
    serde_json::json!({ "id": id }).to_string()
}

/// A refusal carrying Discord's own error code.
pub(super) fn refusal(code: u64) -> String {
    serde_json::json!({ "code": code, "message": "refused" }).to_string()
}

/// The stored pairs, shared with the test that armed the channel.
#[derive(Clone, Default)]
pub(super) struct Remembered(Arc<Mutex<HashMap<(String, String), String>>>);

impl Remembered {
    pub(super) fn rows(&self) -> Vec<((String, String), String)> {
        let mut rows: Vec<_> = self
            .0
            .lock()
            .unwrap()
            .iter()
            .map(|(pair, thread)| (pair.clone(), thread.clone()))
            .collect();
        rows.sort();
        rows
    }
}

impl SessionThreads for Remembered {
    fn thread(&self, session: &str, channel: &str) -> Option<String> {
        self.0
            .lock()
            .unwrap()
            .get(&(session.to_string(), channel.to_string()))
            .cloned()
    }

    fn remember(&self, session: &str, channel: &str, thread: &str) {
        self.0.lock().unwrap().insert(
            (session.to_string(), channel.to_string()),
            thread.to_string(),
        );
    }

    fn forget(&self, session: &str, channel: &str) {
        self.0
            .lock()
            .unwrap()
            .remove(&(session.to_string(), channel.to_string()));
    }
}

pub(super) const TOKEN: &str = "MTIzNDU2.a-secret-bot-token";

pub(super) fn event() -> Event {
    Event {
        agent: "claude".to_string(),
        state: "blocked".to_string(),
        project: "dotfiles".to_string(),
        branch: "feat/x".to_string(),
        detail: "Bash(git push) needs approval".to_string(),
        ..Event::default()
    }
}

pub(super) fn delivered_by(channel: &DiscordChannel<Recorder>) -> Delivery {
    delivered_about(channel, &event())
}

pub(super) fn delivered_about(channel: &DiscordChannel<Recorder>, event: &Event) -> Delivery {
    channel.deliver(&DeliveryRequest {
        producer: "pns",
        request_id: Some("req-1"),
        producer_request: None,
        event,
        route: "",
        mode: pns_domain::routing::ReportMode::ReportOutcome,
    })
}

pub(super) fn channels(entries: &[(&str, &str)]) -> ChannelMap {
    entries
        .iter()
        .map(|(key, channel)| ((*key).to_string(), (*channel).to_string()))
        .collect()
}

pub(super) fn armed(answer: DeliveryOutcome) -> DiscordChannel<Recorder> {
    armed_on("", channels(&[("default", "9001")]), answer)
}

pub(super) fn armed_on(
    route: &str,
    channels: ChannelMap,
    answer: DeliveryOutcome,
) -> DiscordChannel<Recorder> {
    holding(
        Recorder::answering(answer),
        route,
        channels,
        Remembered::default(),
    )
}

pub(super) fn holding(
    post: Recorder,
    route: &str,
    channels: ChannelMap,
    threads: Remembered,
) -> DiscordChannel<Recorder> {
    DiscordChannel {
        post,
        token: Some(TOKEN.to_string()),
        channels,
        route: route.to_string(),
        default_route: DEFAULT_ROUTE.to_string(),
        threads: Box::new(threads),
    }
}

/// The channel or thread one post was addressed to, read off the URL the
/// seam recorded.
pub(super) fn posted_to(channel: &DiscordChannel<Recorder>) -> String {
    addressed(channel, 0)
}

/// The channel or thread the nth post was addressed to.
///
/// A MESSAGE ENDPOINT ONLY: the id is the first path segment, but the whole
/// remainder must be `{id}/messages`, so a thread-creation URL (whose id is
/// followed by `/messages/{message_id}/threads`) panics here instead of
/// silently answering the right-looking id off the wrong endpoint.
pub(super) fn addressed(channel: &DiscordChannel<Recorder>, nth: usize) -> String {
    let seen = channel.post.seen.lock().unwrap();
    let sent = seen.get(nth).expect("a post went out");
    let rest = sent
        .url
        .strip_prefix(&format!("{}/channels/", super::request::API_BASE))
        .expect("a channel URL");
    let id = rest.split('/').next().expect("a channel URL");
    assert_eq!(rest, format!("{id}/messages"), "a message endpoint");
    id.to_string()
}
