//! The Google source over the scripted transport. NO LIVE CALL IS EVER MADE
//! from here: `ScriptedConnector` answers every request, and every credential
//! below is an obvious fake.

use super::freebusy::{
    FREEBUSY_ERRORS, FREEBUSY_MISSING, FREEBUSY_REFUSED, FREEBUSY_TIME, parse_freebusy,
};
use super::token::{TOKEN_REFUSED, TOKEN_STATE};
use super::*;
use crate::http_script::{LoopbackResolver, ScriptedConnector, http_ok, http_response};
use crate::state_fixtures::scratch;
use std::sync::{Arc, Mutex};

const FAKE_CLIENT_ID: &str = "not-a-real-client-id";
const FAKE_CLIENT_SECRET: &str = "not-a-real-client-secret";
const FAKE_REFRESH_TOKEN: &str = "not-a-real-refresh-token";
const FAKE_ACCESS_TOKEN: &str = "not-a-real-access-token";

/// Every secret byte one poll handles, which no error string may carry.
const SECRETS: [&str; 4] = [
    FAKE_CLIENT_ID,
    FAKE_CLIENT_SECRET,
    FAKE_REFRESH_TOKEN,
    FAKE_ACCESS_TOKEN,
];

const NOW: u64 = 1_789_658_187;
const DEADLINE: Duration = Duration::from_secs(5);

fn settings(calendars: &[&str]) -> GoogleCalendar {
    GoogleCalendar {
        calendars: calendars.iter().map(|id| (*id).to_string()).collect(),
        client_id: FAKE_CLIENT_ID.to_string(),
        client_secret: FAKE_CLIENT_SECRET.to_string(),
        refresh_token: FAKE_REFRESH_TOKEN.to_string(),
    }
}

/// The production config semantics (no redirects) over the scripted wire.
fn scripted(
    responses: &[Vec<u8>],
    calendars: &[&str],
) -> (GoogleCalendarSource, Arc<Mutex<Vec<u8>>>) {
    let connector = ScriptedConnector::default();
    let wire = Arc::clone(&connector.wire);
    *connector.responses.lock().unwrap() = responses.iter().cloned().collect();
    // THE PRODUCTION CONFIG ITSELF, not a copy of its intent.
    let agent = ureq::Agent::with_parts(
        GoogleCalendarSource::production_config(DEADLINE),
        connector,
        LoopbackResolver,
    );
    (
        GoogleCalendarSource::with_agent(
            agent,
            "http://localhost:9/token".to_string(),
            "http://localhost:9/freeBusy".to_string(),
            settings(calendars),
        ),
        wire,
    )
}

fn token_answer(expires_in: u64) -> Vec<u8> {
    http_ok(&format!(
        r#"{{"access_token":"{FAKE_ACCESS_TOKEN}","expires_in":{expires_in},"token_type":"Bearer"}}"#
    ))
}

const ONE_BUSY_HOUR: &str = r#"{"calendars":{"primary":{"busy":[
  {"start":"2026-09-17T16:00:00Z","end":"2026-09-17T17:00:00Z"}]}}}"#;

fn sent(wire: &Arc<Mutex<Vec<u8>>>) -> String {
    String::from_utf8_lossy(&wire.lock().unwrap()).to_lowercase()
}

fn cached_token(state: &std::path::Path) -> String {
    std::fs::read_to_string(state.join(TOKEN_STATE)).unwrap_or_default()
}

#[test]
fn a_fresh_poll_exchanges_the_refresh_token_then_asks_for_the_next_hour() {
    let state = scratch("google-fresh");
    let (source, wire) = scripted(&[token_answer(3_600), http_ok(ONE_BUSY_HOUR)], &["primary"]);
    let events = source.read(&state, NOW).expect("the documented pair reads");
    assert_eq!(
        events,
        vec![Event {
            start: 1_789_660_800,
            end: 1_789_664_400,
            busy: true,
        }]
    );
    let wire = sent(&wire);
    assert!(
        wire.contains("grant_type=refresh_token")
            && wire.contains(&format!("refresh_token={FAKE_REFRESH_TOKEN}")),
        "the exchange states the grant and the token: {wire}"
    );
    assert!(
        wire.contains("content-type: application/x-www-form-urlencoded"),
        "the exchange is a form post: {wire}"
    );
    assert!(
        wire.contains(&format!("authorization: bearer {FAKE_ACCESS_TOKEN}")),
        "the freeBusy call carries the access token: {wire}"
    );
    assert!(
        wire.contains("\"timemin\":\"2026-09-17t15:16:27z\"")
            && wire.contains("\"timemax\":\"2026-09-17t16:16:27z\""),
        "the window is the next hour: {wire}"
    );
    assert!(
        cached_token(&state).starts_with(&format!("{} {FAKE_ACCESS_TOKEN}", NOW + 3_600)),
        "the access token and its expiry are cached"
    );
}

/// THE MUTANT THIS PINS: the cache never read, which spends a token exchange
/// on every poll of every day.
#[test]
fn a_cached_token_still_inside_its_life_spends_no_exchange() {
    let state = scratch("google-cached");
    std::fs::write(
        state.join(TOKEN_STATE),
        format!("{} {FAKE_ACCESS_TOKEN}\n", NOW + 3_600),
    )
    .expect("plant the cache");
    let (source, wire) = scripted(&[http_ok(ONE_BUSY_HOUR)], &["primary"]);
    assert!(source.read(&state, NOW).is_ok());
    let wire = sent(&wire);
    assert!(
        !wire.contains("grant_type"),
        "a live cached token is not exchanged again: {wire}"
    );
}

/// THE MUTANT THIS PINS: the margin dropped, which leaves a token expiring
/// mid-call and a poll refused for no reason the operator can act on.
#[test]
fn a_cached_token_inside_the_margin_is_exchanged_again() {
    let state = scratch("google-margin");
    std::fs::write(
        state.join(TOKEN_STATE),
        format!("{} {FAKE_ACCESS_TOKEN}\n", NOW + 30),
    )
    .expect("plant the cache");
    let (source, wire) = scripted(&[token_answer(3_600), http_ok(ONE_BUSY_HOUR)], &["primary"]);
    assert!(source.read(&state, NOW).is_ok());
    assert!(
        sent(&wire).contains("grant_type=refresh_token"),
        "a token about to expire is exchanged"
    );
}

#[test]
fn a_refused_exchange_is_the_whole_poll_refused_naming_the_step() {
    let state = scratch("google-refused-token");
    for answer in [
        http_response("401 Unauthorized", &[], "{\"error\":\"invalid_grant\"}"),
        http_ok("{\"token_type\":\"Bearer\"}"),
        http_ok("not json at all"),
    ] {
        let (source, _) = scripted(&[answer], &["primary"]);
        assert_eq!(
            source.read(&state, NOW),
            Err(TOKEN_REFUSED.to_string()),
            "a refused exchange names the step and nothing else"
        );
    }
}

#[test]
fn two_calendars_are_read_as_the_union_of_their_busy_intervals() {
    let answer = r#"{"calendars":{
      "primary":{"busy":[{"start":"2026-09-17T16:00:00Z","end":"2026-09-17T17:00:00Z"}]},
      "second":{"busy":[{"start":"2026-09-17T18:00:00Z","end":"2026-09-17T18:30:00Z"}]}}}"#;
    let events =
        parse_freebusy(answer, &["primary".into(), "second".into()]).expect("both calendars read");
    assert_eq!(
        events,
        vec![
            Event {
                start: 1_789_660_800,
                end: 1_789_664_400,
                busy: true
            },
            Event {
                start: 1_789_668_000,
                end: 1_789_669_800,
                busy: true
            },
        ]
    );
}

#[test]
fn an_empty_busy_list_is_an_answer_and_not_a_refusal() {
    assert_eq!(
        parse_freebusy(
            r#"{"calendars":{"primary":{"busy":[]}}}"#,
            &["primary".into()]
        ),
        Ok(Vec::new())
    );
}

/// A DROPPED INTERVAL IS A MEETING THAT SILENTLY DOES NOT MUTE, and it looks
/// exactly like a clear calendar, so every one of these refuses the whole
/// answer.
#[test]
fn an_error_a_missing_calendar_or_an_unreadable_time_refuses_the_whole_answer() {
    let calendars = ["primary".to_string()];
    for (answer, refusal) in [
        (
            r#"{"calendars":{"primary":{"errors":[{"domain":"global","reason":"notFound"}],"busy":[]}}}"#,
            FREEBUSY_ERRORS,
        ),
        (r#"{"calendars":{"other":{"busy":[]}}}"#, FREEBUSY_MISSING),
        (
            r#"{"calendars":{"primary":{"busy":[{"start":"tomorrow","end":"2026-09-17T17:00:00Z"}]}}}"#,
            FREEBUSY_TIME,
        ),
        (
            r#"{"calendars":{"primary":{"busy":[{"end":"2026-09-17T17:00:00Z"}]}}}"#,
            FREEBUSY_REFUSED,
        ),
        (r#"{"calendars":{"primary":{}}}"#, FREEBUSY_REFUSED),
        ("{}", FREEBUSY_REFUSED),
        ("not json at all", FREEBUSY_REFUSED),
    ] {
        assert_eq!(
            parse_freebusy(answer, &calendars),
            Err(refusal.to_string()),
            "{answer}"
        );
    }
}

/// THE ONE TEST THE WHOLE PRIVACY RULE RESTS ON: every refusal path, driven
/// end to end, with the secret bytes asserted absent from what comes back.
#[test]
fn no_refusal_on_any_path_carries_a_credential_or_a_response_body() {
    let state = scratch("google-no-leak");
    let hostile =
        format!(r#"{{"error":"{FAKE_CLIENT_SECRET}","access_token_hint":"{FAKE_REFRESH_TOKEN}"}}"#);
    let scripts = [
        vec![http_response("401 Unauthorized", &[], hostile.as_str())],
        vec![http_ok(hostile.as_str())],
        vec![
            token_answer(3_600),
            http_response("403 Forbidden", &[], hostile.as_str()),
        ],
        vec![token_answer(3_600), http_ok(&hostile)],
        vec![
            token_answer(3_600),
            http_ok(r#"{"calendars":{"primary":{"errors":[{"reason":"notFound"}]}}}"#),
        ],
        vec![
            token_answer(3_600),
            http_ok(
                r#"{"calendars":{"primary":{"busy":[{"start":"Standup with Dana","end":"x"}]}}}"#,
            ),
        ],
        vec![],
    ];
    for (case, script) in scripts.into_iter().enumerate() {
        // A FRESH STATE EACH TIME, so a cached token from the run before
        // cannot skip the exchange this case is about.
        let state = state.join(format!("case-{case}"));
        let (source, _) = scripted(&script, &["primary"]);
        let refusal = source
            .read(&state, NOW)
            .expect_err("every script above refuses");
        for secret in SECRETS {
            assert!(
                !refusal.contains(secret),
                "a refusal never carries a credential: {refusal}"
            );
        }
        for said in ["Dana", "notFound", "invalid_grant"] {
            assert!(
                !refusal.contains(said),
                "a refusal never quotes what came back: {refusal}"
            );
        }
    }
}
