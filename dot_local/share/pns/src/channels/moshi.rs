//! The moshi backend of the `mobile` channel, native: the phone push, a single
//! HTTPS POST. `mobile` is the plugin the config selects and `type = "moshi"`
//! is what picks this; the module keeps the backend's name because that is what
//! it implements.
//!
//! THE SECRET'S PATH IS THE POINT. The token is read from the config's
//! `[plugins.mobile]` table, placed in the request BODY, and never touches
//! argv, the environment of a child, or an error string: the bash put it on
//! stdin for the same reason (the process table is world-readable), and
//! in-process is the stronger form of the same rule. A missing or empty token
//! is the not-set-up case, and the deliver seam FAILS it by naming the config
//! key to write: nothing is posted, and no event hears the sentence, because
//! this channel is never handed a reporting leg. What has not changed is where
//! the token may appear, which is the request body and nowhere else.

use super::{Delivery, Event};
use crate::routing::ReportMode;

/// Where the push goes when `PNS_MOSHI_URL` says nothing.
pub const DEFAULT_MOSHI_URL: &str = "https://api.getmoshi.app/api/webhook";

/// The POST seam: one call, a URL and a JSON body in, success or not out.
/// The production impl carries the 10 second deadline; a fake records.
pub trait HttpPost {
    fn post_json(&self, url: &str, body: &str) -> bool;
}

/// The one `[plugins.mobile] type` a compiled-in backend answers. VALIDATED
/// AND THEN DISCARDED, the way the router sensor's is: the enum that
/// dispatches between two backends is worth writing the day there are two.
pub const MOSHI_TYPE: &str = "moshi";

/// Whether the mobile table names a backend this binary answers, and the
/// REASON when it does not.
///
/// `mobile` IS THE PLUGIN AND `type` IS WHAT IS BEHIND IT, which is why this
/// is settled before anything under the table is read: every setting there
/// belongs to whichever backend the table names, and a table that names none
/// must not be read as this one. The day a second backend compiles in, a
/// config that never said which would otherwise keep whichever arm happened to
/// be written first, silently.
///
/// PRESENT BUT EMPTY IS THE KEY LEFT BLANK, the same hole as absent and the
/// same reading `home::router_settings` gives its own `type`. THE TWO ARE THE
/// SAME QUESTION ASKED OF TWO TABLES and they are worded to match on purpose:
/// name the table, quote what was written, name the one type that answers.
/// Reword one and reword the other, or the rename that gave both tables one
/// word leaves them two sentences.
///
/// THE REASON CARRIES NO PREFIX AND NO VERDICT. It is wrapped twice, by
/// `refused_backend_line` for the leg that will not be dispatched and by the
/// composition root for its one line on stderr, so one fault has one wording
/// wherever it is said.
///
/// IT IS RETURNED, NOT PRINTED, exactly as `stale_alert_channel`'s complaint
/// is: this stays a value function, and the composition root is where a
/// warning becomes a line.
pub fn mobile_backend(settings: &toml::Table) -> Result<(), String> {
    let named = settings
        .get("type")
        .and_then(toml::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("no type in [plugins.mobile]; the only type is {MOSHI_TYPE:?}"))?;
    if named != MOSHI_TYPE {
        return Err(format!(
            "[plugins.mobile] has type {named:?}, which no compiled-in backend answers; \
             the only type is {MOSHI_TYPE:?}"
        ));
    }
    Ok(())
}

/// The moshi token out of the `[plugins.mobile]` settings, or None for every
/// way the config can fail to provide one: no `token` key, the wrong type,
/// or an empty value. All of them mean "not set up", never an error.
pub fn moshi_secret(settings: &toml::Table) -> Option<String> {
    let token = settings.get("token")?.as_str()?;
    (!token.is_empty()).then(|| token.to_string())
}

/// The webhook body: token, title, and the PREVIEW as the message, because
/// the phone card has a length ceiling the full message ignores.
pub fn webhook_body(token: &str, title: &str, preview: &str) -> String {
    serde_json::json!({ "token": token, "title": title, "message": preview }).to_string()
}

/// The native moshi plugin.
pub struct MoshiChannel<H: HttpPost> {
    pub http: H,
    /// The token, read from the config at the composition root. None is the
    /// not-set-up case, which delivers nothing.
    pub token: Option<String>,
    /// `PNS_MOSHI_URL` override, else the default.
    pub url: String,
}

impl<H: HttpPost> MoshiChannel<H> {
    /// WHETHER THE PUSH LANDED, NEVER WHAT IT CARRIED. The channel used to be
    /// silent on the reasoning that the only thing worth reporting would be
    /// the request holding the token; the verdict says nothing about the
    /// request, so the secret stays where it was and a hand-run check can
    /// finally learn that the phone leg is broken.
    ///
    /// NO EVENT HEARS ANY OF IT. `ReportOutcome` is produced only under
    /// `--remote-only`, which selects durable plugins, and this one is not
    /// durable, so these sentences are unreachable from an event's stdout.
    pub fn deliver(&self, event: &Event, _mode: ReportMode) -> Delivery {
        let Some(token) = &self.token else {
            return Delivery::Failed(NO_TOKEN_LINE.to_string());
        };
        if self.http.post_json(
            &self.url,
            &webhook_body(token, &event.title, &event.preview),
        ) {
            Delivery::Delivered("pushed the card".to_string())
        } else {
            // WHY IS NOT KNOWN, and the sentence says so rather than picking
            // one: the seam answers a bool, so a refusal and an unreachable
            // endpoint arrive here identically.
            Delivery::Failed(
                "push FAILED (the moshi endpoint refused it or could not be reached)".to_string(),
            )
        }
    }
}

/// The line for a channel that was selected and never set up. It names the
/// config key to write, the way hermes's does, because "not set up" without an
/// address sends the operator hunting.
const NO_TOKEN_LINE: &str =
    "push SKIPPED -- no moshi token in the config ([plugins.mobile] token); nothing was sent";

/// The line for a mobile leg refused before either delivery seam: the table
/// names a backend nothing compiled in answers.
///
/// THE SAME SHAPE AS `NO_TOKEN_LINE` ABOVE IT, and deliberately beside it,
/// because the two are the same news in the operator's terms: the leg was
/// selected, nothing was sent, and here is the config to fix. What differs is
/// only which key is wrong, and a report that named `token` for a `type` fault
/// sends them to the one edit that is already correct.
pub fn refused_backend_line(reason: &str) -> String {
    format!("push SKIPPED -- {reason}; nothing was sent")
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
            // NOT `is_ok`. With no redirects followed, a 3xx comes back as a
            // RESPONSE rather than an error, so `is_ok` answered true for a
            // card the endpoint bounced somewhere else and never delivered.
            .is_ok_and(|response| DELIVERED_STATUS.contains(&response.status().as_u16()))
    }
}

/// The status codes that mean the card reached the phone. Spelled here rather
/// than shared with hermes: the two channels answer to different endpoints and
/// a range moved for one of them must not follow the other.
const DELIVERED_STATUS: std::ops::Range<u16> = 200..300;

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_MOSHI_URL, HttpPost, MOSHI_TYPE, MoshiChannel, mobile_backend, moshi_secret,
        webhook_body,
    };
    use crate::channels::{Delivery, Event};
    use crate::routing::ReportMode;
    use std::cell::RefCell;

    struct RecordingHttp {
        /// What the endpoint answers. Scripted, the way hermes's recorded post
        /// already carries its outcome: a push that was refused is reachable
        /// no other way, and it is the direction a doctor exists to find.
        answers: bool,
        posts: RefCell<Vec<(String, String)>>,
    }

    impl RecordingHttp {
        fn answering(answers: bool) -> Self {
            RecordingHttp {
                answers,
                posts: RefCell::new(Vec::new()),
            }
        }
    }

    impl HttpPost for RecordingHttp {
        fn post_json(&self, url: &str, body: &str) -> bool {
            self.posts
                .borrow_mut()
                .push((url.to_string(), body.to_string()));
            self.answers
        }
    }

    /// The channel as the composition root builds it: the secret already
    /// extracted from the `[plugins.mobile]` settings, no file anywhere near it.
    fn channel_with_settings(settings: &str) -> MoshiChannel<RecordingHttp> {
        MoshiChannel {
            http: RecordingHttp::answering(true),
            token: moshi_secret(&settings.parse().unwrap()),
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

    // --- the backend the table names ----------------------------------------

    #[test]
    fn the_table_has_to_name_a_backend_and_the_refusal_names_the_key() {
        // NOTHING GUESSES A BACKEND. `mobile` is the plugin and `type` is the
        // implementation behind it, so a table that names none is refused
        // rather than read as this one: the day a second backend compiles in,
        // a config that never said which would silently keep whichever arm
        // happened to be first. A non-string and an EMPTY value are the same
        // hole as an absent key, which is the router's own reading.
        for settings in ["", "token = \"tok-1\"\n", "type = 5\n", "type = \"\"\n"] {
            let complaint = mobile_backend(&settings.parse().unwrap())
                .expect_err(&format!("case: {settings:?}"));
            assert!(complaint.contains("type"), "names the key: {complaint}");
            assert!(
                complaint.contains("[plugins.mobile]"),
                "and the table: {complaint}"
            );
            assert!(
                complaint.contains(MOSHI_TYPE),
                "and the one type that answers: {complaint}"
            );
        }
    }

    #[test]
    fn a_type_no_compiled_in_backend_answers_is_refused_quoting_it() {
        let complaint = mobile_backend(&"type = \"pushover\"\n".parse().unwrap())
            .expect_err("no backend answers `pushover`");
        assert!(complaint.contains("\"pushover\""), "got: {complaint}");
        assert!(complaint.contains(MOSHI_TYPE), "got: {complaint}");
    }

    #[test]
    fn the_one_compiled_in_type_is_accepted_and_its_token_is_read() {
        // The positive control: a refusal that fired on every table would pass
        // the two tests above and take the phone card away entirely.
        let settings: toml::Table = "type = \"moshi\"\ntoken = \"tok-1\"\n".parse().unwrap();
        assert_eq!(mobile_backend(&settings), Ok(()));
        assert_eq!(moshi_secret(&settings), Some("tok-1".to_string()));
    }

    // --- the secret ---------------------------------------------------------

    #[test]
    fn the_secret_is_the_non_empty_token_setting() {
        assert_eq!(
            moshi_secret(&"token = \"tok-1\"\nother = \"x\"\n".parse().unwrap()),
            Some("tok-1".to_string())
        );
    }

    #[test]
    fn every_way_the_settings_can_fail_to_provide_a_token_reads_not_set_up() {
        for settings in ["", "other = \"x\"\n", "token = \"\"\n", "token = 42\n"] {
            assert_eq!(
                moshi_secret(&settings.parse().unwrap()),
                None,
                "case: {settings:?}"
            );
        }
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
    fn a_missing_token_posts_nothing_and_fails_by_naming_the_config_key_to_write() {
        // BOTH WAYS a channel arrives without a token: a settings table that
        // provided none, and a composition root that read none at all.
        for channel in [
            channel_with_settings("other = \"x\"\n"),
            MoshiChannel {
                http: RecordingHttp::answering(true),
                token: None,
                url: DEFAULT_MOSHI_URL.to_string(),
            },
        ] {
            assert_eq!(
                channel.deliver(&event(), ReportMode::Silent),
                Delivery::Failed(
                    "push SKIPPED -- no moshi token in the config ([plugins.mobile] token); \
                     nothing was sent"
                        .to_string()
                )
            );
            assert!(channel.http.posts.borrow().is_empty());
        }
    }

    #[test]
    fn a_push_the_endpoint_took_is_delivered_and_one_it_did_not_is_failed_without_the_token() {
        // THE SENTENCE IS THE WHOLE ASSERTION on the failing side: the only
        // thing worth reporting about this channel used to be the request that
        // carries the token, so a verdict that named one would be the leak the
        // silence was protecting.
        for (answered, verdict) in [
            (true, Delivery::Delivered("pushed the card".to_string())),
            (
                false,
                Delivery::Failed(
                    "push FAILED (the moshi endpoint refused it or could not be reached)"
                        .to_string(),
                ),
            ),
        ] {
            let channel = MoshiChannel {
                http: RecordingHttp::answering(answered),
                token: Some("tok-secret-9".to_string()),
                url: "https://example.invalid/hook".to_string(),
            };
            assert_eq!(
                channel.deliver(&event(), ReportMode::Silent),
                verdict,
                "answered: {answered}"
            );
        }
    }

    #[test]
    fn a_token_posts_once_to_the_url_with_the_preview_never_the_message() {
        let channel = channel_with_settings("token = \"tok-1\"\n");
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
    fn a_redirect_is_not_a_delivery_however_the_endpoint_dresses_it_up() {
        // `max_redirects(0)` hands a 3xx back as an Ok RESPONSE rather than an
        // error, so a bare `is_ok` read one as a card that landed: the check
        // printed Delivered for a phone that was never pushed to. THE 200 IS
        // THE OTHER HALF, over a real socket, so the answer cannot become a
        // blanket false and pass this by refusing everything.
        use std::io::{Read, Write};
        for (status, delivered) in [("302 Found", false), ("200 OK", true)] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/hook", listener.local_addr().unwrap());
            let server = std::thread::spawn(move || {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut request = [0u8; 1024];
                    let _ = stream.read(&mut request);
                    let _ = stream.write_all(
                        format!(
                            "HTTP/1.1 {status}\r\nLocation: http://127.0.0.1:1/\r\n\
                             Content-Length: 0\r\n\r\n"
                        )
                        .as_bytes(),
                    );
                    let _ = stream.flush();
                    // HOLD the socket until the client hangs up: closing it
                    // right after the write can reset the response out from
                    // under a reader on a slow runner, which turns a written
                    // status into no response at all.
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                    let mut drain = [0u8; 256];
                    while matches!(stream.read(&mut drain), Ok(read) if read > 0) {}
                }
            });
            let post = UreqPost {
                timeout: Duration::from_secs(2),
            };
            assert_eq!(
                post.post_json(&url, "{}"),
                delivered,
                "the endpoint answered {status}"
            );
            server.join().unwrap();
        }
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
                let _ = stream.flush();
                // HOLD the socket until the client hangs up: closing it
                // right after the write can reset the response out from
                // under a reader on a slow runner, which turns a written
                // status into no response at all.
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let mut drain = [0u8; 256];
                while matches!(stream.read(&mut drain), Ok(read) if read > 0) {}
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
