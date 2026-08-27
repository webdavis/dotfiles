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

use super::{Delivery, Event};
use crate::routing::ReportMode;
use std::path::PathBuf;
use std::time::Duration;

/// The gateway when `PNS_HERMES_URL` says nothing: the local hermes
/// webhook route.
pub const DEFAULT_HERMES_URL: &str = "http://127.0.0.1:8644/webhooks/pns";

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

/// The URL for a NAMED route on the same gateway the default posts to: the
/// base URL with its final path segment swapped for the route. Names, not
/// URLs, cross the CLI: the gateway and its route table stay the single
/// source of truth in the hermes config, and a caller says only WHERE.
///
/// `None` for a route that could not safely become a path segment; the
/// caller says so and posts to the default, because a misrouted notification
/// on the loud route beats a silently dropped one.
pub fn channel_url(base_url: &str, route: &str) -> Option<String> {
    if route.is_empty()
        || !route
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return None;
    }
    let (prefix, _default_route) = base_url.rsplit_once('/')?;
    Some(format!("{prefix}/{route}"))
}

#[cfg(test)]
mod channel_url_tests {
    use super::{DEFAULT_HERMES_URL, channel_url};

    #[test]
    fn a_route_name_swaps_the_default_urls_final_segment() {
        assert_eq!(
            channel_url(DEFAULT_HERMES_URL, "unattended-upgrades").as_deref(),
            Some("http://127.0.0.1:8644/webhooks/unattended-upgrades")
        );
    }

    #[test]
    fn a_name_that_could_not_be_a_path_segment_is_refused_not_glued() {
        // The name is about to become part of a URL, so this is a trust
        // boundary like the site id's: nothing traversal-shaped passes.
        for hostile in ["", "a/b", "../x", "a b", "a?x=1", "a#f", "."] {
            assert_eq!(
                channel_url(DEFAULT_HERMES_URL, hostile),
                None,
                "case: {hostile:?}"
            );
        }
    }

    #[test]
    fn a_base_without_a_path_yields_nothing_rather_than_a_bogus_url() {
        assert_eq!(channel_url("no-slashes-here", "log"), None);
    }
}

/// The line sync mode prints for one outcome, exactly as the bash spells it.
pub fn outcome_line(outcome: PostOutcome) -> String {
    match outcome {
        PostOutcome::Status(code) if (200..300).contains(&code) => {
            format!("pns: posted HTTP {code}")
        }
        PostOutcome::Status(code) => format!("pns: post FAILED HTTP {code}"),
        PostOutcome::NoStatus => {
            "pns: post FAILED (curl reported no HTTP status at all)".to_string()
        }
        PostOutcome::NoResponse => {
            "pns: post FAILED HTTP 000 (no response; is the hermes gateway up?)".to_string()
        }
    }
}

/// The line sync mode prints when there is no signing key.
pub fn skipped_line(auth_path: &std::path::Path) -> String {
    format!(
        "pns: post SKIPPED -- no hermes signing key in {}; nothing was sent",
        auth_path.display()
    )
}

/// The deadline an ASYNC leg posts under. Not configurable: nobody is waiting
/// on the answer, so this only bounds how long a background process lingers.
const ASYNC_DEADLINE: Duration = Duration::from_secs(10);

/// The default SYNC deadline, the one a caller waits out. Short because the
/// caller is blocked on it, and configurable for the same reason.
const DEFAULT_SYNC_DEADLINE_SECS: u64 = 5;

/// The ceiling a configured sync deadline is clamped to: a day is already
/// longer than any notification can matter, and it keeps an absurd value out
/// of ureq's deadline arithmetic.
const MAX_SYNC_DEADLINE_SECS: u64 = 86_400;

/// The sync deadline: `PNS_REMOTE_TIMEOUT` validated as a count, else 5
/// seconds, because a garbled deadline must not become zero or forever.
pub fn remote_deadline(env_value: Option<&str>) -> Option<Duration> {
    let seconds = env_value
        .and_then(crate::parse_count)
        .unwrap_or(DEFAULT_SYNC_DEADLINE_SECS);
    // Zero is curl's `-m 0`: no deadline at all, and caller intent rather
    // than a default.
    (seconds != 0).then(|| Duration::from_secs(seconds.min(MAX_SYNC_DEADLINE_SECS)))
}

/// The native hermes plugin.
pub struct HermesChannel<P: SignedPost> {
    pub post: P,
    /// The signing key, read from the auth file ONCE at the composition
    /// root. None is the not-set-up case.
    pub key: Option<String>,
    /// Where that file was, for the not-set-up line only.
    pub auth_path: PathBuf,
    /// `PNS_HERMES_URL` override, else the default.
    pub url: String,
    /// The sync deadline, already validated at the edge; None is curl's
    /// explicit no-deadline.
    pub sync_deadline: Option<Duration>,
}

impl<P: SignedPost> HermesChannel<P> {
    pub fn deliver(&self, event: &Event, mode: ReportMode) -> Delivery {
        let body = hermes_body(event);
        let Some(signature) = self.key.as_deref().and_then(|key| sign(key, &body)) else {
            // No key is unavailable, not a failure. Reported because an
            // empty Discord channel looks like the jobs stopped.
            return Delivery::Reported(skipped_line(&self.auth_path));
        };

        let deadline = match mode {
            ReportMode::ReportOutcome => self.sync_deadline,
            ReportMode::Silent => Some(ASYNC_DEADLINE),
        };
        Delivery::Reported(outcome_line(
            self.post.post(&self.url, &body, &signature, deadline),
        ))
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
        let sent = ureq::Agent::config_builder()
            // None is no deadline at all, so the option passes straight
            // through rather than being defaulted back into one.
            .timeout_global(deadline)
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
            // The request was never put on the wire: a URI ureq refuses, or a
            // header the http crate refuses to build. Curl prints no status
            // at all for these, which is a different report from a silent
            // gateway.
            Err(ureq::Error::BadUri(_) | ureq::Error::Http(_)) => PostOutcome::NoStatus,
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
    /// extracted, and a path that exists only to be named in the not-set-up
    /// line.
    fn channel_with_auth(auth: &str, outcome: PostOutcome) -> HermesChannel<RecordingPost> {
        HermesChannel {
            post: RecordingPost {
                outcome,
                posts: RefCell::new(Vec::new()),
            },
            key: hermes_secret(auth),
            auth_path: std::path::PathBuf::from("/test/auth.json"),
            url: "http://127.0.0.1:9/test".to_string(),
            sync_deadline: Some(Duration::from_secs(5)),
        }
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
            "pns: posted HTTP 200"
        );
        assert_eq!(
            outcome_line(PostOutcome::Status(204)),
            "pns: posted HTTP 204"
        );
        assert_eq!(
            outcome_line(PostOutcome::Status(404)),
            "pns: post FAILED HTTP 404"
        );
        assert_eq!(
            outcome_line(PostOutcome::NoResponse),
            "pns: post FAILED HTTP 000 (no response; is the hermes gateway up?)"
        );
    }

    #[test]
    fn the_no_key_line_names_the_auth_file_the_operator_must_fix() {
        assert_eq!(
            skipped_line(std::path::Path::new("/x/auth.json")),
            "pns: post SKIPPED -- no hermes signing key in /x/auth.json; nothing was sent"
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
            "pns: post FAILED HTTP 301"
        );
    }

    #[test]
    fn the_never_attempted_case_has_its_own_bash_wording() {
        assert_eq!(
            outcome_line(PostOutcome::NoStatus),
            "pns: post FAILED (curl reported no HTTP status at all)"
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
        let channel = channel_with_auth(
            r#"{"hermes_secret":"sekrit-key-9"}"#,
            PostOutcome::Status(200),
        );
        channel.deliver(&event(), ReportMode::Silent);
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
                // Consume the WHOLE request (headers plus Content-Length body)
                // before answering: responding after one read can reset the
                // socket under a client still writing, which turns the
                // outcome into NoResponse on a slow runner.
                let mut raw = Vec::new();
                let mut chunk = [0u8; 2048];
                let header_end = loop {
                    let read = stream.read(&mut chunk).unwrap_or(0);
                    if read == 0 {
                        break raw.len();
                    }
                    raw.extend_from_slice(&chunk[..read]);
                    if let Some(position) = raw.windows(4).position(|window| window == b"\r\n\r\n")
                    {
                        break position + 4;
                    }
                };
                let content_length = String::from_utf8_lossy(&raw[..header_end])
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())?
                    })
                    .unwrap_or(0);
                while raw.len() < header_end + content_length {
                    let read = stream.read(&mut chunk).unwrap_or(0);
                    if read == 0 {
                        break;
                    }
                    raw.extend_from_slice(&chunk[..read]);
                }
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{decoy_addr}/\r\nContent-Length: 0\r\n\r\n"
                    )
                    .as_bytes(),
                );
                let _ = stream.flush();
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
        let channel = channel_with_auth(r#"{"hermes_secret":"key"}"#, PostOutcome::Status(200));
        assert_eq!(
            channel.deliver(&event(), ReportMode::Silent),
            Delivery::Reported("pns: posted HTTP 200".to_string()),
            "the channel reports what happened; the leg's mode decides who hears it"
        );
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
        let channel = channel_with_auth(r#"{"hermes_secret":"key"}"#, PostOutcome::Status(200));
        channel.deliver(&event(), ReportMode::ReportOutcome);
        assert_eq!(
            channel.post.posts.borrow()[0].3,
            Some(Duration::from_secs(5))
        );
    }

    #[test]
    fn no_key_means_no_post_in_either_mode() {
        for mode in [ReportMode::Silent, ReportMode::ReportOutcome] {
            let channel = channel_with_auth(r#"{}"#, PostOutcome::Status(200));
            assert_eq!(
                channel.deliver(&event(), mode),
                Delivery::Reported(super::skipped_line(std::path::Path::new("/test/auth.json"))),
                "not set up is reported in both modes; only sync prints it"
            );
            assert!(channel.post.posts.borrow().is_empty());
        }
    }

    #[test]
    fn the_default_url_is_the_local_gateway_route() {
        assert_eq!(DEFAULT_HERMES_URL, "http://127.0.0.1:8644/webhooks/pns");
    }
}
