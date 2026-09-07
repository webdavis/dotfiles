use super::{
    DEFAULT_HERMES_URL, HermesChannel, PostOutcome, SignedPost, hermes_body, hermes_secret,
    outcome_line, remote_deadline, sign, skipped_line,
};
use crate::channels::{Delivery, Event};
use crate::routing::ReportMode;
use std::cell::RefCell;
use std::time::Duration;

/// url, body, signature, deadline: one recorded post.
type RecordedPost = (String, String, String, Option<Duration>);

struct RecordingPost {
    outcome: PostOutcome,
    posts: RefCell<Vec<RecordedPost>>,
}

impl SignedPost for RecordingPost {
    fn post(
        &self,
        url: &str,
        body: &str,
        signature_hex: &str,
        deadline: Option<Duration>,
    ) -> PostOutcome {
        self.posts.borrow_mut().push((
            url.to_string(),
            body.to_string(),
            signature_hex.to_string(),
            deadline,
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
            posts: RefCell::new(Vec::new()),
        },
        key: hermes_secret(&settings.parse().unwrap()),
        url: "http://127.0.0.1:9/test".to_string(),
        sync_deadline: Some(Duration::from_secs(5)),
    }
}

// --- the production post, against real sockets ---------------------------

use super::UreqSignedPost;

mod posting;
mod values;
