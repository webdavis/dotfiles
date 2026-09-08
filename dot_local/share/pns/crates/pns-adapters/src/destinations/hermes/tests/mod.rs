use super::{
    DEFAULT_HERMES_URL, HermesChannel, SignedPost, hermes_body, remote_deadline, sign, skipped_line,
};
use crate::destinations::{Delivery, Event};
use crate::hermes_secret;
use pns_application::{DeliveryRequest, NotificationDestination};
use pns_domain::routing::ReportMode;
use std::sync::Mutex;
use std::time::Duration;

/// url, body, signature, deadline: one recorded post.
type RecordedPost = (String, String, String, Option<Duration>, Option<String>);

struct RecordingPost {
    outcome: PostOutcome,
    posts: Mutex<Vec<RecordedPost>>,
}

impl SignedPost for RecordingPost {
    fn post(
        &self,
        url: &str,
        body: &str,
        signature_hex: &str,
        idempotency_key: Option<&str>,
        deadline: Option<Duration>,
    ) -> PostOutcome {
        self.posts.lock().unwrap().push((
            url.to_string(),
            body.to_string(),
            signature_hex.to_string(),
            deadline,
            idempotency_key.map(str::to_owned),
        ));
        self.outcome
    }
}

fn event() -> Event {
    Event {
        agent: "claude".to_string(),
        state: "done".to_string(),
        project: "dotfiles".to_string(),
        message: "the full message".to_string(),
        preview: "a preview".to_string(),
        ..Event::default()
    }
}

/// The channel as the composition root builds it: the key already
/// extracted from the `[plugins.hermes]` settings.
fn channel_with_settings(settings: &str, outcome: PostOutcome) -> HermesChannel<RecordingPost> {
    HermesChannel {
        post: RecordingPost {
            outcome,
            posts: Mutex::new(Vec::new()),
        },
        key: hermes_secret(&settings.parse().unwrap()),
        url: "http://127.0.0.1:9/test".to_string(),
        sync_deadline: Some(Duration::from_secs(5)),
    }
}

// --- the production post, against real sockets ---------------------------

use pns_hermes::PostOutcome;

mod posting;
mod values;

mod routes;

mod request;

fn delivery_request(event: &Event, mode: ReportMode) -> DeliveryRequest<'_> {
    DeliveryRequest {
        producer: "test",
        request_id: Some("original-42"),
        event,
        route: "",
        mode,
    }
}
