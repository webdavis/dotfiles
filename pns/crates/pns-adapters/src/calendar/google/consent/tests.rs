//! The consent walk over the scripted transport and a fake browser. NO LIVE
//! CALL IS EVER MADE from here: the token endpoint is scripted, the redirect
//! comes from a thread this file starts, and every credential below is an
//! obvious fake.

use super::*;
use crate::http_script::{LoopbackResolver, ScriptedConnector, http_ok, http_response};
use std::sync::{Arc, Mutex};

const FAKE_CLIENT_ID: &str = "not-a-real-client-id";
const FAKE_CLIENT_SECRET: &str = "not-a-real-client-secret";
const FAKE_CODE: &str = "not/a/real/code";
const FAKE_REFRESH_TOKEN: &str = "not-a-real-refresh-token";
/// A verifier of the shortest admitted length, stated rather than minted so
/// the URL below is one fixed text.
const VERIFIER: &str = "0123456789012345678901234567890123456789012";
const STATE: &str = "not-a-real-state";

/// The whole authorization URL this walk prints, with only the ephemeral port
/// left to fill in.
const GOLDEN_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth\
?client_id=not-a-real-client-id\
&redirect_uri=http%3A%2F%2F127.0.0.1%3A{port}\
&response_type=code\
&scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcalendar.freebusy\
&code_challenge=_RpfHqw8pAZIomzVUE7sjRmHSM543WVdC4o-Kc4_3C0\
&code_challenge_method=S256\
&state=not-a-real-state\
&access_type=offline\
&prompt=consent";

fn scripted(responses: &[Vec<u8>]) -> (GoogleConsent, Arc<Mutex<Vec<u8>>>) {
    let connector = ScriptedConnector::default();
    let wire = Arc::clone(&connector.wire);
    *connector.responses.lock().unwrap() = responses.iter().cloned().collect();
    let agent = ureq::Agent::with_parts(
        GoogleCalendarSource::production_config(EXCHANGE_DEADLINE),
        connector,
        LoopbackResolver,
    );
    (
        GoogleConsent::with_agent(agent, "http://localhost:9/token".to_string()),
        wire,
    )
}

fn token_answer() -> Vec<u8> {
    http_ok(&format!(
        r#"{{"access_token":"not-a-real-access-token","refresh_token":"{FAKE_REFRESH_TOKEN}","expires_in":3600}}"#
    ))
}

/// The port the announced URL told the browser to come back to.
fn port_of(url: &str) -> u16 {
    url.split("127.0.0.1%3A")
        .nth(1)
        .and_then(|tail| tail.split('&').next())
        .and_then(|port| port.parse().ok())
        .expect("the URL names the loopback port")
}

/// The browser the operator would have opened: one GET of the redirect, with
/// the code percent-encoded the way a real one arrives.
fn browser(url: &str, query: &str) {
    let port = port_of(url);
    let request = format!("GET /?{query} HTTP/1.1\r\nhost: 127.0.0.1\r\n\r\n");
    std::thread::spawn(move || {
        use std::io::Write;
        if let Ok(mut stream) = std::net::TcpStream::connect(("127.0.0.1", port)) {
            let _ = stream.write_all(request.as_bytes());
        }
    });
}

/// One walk answered by a browser stating this query and a token endpoint
/// stating these responses.
fn walk(responses: &[Vec<u8>], query: &str) -> Result<String, String> {
    let (consent, _) = scripted(responses);
    let query = query.to_string();
    consent.mint_with(
        FAKE_CLIENT_ID,
        FAKE_CLIENT_SECRET,
        VERIFIER,
        STATE,
        &mut |url| browser(url, &query),
    )
}

#[test]
fn the_walk_prints_the_pinned_url_and_mints_the_refresh_token() {
    let (consent, wire) = scripted(&[token_answer()]);
    let mut announced = String::new();
    let token = consent
        .mint_with(
            FAKE_CLIENT_ID,
            FAKE_CLIENT_SECRET,
            VERIFIER,
            STATE,
            &mut |url| {
                announced = url.to_string();
                browser(url, &format!("code=not%2Fa%2Freal%2Fcode&state={STATE}"));
            },
        )
        .expect("the documented walk mints a token");
    assert_eq!(token, FAKE_REFRESH_TOKEN);
    assert_eq!(
        announced,
        GOLDEN_URL.replace("{port}", &port_of(&announced).to_string())
    );
    let sent = String::from_utf8_lossy(&wire.lock().unwrap()).to_lowercase();
    assert!(
        sent.contains("grant_type=authorization_code")
            && sent.contains("code=not%2fa%2freal%2fcode")
            && sent.contains(&format!("code_verifier={VERIFIER}")),
        "the exchange states the grant, the decoded code and the verifier: {sent}"
    );
}

#[test]
fn a_redirect_that_does_not_carry_this_runs_state_is_refused_unexchanged() {
    assert_eq!(
        walk(&[token_answer()], "code=x&state=somebody-elses-state"),
        Err(STATE_REFUSED.to_string())
    );
}

#[test]
fn a_consent_the_operator_declined_is_refused_by_the_shape_of_its_redirect() {
    assert_eq!(
        walk(
            &[token_answer()],
            &format!("error=access_denied&state={STATE}")
        ),
        Err(CODE_REFUSED.to_string())
    );
}

#[test]
fn an_exchange_the_endpoint_refuses_says_only_that() {
    let refused = http_response(
        "400 Bad Request",
        &[("content-type", "application/json")],
        &format!(r#"{{"error":"invalid_grant","client_secret":"{FAKE_CLIENT_SECRET}"}}"#),
    );
    let outcome = walk(&[refused], &format!("code={FAKE_CODE}&state={STATE}"));
    assert_eq!(outcome, Err(TOKEN_REFUSED.to_string()));
}

#[test]
fn an_answer_carrying_only_an_access_token_is_not_a_minted_consent() {
    assert_eq!(
        walk(
            &[http_ok(r#"{"access_token":"not-a-real-access-token"}"#)],
            &format!("code={FAKE_CODE}&state={STATE}")
        ),
        Err(REFRESH_REFUSED.to_string())
    );
}

/// THE MUTANT THIS PINS: a refusal that quotes the answer, which is where a
/// refused exchange echoes the client secret back.
#[test]
fn no_refusal_carries_a_credential() {
    let secrets = [FAKE_CLIENT_ID, FAKE_CLIENT_SECRET, FAKE_CODE, VERIFIER];
    let refused = http_response(
        "400 Bad Request",
        &[("content-type", "application/json")],
        &format!(r#"{{"error":"invalid_grant","seen":"{FAKE_CLIENT_SECRET}"}}"#),
    );
    for complaint in [
        walk(&[token_answer()], "code=x&state=wrong"),
        walk(
            &[token_answer()],
            &format!("error=access_denied&state={STATE}"),
        ),
        walk(&[refused], &format!("code={FAKE_CODE}&state={STATE}")),
    ] {
        let complaint = complaint.expect_err("every walk above is refused");
        for secret in secrets {
            assert!(
                !complaint.contains(secret),
                "the refusal carries a credential: {complaint}"
            );
        }
    }
}

/// The encoding both the challenge and the two random tokens are written in,
/// over the three chunk lengths.
#[test]
fn base64url_is_unpadded_and_carries_no_character_a_url_would_escape() {
    assert_eq!(base64url(&[0xff, 0xef, 0xfe]), "_-_-");
    assert_eq!(base64url(&[0xff]), "_w");
    assert_eq!(base64url(&[0xff, 0xef]), "_-8");
}

/// A minted verifier is the length and the alphabet the flow admits.
#[test]
fn a_minted_token_is_a_verifier_the_flow_admits() {
    let token = random_token().expect("/dev/urandom reads");
    assert_eq!(token.len(), 43);
    assert!(
        token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'),
        "{token}"
    );
}
