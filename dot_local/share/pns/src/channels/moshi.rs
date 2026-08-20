//! The moshi channel, native: the phone push, a single HTTPS POST.
//!
//! THE SECRET'S PATH IS THE POINT. The token is read from the auth file,
//! placed in the request BODY, and never touches argv, the environment of a
//! child, or an error string: the bash put it on stdin for the same reason
//! (the process table is world-readable), and in-process is the stronger
//! form of the same rule. An absent auth file or key is the
//! silent-unavailable case: the channel is simply not set up, which is not
//! an error and must not say anything.

use super::{Delivery, Event};
use crate::routing::ReportMode;
use std::path::Path;

/// Where the push goes when `RELAY_MOSHI_URL` says nothing.
pub const DEFAULT_MOSHI_URL: &str = "https://api.getmoshi.app/api/webhook";

/// The POST seam: one call, a URL and a JSON body in, success or not out.
/// The production impl carries the 10 second deadline; a fake records.
pub trait HttpPost {
    fn post_json(&self, url: &str, body: &str) -> bool;
}

/// The moshi token, or None for every way the auth file can fail to provide
/// one: absent, unreadable, not JSON, no key, or an empty value. All of
/// them mean "not set up", never an error.
pub fn moshi_secret(auth_json: &str) -> Option<String> {
    let token = serde_json::from_str::<serde_json::Value>(auth_json)
        .ok()?
        .get("moshi_secret")?
        .as_str()?
        .to_string();
    (!token.is_empty()).then_some(token)
}

/// The webhook body: token, title, and the PREVIEW as the message, because
/// the phone card has a length ceiling the full message ignores.
pub fn webhook_body(token: &str, title: &str, preview: &str) -> String {
    serde_json::json!({ "token": token, "title": title, "message": preview }).to_string()
}

/// Read the auth file quietly: any failure is None, the not-set-up case.
pub fn read_auth(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// The native moshi plugin.
pub struct MoshiChannel<H: HttpPost> {
    pub http: H,
    /// The token, read from the auth file ONCE at the composition root. None
    /// is the not-set-up case, which delivers nothing.
    pub token: Option<String>,
    /// `RELAY_MOSHI_URL` override, else the default.
    pub url: String,
}

impl<H: HttpPost> MoshiChannel<H> {
    /// Always silent: the only thing worth reporting would be the request
    /// that carries the token.
    pub fn deliver(&self, event: &Event, _mode: ReportMode) -> Delivery {
        if let Some(token) = &self.token {
            self.http.post_json(
                &self.url,
                &webhook_body(token, &event.title, &event.preview),
            );
        }
        Delivery::Silent
    }
}

/// The deadline one moshi post runs under. Nobody waits on the answer and
/// nothing is retried, so this only bounds how long the process lingers.
pub const POST_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

/// The production POST: one agent, one deadline, no retry. Every failure is
/// `false` and nothing is logged, because the only thing worth reporting
/// would be the request that carries the token.
pub struct UreqPost {
    /// The whole-request deadline. Production uses the default; tests hand
    /// in a short one to prove the deadline actually fires.
    pub timeout: std::time::Duration,
}

impl Default for UreqPost {
    fn default() -> Self {
        Self {
            timeout: POST_DEADLINE,
        }
    }
}

impl HttpPost for UreqPost {
    fn post_json(&self, url: &str, body: &str) -> bool {
        ureq::Agent::config_builder()
            .timeout_global(Some(self.timeout))
            // The bash curl carried no -L, and following one would send the
            // token to whatever host the endpoint names. Zero returns the 3xx
            // as the response rather than an error, so the post simply ends.
            .max_redirects(0)
            .build()
            .new_agent()
            .post(url)
            .content_type("application/json")
            .send(body)
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_MOSHI_URL, HttpPost, MoshiChannel, moshi_secret, webhook_body};
    use crate::channels::{Delivery, Event};
    use crate::routing::ReportMode;
    use std::cell::RefCell;

    struct RecordingHttp {
        posts: RefCell<Vec<(String, String)>>,
    }

    impl HttpPost for RecordingHttp {
        fn post_json(&self, url: &str, body: &str) -> bool {
            self.posts
                .borrow_mut()
                .push((url.to_string(), body.to_string()));
            true
        }
    }

    /// The channel as the composition root builds it: the secret already
    /// extracted, no file anywhere near it.
    fn channel_with_auth(auth: &str) -> MoshiChannel<RecordingHttp> {
        MoshiChannel {
            http: RecordingHttp {
                posts: RefCell::new(Vec::new()),
            },
            token: moshi_secret(auth),
            url: "https://example.invalid/hook".to_string(),
        }
    }

    fn event() -> Event {
        Event {
            title: "claude done: dotfiles".to_string(),
            preview: "a preview".to_string(),
            message: "the full message, longer than the preview".to_string(),
            ..Event::default()
        }
    }

    // --- the secret ---------------------------------------------------------

    #[test]
    fn the_secret_is_the_non_empty_moshi_key() {
        assert_eq!(
            moshi_secret(r#"{"moshi_secret":"tok-1","other":"x"}"#),
            Some("tok-1".to_string())
        );
    }

    #[test]
    fn every_way_the_auth_can_fail_to_provide_a_token_reads_not_set_up() {
        assert_eq!(moshi_secret(""), None);
        assert_eq!(moshi_secret("not json"), None);
        assert_eq!(moshi_secret(r#"{"other":"x"}"#), None);
        assert_eq!(moshi_secret(r#"{"moshi_secret":""}"#), None);
        assert_eq!(moshi_secret(r#"{"moshi_secret":42}"#), None);
    }

    // --- the body -----------------------------------------------------------

    #[test]
    fn the_body_carries_token_title_and_the_preview_as_the_message() {
        let body = webhook_body("tok-1", "a title", "a preview");
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["token"], "tok-1");
        assert_eq!(parsed["title"], "a title");
        assert_eq!(parsed["message"], "a preview");
        assert_eq!(
            parsed.as_object().unwrap().len(),
            3,
            "nothing else rides along with the secret"
        );
    }

    // --- delivery -----------------------------------------------------------

    #[test]
    fn no_token_means_no_post_and_no_sound() {
        let channel = channel_with_auth(r#"{"other":"x"}"#);
        assert_eq!(
            channel.deliver(&event(), ReportMode::Silent),
            Delivery::Silent
        );
        assert!(channel.http.posts.borrow().is_empty());
    }

    #[test]
    fn a_token_posts_once_to_the_url_with_the_preview_never_the_message() {
        let channel = channel_with_auth(r#"{"moshi_secret":"tok-1"}"#);
        channel.deliver(&event(), ReportMode::Silent);
        let posts = channel.http.posts.borrow();
        assert_eq!(posts.len(), 1);
        assert_eq!(
            posts[0].0, "https://example.invalid/hook",
            "the configured url is the one posted to, never the constant"
        );
        assert!(posts[0].1.contains("a preview"));
        assert!(
            !posts[0].1.contains("longer than the preview"),
            "the phone gets the ceiling-safe preview"
        );
    }

    #[test]
    fn no_token_at_all_is_silently_not_set_up() {
        let channel = MoshiChannel {
            http: RecordingHttp {
                posts: RefCell::new(Vec::new()),
            },
            token: None,
            url: DEFAULT_MOSHI_URL.to_string(),
        };
        assert_eq!(
            channel.deliver(&event(), ReportMode::Silent),
            Delivery::Silent
        );
        assert!(channel.http.posts.borrow().is_empty());
    }

    // --- the production post, against real sockets ---------------------------

    use super::UreqPost;
    use std::time::Duration;

    #[test]
    fn the_deadline_fires_instead_of_parking_the_notification_path() {
        // A bound socket that never accepts: the connection black-holes, and
        // the deadline is what keeps the hermes and banner legs from queuing
        // behind a dead network for minutes.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/hook", listener.local_addr().unwrap());
        let post = UreqPost {
            timeout: Duration::from_millis(100),
        };
        let started = std::time::Instant::now();
        assert!(!post.post_json(&url, "{}"), "a dead endpoint is a false");
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "the deadline fired, not the OS timeout"
        );
    }

    #[test]
    fn a_closed_port_is_a_quiet_false_never_a_report() {
        // The only thing worth reporting would be the request that carries
        // the token, so failure returns false and says nothing. The stderr
        // half is held by the integration gate, which runs this same path
        // against a closed port and asserts empty output.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/hook", listener.local_addr().unwrap());
        drop(listener);
        assert!(!UreqPost::default().post_json(&url, "{}"));
    }

    #[test]
    fn a_redirecting_endpoint_is_never_followed() {
        // The bash curl carried no -L: a compromised endpoint must not turn
        // the channel into a blind request against a target it names. The
        // decoy listener proves no second request happens.
        use std::io::{Read, Write};
        let decoy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let decoy_addr = decoy.local_addr().unwrap();
        let hit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let decoy_hit = hit.clone();
        decoy.set_nonblocking(true).unwrap();

        let redirector = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/hook", redirector.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = redirector.accept() {
                let mut request = [0u8; 1024];
                let _ = stream.read(&mut request);
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 302 Found\r\nLocation: http://{decoy_addr}/\r\nContent-Length: 0\r\n\r\n"
                    )
                    .as_bytes(),
                );
            }
        });
        let post = UreqPost {
            timeout: Duration::from_secs(2),
        };
        post.post_json(&url, "{}");
        server.join().unwrap();
        std::thread::sleep(Duration::from_millis(100));
        if decoy.accept().is_ok() {
            decoy_hit.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        assert!(
            !hit.load(std::sync::atomic::Ordering::SeqCst),
            "the redirect target must never be contacted"
        );
    }
}
