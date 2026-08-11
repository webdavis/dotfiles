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
    /// A request that went out and got no HTTP status back: curl's 000.
    NoResponse,
    /// A request that could not even be attempted (a malformed URL): curl's
    /// empty status, reported with its own wording.
    NoStatus,
}

/// The POST seam: body plus signature header in, outcome out. The production
/// impl honors the deadline; a fake records everything.
pub trait SignedPost {
    /// `deadline` None means NO deadline, curl's `-m 0`: explicit caller
    /// intent, not a default.
    fn post(
        &self,
        url: &str,
        body: &str,
        signature_hex: &str,
        deadline: Option<Duration>,
    ) -> PostOutcome;
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
        PostOutcome::NoStatus => todo!("R2g fix round: the empty-status wording"),
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
pub fn remote_deadline(env_value: Option<&str>) -> Option<Duration> {
    Some({ Duration::from_secs(env_value.and_then(crate::parse_count).unwrap_or(5)) })
}

/// The native hermes plugin.
pub struct HermesChannel<P: SignedPost> {
    pub post: P,
    /// `RELAY_AUTH_FILE` override, else `~/.config/relay/auth.json`.
    pub auth_path: PathBuf,
    /// `RELAY_HERMES_URL` override, else the default.
    pub url: String,
    /// The sync deadline, already validated at the edge; None is curl's
    /// explicit no-deadline.
    pub sync_deadline: Option<Duration>,
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
            Mode::Async => Some(Duration::from_secs(10)),
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
    fn post(
        &self,
        url: &str,
        body: &str,
        signature_hex: &str,
        deadline: Option<Duration>,
    ) -> PostOutcome {
        // Red-state shim: the fix round teaches this the None (no deadline)
        // and clamp semantics.
        let deadline = deadline.unwrap_or(Duration::from_secs(10));
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
        posts: RefCell<Vec<(String, String, String, Option<Duration>)>>,
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
                sync_deadline: Some(Duration::from_secs(5)),
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
    fn a_redirect_is_the_final_answer_so_3xx_reads_failed() {
        assert_eq!(
            outcome_line(PostOutcome::Status(301)),
            "relay: post FAILED HTTP 301"
        );
    }

    #[test]
    fn the_never_attempted_case_has_its_own_bash_wording() {
        assert_eq!(
            outcome_line(PostOutcome::NoStatus),
            "relay: post FAILED (curl reported no HTTP status at all)"
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

    #[test]
    fn the_key_never_rides_in_the_body_the_url_or_the_signature() {
        let (channel, path) = channel_with_auth(
            r#"{"hermes_secret":"sekrit-key-9"}"#,
            PostOutcome::Status(200),
        );
        channel.deliver(&event(), Mode::Async);
        std::fs::remove_file(&path).ok();
        let posts = channel.post.posts.borrow();
        assert!(!posts[0].0.contains("sekrit-key-9"));
        assert!(!posts[0].1.contains("sekrit-key-9"));
        assert!(!posts[0].2.contains("sekrit-key-9"));
    }

    // --- the production post, against real sockets ---------------------------

    use super::UreqSignedPost;

    #[test]
    fn a_malformed_url_is_never_attempted_which_is_its_own_outcome() {
        assert_eq!(
            UreqSignedPost.post("http://[::1", "{}", "sig", Some(Duration::from_secs(2))),
            PostOutcome::NoStatus
        );
    }

    #[test]
    fn a_closed_port_is_no_response() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/hook", listener.local_addr().unwrap());
        drop(listener);
        assert_eq!(
            UreqSignedPost.post(&url, "{}", "sig", Some(Duration::from_secs(2))),
            PostOutcome::NoResponse
        );
    }

    #[test]
    fn a_redirecting_gateway_is_the_final_answer_and_the_signed_body_stays_home() {
        use std::io::{Read, Write};
        let decoy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let decoy_addr = decoy.local_addr().unwrap();
        decoy.set_nonblocking(true).unwrap();
        let redirector = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/hook", redirector.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = redirector.accept() {
                let mut request = [0u8; 2048];
                let _ = stream.read(&mut request);
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{decoy_addr}/\r\nContent-Length: 0\r\n\r\n"
                    )
                    .as_bytes(),
                );
            }
        });
        let outcome = UreqSignedPost.post(
            &url,
            "{\"signed\":true}",
            "sig",
            Some(Duration::from_secs(2)),
        );
        server.join().unwrap();
        std::thread::sleep(Duration::from_millis(100));
        assert!(decoy.accept().is_err(), "the signed body must stay home");
        assert_eq!(outcome, PostOutcome::Status(307));
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
            Some(Duration::from_secs(10)),
            "async carries the ten second deadline"
        );
    }

    #[test]
    fn sync_carries_the_validated_sync_deadline() {
        let (channel, path) =
            channel_with_auth(r#"{"hermes_secret":"key"}"#, PostOutcome::Status(200));
        channel.deliver(&event(), Mode::Sync);
        std::fs::remove_file(&path).ok();
        assert_eq!(
            channel.post.posts.borrow()[0].3,
            Some(Duration::from_secs(5))
        );
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
