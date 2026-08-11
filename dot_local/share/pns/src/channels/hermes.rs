//! The hermes channel, native: the durable Discord log, one signed POST to
//! the local gateway.
//!
//! SYNC MODE EXISTS TO BE SEEN. The weekly job records must be able to see a
//! delivery failure: a 401 swallowed silently leaves the Discord channel
//! empty, and an empty channel looks like the jobs stopped running. So sync
//! posts print the HTTP status and the no-key case says so aloud, while
//! async stays silent like every other leg. The signing key never reaches
//! argv, a child environment, or any printed line; the signature is computed
//! in-process over the exact body bytes.

use super::{Channel, Event};
use crate::routing::Mode;
use std::path::PathBuf;
use std::time::Duration;

/// The gateway when `RELAY_HERMES_URL` says nothing: the local hermes
/// webhook route.
pub const DEFAULT_HERMES_URL: &str = "http://127.0.0.1:8644/webhooks/relay";

/// What one signed POST came back with: a status code, or no response at
/// all, which sync mode reports as its own distinct failure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PostOutcome {
    Status(u16),
    NoResponse,
}

/// The POST seam: body plus signature header in, outcome out. The production
/// impl honors the deadline; a fake records everything.
pub trait SignedPost {
    fn post(&self, url: &str, body: &str, signature_hex: &str, deadline: Duration) -> PostOutcome;
}

/// The gateway body: agent, state, project, and the FULL message as the
/// detail, because Discord has no length ceiling for the preview to serve.
pub fn hermes_body(event: &Event) -> String {
    serde_json::json!({
        "agent": event.agent,
        "state": event.state,
        "project": event.project,
        "detail": event.message,
    })
    .to_string()
}

/// The lowercase hex HMAC-SHA256 of the body under the signing key, or None
/// when the key is empty, which is the not-set-up case.
pub fn sign(secret: &str, body: &str) -> Option<String> {
    use hmac::{Hmac, KeyInit, Mac};
    if secret.is_empty() {
        return None;
    }
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(body.as_bytes());
    Some(
        mac.finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

/// The hermes signing key out of the auth JSON: `.hermes_secret`, non-empty,
/// else None. Silent, like every not-set-up reading.
pub fn hermes_secret(auth_json: &str) -> Option<String> {
    let key = serde_json::from_str::<serde_json::Value>(auth_json)
        .ok()?
        .get("hermes_secret")?
        .as_str()?
        .to_string();
    (!key.is_empty()).then_some(key)
}

/// The line sync mode prints for one outcome, exactly as the bash spells it.
pub fn outcome_line(outcome: PostOutcome) -> String {
    match outcome {
        PostOutcome::Status(code) if (200..300).contains(&code) => {
            format!("relay: posted HTTP {code}")
        }
        PostOutcome::Status(code) => format!("relay: post FAILED HTTP {code}"),
        PostOutcome::NoResponse => {
            "relay: post FAILED HTTP 000 (no response; is the hermes gateway up?)".to_string()
        }
    }
}

/// The line sync mode prints when there is no signing key.
pub fn skipped_line(auth_path: &std::path::Path) -> String {
    format!(
        "relay: post SKIPPED -- no hermes signing key in {}; nothing was sent",
        auth_path.display()
    )
}

/// The sync deadline: `RELAY_REMOTE_TIMEOUT` validated as a count, else 5
/// seconds, because a garbled deadline must not become zero or forever.
pub fn remote_deadline(env_value: Option<&str>) -> Duration {
    Duration::from_secs(env_value.and_then(crate::parse_count).unwrap_or(5))
}

/// The native hermes plugin.
pub struct HermesChannel<P: SignedPost> {
    pub post: P,
    /// `RELAY_AUTH_FILE` override, else `~/.config/relay/auth.json`.
    pub auth_path: PathBuf,
    /// `RELAY_HERMES_URL` override, else the default.
    pub url: String,
    /// The sync deadline, already validated at the edge.
    pub sync_deadline: Duration,
}

impl<P: SignedPost> Channel for HermesChannel<P> {
    fn deliver(&self, event: &Event, mode: Mode) {
        let body = hermes_body(event);
        let signature = std::fs::read_to_string(&self.auth_path)
            .ok()
            .as_deref()
            .and_then(hermes_secret)
            .and_then(|key| sign(&key, &body));
        let Some(signature) = signature else {
            // No key is unavailable, not a failure. Sync callers are told,
            // because an empty Discord channel looks like the jobs stopped.
            if mode == Mode::Sync {
                println!("{}", skipped_line(&self.auth_path));
            }
            return;
        };

        let deadline = match mode {
            Mode::Sync => self.sync_deadline,
            Mode::Async => Duration::from_secs(10),
        };
        let outcome = self.post.post(&self.url, &body, &signature, deadline);
        if mode == Mode::Sync {
            println!("{}", outcome_line(outcome));
        }
    }
}

/// The production POST: one agent, no redirects (following one would send the
/// signed body to whatever host the gateway names), the deadline per call.
/// An HTTP error status IS the answer sync mode prints, so a status-carrying
/// error is unwrapped rather than collapsed, matching the bash's missing -f.
pub struct UreqSignedPost;

impl SignedPost for UreqSignedPost {
    fn post(&self, url: &str, body: &str, signature_hex: &str, deadline: Duration) -> PostOutcome {
        let sent = ureq::Agent::config_builder()
            .timeout_global(Some(deadline))
            .max_redirects(0)
            .build()
            .new_agent()
            .post(url)
            .content_type("application/json")
            .header("X-Webhook-Signature", signature_hex)
            .send(body);
        match sent {
            Ok(response) => PostOutcome::Status(response.status().as_u16()),
            Err(ureq::Error::StatusCode(code)) => PostOutcome::Status(code),
            Err(_) => PostOutcome::NoResponse,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_HERMES_URL, HermesChannel, PostOutcome, SignedPost, hermes_body, hermes_secret,
        outcome_line, remote_deadline, sign, skipped_line,
    };
    use crate::channels::{Channel, Event};
    use crate::routing::Mode;
    use std::cell::RefCell;
    use std::time::Duration;

    struct RecordingPost {
        outcome: PostOutcome,
        posts: RefCell<Vec<(String, String, String, Duration)>>,
    }

    impl SignedPost for RecordingPost {
        fn post(
            &self,
            url: &str,
            body: &str,
            signature_hex: &str,
            deadline: Duration,
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

    fn channel_with_auth(
        auth: &str,
        outcome: PostOutcome,
    ) -> (HermesChannel<RecordingPost>, std::path::PathBuf) {
        // A process-wide counter, because two tests handing in the SAME auth
        // string must never share a path: parallel runs would race on the
        // cleanup and one test would read the other's deleted file.
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "pns-hermes-auth-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::write(&path, auth).unwrap();
        (
            HermesChannel {
                post: RecordingPost {
                    outcome,
                    posts: RefCell::new(Vec::new()),
                },
                auth_path: path.clone(),
                url: "http://127.0.0.1:9/test".to_string(),
                sync_deadline: Duration::from_secs(5),
            },
            path,
        )
    }

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

    #[test]
    fn every_way_the_auth_can_fail_to_provide_a_key_reads_not_set_up() {
        assert_eq!(hermes_secret(""), None);
        assert_eq!(hermes_secret("not json"), None);
        assert_eq!(hermes_secret(r#"{"moshi_secret":"x"}"#), None);
        assert_eq!(hermes_secret(r#"{"hermes_secret":""}"#), None);
    }

    // --- the sync voice ------------------------------------------------------

    #[test]
    fn sync_outcomes_are_spelled_exactly_as_the_bash_spells_them() {
        assert_eq!(
            outcome_line(PostOutcome::Status(200)),
            "relay: posted HTTP 200"
        );
        assert_eq!(
            outcome_line(PostOutcome::Status(204)),
            "relay: posted HTTP 204"
        );
        assert_eq!(
            outcome_line(PostOutcome::Status(404)),
            "relay: post FAILED HTTP 404"
        );
        assert_eq!(
            outcome_line(PostOutcome::NoResponse),
            "relay: post FAILED HTTP 000 (no response; is the hermes gateway up?)"
        );
    }

    #[test]
    fn the_no_key_line_names_the_auth_file_the_operator_must_fix() {
        assert_eq!(
            skipped_line(std::path::Path::new("/x/auth.json")),
            "relay: post SKIPPED -- no hermes signing key in /x/auth.json; nothing was sent"
        );
    }

    // --- the deadline --------------------------------------------------------

    #[test]
    fn the_sync_deadline_validates_and_defaults_to_five() {
        assert_eq!(remote_deadline(None), Duration::from_secs(5));
        assert_eq!(remote_deadline(Some("garbage")), Duration::from_secs(5));
        assert_eq!(remote_deadline(Some("012")), Duration::from_secs(5));
        assert_eq!(remote_deadline(Some("30")), Duration::from_secs(30));
    }

    // --- delivery ------------------------------------------------------------

    #[test]
    fn a_key_posts_once_with_the_signature_of_the_exact_body_bytes() {
        let (channel, path) =
            channel_with_auth(r#"{"hermes_secret":"key"}"#, PostOutcome::Status(200));
        channel.deliver(&event(), Mode::Async);
        std::fs::remove_file(&path).ok();
        let posts = channel.post.posts.borrow();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].0, "http://127.0.0.1:9/test");
        assert_eq!(
            Some(posts[0].2.as_str()),
            super::sign("key", &posts[0].1).as_deref(),
            "the signature covers the body that was actually posted"
        );
        assert_eq!(
            posts[0].3,
            Duration::from_secs(10),
            "async carries the ten second deadline"
        );
    }

    #[test]
    fn sync_carries_the_validated_sync_deadline() {
        let (channel, path) =
            channel_with_auth(r#"{"hermes_secret":"key"}"#, PostOutcome::Status(200));
        channel.deliver(&event(), Mode::Sync);
        std::fs::remove_file(&path).ok();
        assert_eq!(channel.post.posts.borrow()[0].3, Duration::from_secs(5));
    }

    #[test]
    fn no_key_means_no_post_in_either_mode() {
        for mode in [Mode::Async, Mode::Sync] {
            let (channel, path) = channel_with_auth(r#"{}"#, PostOutcome::Status(200));
            channel.deliver(&event(), mode);
            std::fs::remove_file(&path).ok();
            assert!(channel.post.posts.borrow().is_empty());
        }
    }

    #[test]
    fn the_default_url_is_the_local_gateway_route() {
        assert_eq!(DEFAULT_HERMES_URL, "http://127.0.0.1:8644/webhooks/relay");
    }
}
