//! The poll over the scripted transport. NO LIVE CALL IS EVER MADE from
//! here: `ScriptedConnector` answers every request, and the token below is an
//! obvious fake.

use super::*;
use crate::http_script::{LoopbackResolver, ScriptedConnector, http_response};
use std::sync::{Arc, Mutex};

/// An obviously fake token. The real one lives in the vault and is never read
/// by a test.
const FAKE_TOKEN: &str = "ghp-not-a-real-token";

/// The production config semantics (no redirects) over the scripted wire.
fn scripted(responses: &[Vec<u8>]) -> (GithubNotifications, Arc<Mutex<Vec<u8>>>) {
    let connector = ScriptedConnector::default();
    let wire = Arc::clone(&connector.wire);
    *connector.responses.lock().unwrap() = responses.iter().cloned().collect();
    // THE PRODUCTION CONFIG ITSELF, not a copy of its intent.
    let agent = ureq::Agent::with_parts(
        GithubNotifications::production_config(),
        connector,
        LoopbackResolver,
    );
    (
        GithubNotifications::with_agent(
            agent,
            "http://localhost:9".to_string(),
            FAKE_TOKEN.to_string(),
        ),
        wire,
    )
}

const ONE_THREAD: &str = r#"[{"id":"20111","reason":"ci_activity","updated_at":"2026-09-14T15:16:27Z",
  "subject":{"title":"lint","url":"https://api.github.com/repos/webdavis/dotfiles/check-suites/4471","type":"CheckSuite"},
  "repository":{"full_name":"webdavis/dotfiles","html_url":"https://github.com/webdavis/dotfiles"}}]"#;

fn sent(wire: &Arc<Mutex<Vec<u8>>>) -> String {
    String::from_utf8_lossy(&wire.lock().unwrap()).to_lowercase()
}

#[test]
fn the_request_carries_the_token_the_accept_header_and_the_api_version() {
    let (github, wire) = scripted(&[http_response(
        "200 OK",
        &[("content-type", "application/json")],
        ONE_THREAD,
    )]);
    github.poll("");
    let wire = sent(&wire);
    assert!(
        wire.contains(&format!("authorization: bearer {FAKE_TOKEN}")),
        "the token rides the header: {wire}"
    );
    assert!(
        wire.contains("accept: application/vnd.github+json"),
        "{wire}"
    );
    assert!(wire.contains("x-github-api-version: 2022-11-28"), "{wire}");
    assert!(
        wire.contains("get /notifications?participating=false&per_page=50 http/1.1"),
        "participating and per_page are stated rather than left to a default: {wire}"
    );
}

#[test]
fn a_stored_cursor_is_sent_as_if_modified_since_and_an_empty_one_is_not_sent() {
    // THE MUTANT THIS PINS: the header omitted, which turns every free 304
    // into a full 200 and spends the rate limit the poll is sized against.
    let (github, wire) = scripted(&[http_response("304 Not Modified", &[], "")]);
    github.poll("Thu, 25 Oct 2026 15:16:27 GMT");
    assert!(
        sent(&wire).contains("if-modified-since: thu, 25 oct 2026 15:16:27 gmt"),
        "{}",
        sent(&wire)
    );

    let (github, wire) = scripted(&[http_response("304 Not Modified", &[], "")]);
    github.poll("");
    assert!(
        !sent(&wire).contains("if-modified-since"),
        "no cursor means no header, rather than an empty one: {}",
        sent(&wire)
    );
}

#[test]
fn a_304_is_nothing_happened_and_the_interval_it_asked_for() {
    let (github, _) = scripted(&[http_response(
        "304 Not Modified",
        &[("x-poll-interval", "120")],
        "",
    )]);
    assert_eq!(
        github.poll("Thu, 25 Oct 2026 15:16:27 GMT"),
        Polled::NotModified {
            interval_secs: Some(120)
        }
    );
}

#[test]
fn a_304_stating_no_interval_states_none_rather_than_a_compiled_in_number() {
    // The caller keeps the interval it already held; a number invented here
    // would silently override a server that had raised it.
    let (github, _) = scripted(&[http_response("304 Not Modified", &[], "")]);
    assert_eq!(
        github.poll("c"),
        Polled::NotModified {
            interval_secs: None
        }
    );
}

#[test]
fn a_200_carries_the_threads_the_cursor_and_the_interval() {
    let (github, _) = scripted(&[http_response(
        "200 OK",
        &[
            ("content-type", "application/json"),
            ("last-modified", "Thu, 25 Oct 2026 15:16:27 GMT"),
            ("x-poll-interval", "60"),
        ],
        ONE_THREAD,
    )]);
    let Polled::Listed { threads, answer } = github.poll("") else {
        panic!("a 200 lists");
    };
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].id, "20111");
    assert_eq!(answer.last_modified, "Thu, 25 Oct 2026 15:16:27 GMT");
    assert_eq!(answer.interval_secs, Some(60));
    assert!(
        answer.identities.is_empty(),
        "the identities are the policy's to compute, not the wire's"
    );
}

#[test]
fn a_link_header_naming_a_next_page_is_followed_and_only_page_ones_headers_are_kept() {
    // THE MUTANT THIS PINS: page one's own fifty read as the whole listing,
    // which permanently hides an account's older unread threads behind a
    // page boundary nothing here ever crosses; notifications are never
    // marked read, so a busy account can carry more than one page for a
    // long time.
    let second_thread = ONE_THREAD.replace("20111", "20222").replace("4471", "4472");
    let (github, wire) = scripted(&[
        http_response(
            "200 OK",
            &[
                ("content-type", "application/json"),
                ("last-modified", "Thu, 25 Oct 2026 15:16:27 GMT"),
                ("x-poll-interval", "60"),
                (
                    "link",
                    r#"<http://localhost:9/notifications?participating=false&per_page=50&page=2>; rel="next""#,
                ),
            ],
            ONE_THREAD,
        ),
        http_response(
            "200 OK",
            &[
                ("content-type", "application/json"),
                // A continuation page carries no cursor of its own; only page
                // one's headers are read.
                ("last-modified", "Thu, 25 Oct 2026 16:00:00 GMT"),
            ],
            &second_thread,
        ),
    ]);
    let Polled::Listed { threads, answer } = github.poll("") else {
        panic!("a 200 lists");
    };
    assert_eq!(threads.len(), 2, "both pages' threads are carried");
    assert_eq!(threads[0].id, "20111");
    assert_eq!(threads[1].id, "20222");
    assert_eq!(
        answer.last_modified, "Thu, 25 Oct 2026 15:16:27 GMT",
        "page one's cursor, not the continuation page's"
    );
    let wire = sent(&wire);
    assert_eq!(
        wire.matches("get ").count(),
        2,
        "the next link was followed: {wire}"
    );
    assert!(
        wire.contains("page=2"),
        "the request used the server's own url rather than one rebuilt here: {wire}"
    );
}

#[test]
fn a_401_is_a_configuration_problem_and_never_an_empty_listing() {
    // THE MUTANT THIS PINS: a refusal read as "no notifications", which is a
    // source that looks alive and reports nothing for as long as the token
    // stays revoked.
    let (github, _) = scripted(&[http_response(
        "401 Unauthorized",
        &[("content-type", "application/json")],
        r#"{"message":"Bad credentials"}"#,
    )]);
    assert_eq!(github.poll(""), Polled::Unauthorized { status: 401 });
}

#[test]
fn a_403_that_is_not_the_rate_limit_is_the_same_configuration_problem() {
    let (github, _) = scripted(&[http_response(
        "403 Forbidden",
        &[
            ("content-type", "application/json"),
            ("x-ratelimit-remaining", "4998"),
        ],
        r#"{"message":"Resource not accessible by personal access token"}"#,
    )]);
    assert_eq!(github.poll(""), Polled::Unauthorized { status: 403 });
}

#[test]
fn a_5xx_or_an_unreachable_host_is_unavailable_rather_than_either_of_those() {
    // Transient by assumption: the next tick tries again, and nothing tells
    // the operator to go and check a token that is fine.
    let (github, _) = scripted(&[http_response("503 Service Unavailable", &[], "")]);
    assert!(matches!(github.poll(""), Polled::Unavailable { .. }));

    let (github, _) = scripted(&[Vec::new()]);
    assert!(matches!(github.poll(""), Polled::Unavailable { .. }));
}

#[test]
fn a_redirect_is_never_followed_so_the_token_reaches_one_host_only() {
    // max_redirects(0) is production config: a 301 must not send the
    // Authorization header to the Location target.
    let (github, wire) = scripted(&[http_response(
        "301 Moved Permanently",
        &[("location", "http://elsewhere.test/")],
        "",
    )]);
    assert!(matches!(github.poll(""), Polled::Unavailable { .. }));
    let wire = sent(&wire);
    assert_eq!(wire.matches("get ").count(), 1, "no second request: {wire}");
    assert!(
        !wire.contains("elsewhere"),
        "the redirect target is never contacted"
    );
}

#[test]
fn a_listing_past_the_cap_is_unavailable_rather_than_silently_empty() {
    // The oversized body is VALID and would parse into one thread if it were
    // read, which is what makes this pin the cap itself rather than the
    // parse: a garbage body reads as no threads under any cap.
    let padded = format!(
        r#"[{{"id":"20111","padding":"{}","reason":"ci_activity","updated_at":"2026-09-14T15:16:27Z",
           "subject":{{"title":"lint","url":"","type":"CheckSuite"}},
           "repository":{{"full_name":"a/b","html_url":"https://github.com/a/b"}}}}]"#,
        "x".repeat(GITHUB_BODY_CAP as usize + 100)
    );
    let (github, _) = scripted(&[http_response(
        "200 OK",
        &[("content-type", "application/json")],
        &padded,
    )]);
    assert!(matches!(github.poll(""), Polled::Unavailable { .. }));
}
