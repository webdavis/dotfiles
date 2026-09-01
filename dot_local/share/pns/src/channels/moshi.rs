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

/// The deep link a card's tap follows, built from the ORIGIN PANE and nothing
/// else, or None when there is no pane worth linking to.
///
/// PANE-PRECISE AND PLUMBING-FREE. moshi's scheme is
/// `moshi://herdr?workspace=&tab=&pane=&session=` with every parameter
/// optional (tab and pane since moshi 3.13.0), and the sanitized origin pane
/// is already here at the dispatch site. So the link is built from that and
/// nothing is plumbed: no second herdr call, no workspace on the session
/// view, no id threaded through the gate inputs.
///
/// WELL-FORMED BY CONSTRUCTION, because a malformed action does not degrade
/// the card, it DELETES it: moshi answers a bad body non-2xx, and this
/// channel reads any non-2xx as a delivery that failed. So the guard is
/// `pane_is_safe`, asked HERE rather than assumed of the caller, and its
/// charset (ascii alphanumeric plus `.`, `_`, `:` and `-`) is legal unencoded
/// in a query value, which is what leaves nothing to escape.
///
/// WHAT THE TAP ACTUALLY DOES, stated rather than fought: moshi looks for an
/// active card matching server session AND workspace, else resumes the most
/// recently minimized card for that session, and with no card matching at all
/// it SHOWS AN ERROR rather than opening a connection, because these links
/// only ever resume a card moshi already holds. This link names neither, so it
/// rides whichever card the phone already has and asks the host daemon to
/// refine it to the pane; a pane-only link is best-effort exact focus with no
/// parent to degrade to. It is a DECORATION, so no pane means no action and
/// the card ships exactly as it does without this.
pub fn herdr_link(pane: &str) -> Option<String> {
    crate::safety::pane_is_safe(pane).then(|| format!("moshi://herdr?pane={pane}"))
}

/// The webhook body: token, title, and the PREVIEW as the message, because
/// the phone card has a length ceiling the full message ignores, plus the
/// optional deep link the card's tap follows.
pub fn webhook_body(token: &str, title: &str, preview: &str, link: Option<&str>) -> String {
    let mut body = serde_json::json!({ "token": token, "title": title, "message": preview });
    if let Some(link) = link {
        // ONE `data` object carrying ONE `type`, which is what makes a url
        // action and an image action mutually exclusive: a structural limit of
        // the field, not a rule moshi states.
        body["data"] = serde_json::json!({ "type": "url", "url": link });
    }
    body.to_string()
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
            &webhook_body(
                token,
                &event.title,
                &event.preview,
                herdr_link(&event.pane).as_deref(),
            ),
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
        DEFAULT_MOSHI_URL, HttpPost, MOSHI_TYPE, MoshiChannel, herdr_link, mobile_backend,
        moshi_secret, webhook_body,
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
            // A REAL herdr pane id, colon and all, because the card's deep
            // link is built from exactly this field.
            pane: "wW:p21".to_string(),
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

    // --- the deep link ------------------------------------------------------

    #[test]
    fn a_safe_pane_becomes_a_pane_precise_herdr_link() {
        // The whole feature: the sanitized origin pane, spelled into moshi's
        // scheme with no workspace, no tab and no session, because the
        // dispatch site holds a pane and nothing else. Every parameter is
        // optional, so a pane-only link is a link.
        assert_eq!(
            herdr_link("wW:p21"),
            Some("moshi://herdr?pane=wW:p21".to_string())
        );
    }

    #[test]
    fn a_pane_the_safety_guard_refuses_gets_no_link_rather_than_an_escaped_one() {
        // A MALFORMED ACTION DELETES THE CARD, it does not degrade it: moshi
        // answers a bad body non-2xx and the whole delivery turns Failed. So
        // anything outside the guard's charset produces no action at all,
        // which is the plain card this feature decorates.
        for refused in [
            "",
            "wW:p21 evil",
            "a&workspace=x",
            "a#b",
            "a/b",
            "a?b",
            "a=b",
            "a%b",
            "a+b",
            "panée",
            "x; curl evil.sh | sh",
        ] {
            assert_eq!(herdr_link(refused), None, "case: {refused:?}");
        }
    }

    #[test]
    fn the_link_needs_no_escaping_because_the_guard_already_bounded_its_charset() {
        // The guard's whole alphabet at once. Every one of these characters is
        // legal unencoded in a query value (unreserved, plus the colon that
        // `pchar` admits), which is WHY the link can be a format string rather
        // than a percent-encoder nobody would test.
        let link = herdr_link("aZ9.-_:").expect("the guard's own charset is safe");
        assert_eq!(link, "moshi://herdr?pane=aZ9.-_:");
    }

    // --- the body -----------------------------------------------------------

    #[test]
    fn the_body_carries_token_title_and_the_preview_as_the_message() {
        // THE KEY COUNT IS A GUARD, not a formality: nothing rides along with
        // the secret that was not put there deliberately. It is now asked of
        // BOTH arms, because the link arm is exactly the edit that could
        // smuggle a fourth key in unnoticed.
        for (link, keys) in [(None, 3), (Some("moshi://herdr?pane=wW:p21"), 4)] {
            let body = webhook_body("tok-1", "a title", "a preview", link);
            let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert_eq!(parsed["token"], "tok-1");
            assert_eq!(parsed["title"], "a title");
            assert_eq!(parsed["message"], "a preview");
            assert_eq!(
                parsed.as_object().unwrap().len(),
                keys,
                "nothing else rides along with the secret; link: {link:?}"
            );
        }
    }

    #[test]
    fn a_link_rides_as_the_one_url_action_and_no_link_leaves_the_slot_absent() {
        // ONE `data` object holding ONE `type` is what makes url and image
        // mutually exclusive here: a structural limit of the field, not a rule
        // moshi documents.
        let linked = webhook_body("tok-1", "t", "p", Some("moshi://herdr?pane=wW:p21"));
        let parsed: serde_json::Value = serde_json::from_str(&linked).unwrap();
        assert_eq!(parsed["data"]["type"], "url");
        assert_eq!(parsed["data"]["url"], "moshi://herdr?pane=wW:p21");
        assert_eq!(
            parsed["data"].as_object().unwrap().len(),
            2,
            "one type and one url, never a second action beside them"
        );

        let plain = webhook_body("tok-1", "t", "p", None);
        let parsed: serde_json::Value = serde_json::from_str(&plain).unwrap();
        assert!(
            parsed.get("data").is_none(),
            "no pane means no action key at all, not an empty one: {plain}"
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

    #[test]
    fn the_posted_card_links_to_the_origin_pane_and_a_paneless_one_ships_plain() {
        // THE WIRING, end to end through the public seam: the event's own pane
        // is what the card links to, and an event that carries none posts the
        // card it posts today. A channel that built the link from anything
        // else, or dropped it, differs right here.
        //
        // THE THIRD ROW IS THE GUARD ITSELF, AT THIS SEAM. Without it a
        // dispatch site that stopped calling `herdr_link` and asked only
        // "is the pane non-empty" passes every test in this module: the two
        // rows above agree with that weaker rule, and `herdr_link` keeps its
        // own tests green while nothing calls it. So a pane that is present
        // and UNSAFE is asked here too, because this is where an unescaped
        // value would actually reach moshi's parser and turn the decoration
        // into a non-2xx that DELETES the card.
        for (pane, action) in [
            ("wW:p21", Some("moshi://herdr?pane=wW:p21")),
            ("", None),
            ("wW:p21 evil&workspace=x", None),
        ] {
            let channel = channel_with_settings("token = \"tok-1\"\n");
            channel.deliver(
                &Event {
                    pane: pane.to_string(),
                    ..event()
                },
                ReportMode::Silent,
            );
            let posts = channel.http.posts.borrow();
            let parsed: serde_json::Value = serde_json::from_str(&posts[0].1).unwrap();
            assert_eq!(
                parsed.get("data").map(|data| data["url"].clone()),
                action.map(serde_json::Value::from),
                "pane: {pane:?}"
            );
        }
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
