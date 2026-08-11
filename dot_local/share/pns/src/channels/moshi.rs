//! The moshi channel, native: the phone push, a single HTTPS POST.
//!
//! THE SECRET'S PATH IS THE POINT. The token is read from the auth file,
//! placed in the request BODY, and never touches argv, the environment of a
//! child, or an error string: the bash put it on stdin for the same reason
//! (the process table is world-readable), and in-process is the stronger
//! form of the same rule. An absent auth file or key is the
//! silent-unavailable case: the channel is simply not set up, which is not
//! an error and must not say anything.

use super::{Channel, Event};
use crate::routing::Mode;
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
    let _ = auth_json;
    todo!("R2f: .moshi_secret, non-empty, else None")
}

/// The webhook body: token, title, and the PREVIEW as the message, because
/// the phone card has a length ceiling the full message ignores.
pub fn webhook_body(token: &str, title: &str, preview: &str) -> String {
    let _ = (token, title, preview);
    todo!("R2f: the three-field JSON object")
}

/// Read the auth file quietly: any failure is None, the not-set-up case.
pub fn read_auth(path: &Path) -> Option<String> {
    let _ = path;
    todo!("R2f: read to string, silently None on any failure")
}

/// The native moshi plugin.
pub struct MoshiChannel<H: HttpPost> {
    pub http: H,
    /// `RELAY_AUTH_FILE` override, else `~/.config/relay/auth.json`.
    pub auth_path: std::path::PathBuf,
    /// `RELAY_MOSHI_URL` override, else the default.
    pub url: String,
}

impl<H: HttpPost> Channel for MoshiChannel<H> {
    fn deliver(&self, event: &Event, mode: Mode) {
        let _ = (event, mode);
        todo!("R2f: no secret means no post; one post otherwise, failure ignored")
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_MOSHI_URL, HttpPost, MoshiChannel, moshi_secret, webhook_body};
    use crate::channels::{Channel, Event};
    use crate::routing::Mode;
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

    fn channel_with_auth(auth: &str) -> (MoshiChannel<RecordingHttp>, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "pns-moshi-auth-{}-{}",
            std::process::id(),
            auth.len()
        ));
        std::fs::write(&path, auth).unwrap();
        (
            MoshiChannel {
                http: RecordingHttp {
                    posts: RefCell::new(Vec::new()),
                },
                auth_path: path.clone(),
                url: DEFAULT_MOSHI_URL.to_string(),
            },
            path,
        )
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
        let (channel, path) = channel_with_auth(r#"{"other":"x"}"#);
        channel.deliver(&event(), Mode::Async);
        std::fs::remove_file(&path).ok();
        assert!(channel.http.posts.borrow().is_empty());
    }

    #[test]
    fn a_token_posts_once_to_the_url_with_the_preview_never_the_message() {
        let (channel, path) = channel_with_auth(r#"{"moshi_secret":"tok-1"}"#);
        channel.deliver(&event(), Mode::Async);
        std::fs::remove_file(&path).ok();
        let posts = channel.http.posts.borrow();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].0, DEFAULT_MOSHI_URL);
        assert!(posts[0].1.contains("a preview"));
        assert!(
            !posts[0].1.contains("longer than the preview"),
            "the phone gets the ceiling-safe preview"
        );
    }

    #[test]
    fn an_absent_auth_file_is_silently_not_set_up() {
        let channel = MoshiChannel {
            http: RecordingHttp {
                posts: RefCell::new(Vec::new()),
            },
            auth_path: std::path::PathBuf::from("/nonexistent/pns-moshi-auth"),
            url: DEFAULT_MOSHI_URL.to_string(),
        };
        channel.deliver(&event(), Mode::Async);
        assert!(channel.http.posts.borrow().is_empty());
    }
}
