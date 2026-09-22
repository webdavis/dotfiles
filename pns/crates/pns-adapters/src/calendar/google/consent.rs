//! The one-time consent walk that mints the refresh token `[quiet.calendar]`
//! reads, and the only place in this crate that asks a human for anything.
//!
//! GOOGLE'S INSTALLED-APPLICATION FLOW, read from
//! <https://developers.google.com/identity/protocols/oauth2/native-app> on
//! 2026-09-21: a loopback redirect on an ephemeral port, PKCE with `S256`,
//! and `access_type=offline` with `prompt=consent`, which is what makes the
//! exchange answer with a refresh token rather than an access token alone.
//!
//! THE SCOPE IS THE NARROWEST ONE THE freeBusy QUERY TAKES,
//! `calendar.freebusy` ("view your availability"), of the four that method
//! lists. It reads no event text, which is the same property the poll relies
//! on.
//!
//! THE CREDENTIALS TRAVEL IN A REQUEST BODY AND THE URL THIS PRINTS AND
//! NOWHERE ELSE. Every refusal is a fixed sentence naming the step, so no
//! response body, client secret, authorization code or token reaches a log
//! line through one. The refresh token is printed once, to the terminal that
//! asked for it, and written nowhere.

use super::token::{TOKEN_REFUSED, form};
use super::{GOOGLE_BODY_CAP, GoogleCalendarSource};
use std::io::{BufRead, Read, Write};
use std::net::TcpListener;
use std::time::Duration;

/// Where the operator grants the consent.
const AUTHORIZATION_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
/// The one scope asked for.
const FREEBUSY_SCOPE: &str = "https://www.googleapis.com/auth/calendar.freebusy";
/// The address the redirect comes back to. LOOPBACK ONLY: the code is a
/// credential, and a listener on any other interface offers it to the
/// network.
const REDIRECT_HOST: &str = "127.0.0.1";
/// How long the redirect itself may take to arrive once the browser has
/// connected.
const REDIRECT_READ_DEADLINE: Duration = Duration::from_secs(30);
/// The most of the browser's request read. A redirect request line measures
/// hundreds of bytes.
const REDIRECT_READ_MAX: u64 = 8 * 1024;
/// How long the exchange may take.
const EXCHANGE_DEADLINE: Duration = Duration::from_secs(30);
/// The bytes a verifier and a state are each made of. Thirty-two encode to
/// forty-three characters, which is the shortest verifier the flow admits.
const RANDOM_BYTES: usize = 32;

const LISTENER_REFUSED: &str =
    "no loopback port could be opened for the redirect, so nothing was asked for";
const RANDOM_REFUSED: &str = "no random bytes could be read, so nothing was asked for";
const REDIRECT_REFUSED: &str = "the browser redirect did not arrive, so no code was exchanged";
const STATE_REFUSED: &str = "the redirect did not carry this run's state, so it was not answered";
const CODE_REFUSED: &str = "the redirect carried no authorization code, so the consent was refused";
const REFRESH_REFUSED: &str = "the exchange answered with no refresh token";

/// The consent walk over one HTTP agent and one token endpoint.
pub struct GoogleConsent {
    /// The agent the exchange rides, INJECTED for the reason the poll's is:
    /// a test hands in one wearing a scripted transport and the production
    /// pipeline runs for real.
    agent: ureq::Agent,
    token_endpoint: String,
}

impl Default for GoogleConsent {
    fn default() -> Self {
        Self::with_agent(
            GoogleCalendarSource::production_config(EXCHANGE_DEADLINE).new_agent(),
            super::TOKEN_ENDPOINT.to_string(),
        )
    }
}

impl GoogleConsent {
    /// The same walk over any agent and any token endpoint: the seam the
    /// scripted transport injects through.
    pub(crate) fn with_agent(agent: ureq::Agent, token_endpoint: String) -> Self {
        Self {
            agent,
            token_endpoint,
        }
    }

    /// One consent: the authorization URL handed to `announce` for the
    /// operator to open, the single redirect read off a loopback port, and
    /// the code exchanged for a refresh token.
    ///
    /// THE VERIFIER AND THE STATE ARE MINTED HERE, from `/dev/urandom`: a
    /// caller that chose them could reuse one, and a reused verifier is a
    /// code anybody who saw the URL can spend.
    pub fn mint(
        &self,
        client_id: &str,
        client_secret: &str,
        announce: &mut dyn FnMut(&str),
    ) -> Result<String, String> {
        let verifier = random_token()?;
        let state = random_token()?;
        self.mint_with(client_id, client_secret, &verifier, &state, announce)
    }

    /// The same walk over a stated verifier and state, which is what pins the
    /// authorization URL byte for byte.
    pub(crate) fn mint_with(
        &self,
        client_id: &str,
        client_secret: &str,
        verifier: &str,
        state: &str,
        announce: &mut dyn FnMut(&str),
    ) -> Result<String, String> {
        let listener = TcpListener::bind((REDIRECT_HOST, 0)).map_err(|_| LISTENER_REFUSED)?;
        let port = listener.local_addr().map_err(|_| LISTENER_REFUSED)?.port();
        let redirect_uri = format!("http://{REDIRECT_HOST}:{port}");
        announce(&authorization_url(
            client_id,
            &redirect_uri,
            verifier,
            state,
        ));
        let code = await_code(&listener, state)?;
        self.exchange(client_id, client_secret, &code, verifier, &redirect_uri)
    }

    /// One `authorization_code` grant, as `application/x-www-form-urlencoded`.
    fn exchange(
        &self,
        client_id: &str,
        client_secret: &str,
        code: &str,
        verifier: &str,
        redirect_uri: &str,
    ) -> Result<String, String> {
        let body = form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code_verifier", verifier),
            ("redirect_uri", redirect_uri),
        ]);
        let answer = self
            .agent
            .post(&self.token_endpoint)
            .header("content-type", "application/x-www-form-urlencoded")
            .send(body.as_str())
            .ok()
            .and_then(|mut answer| {
                answer
                    .body_mut()
                    .with_config()
                    .limit(GOOGLE_BODY_CAP)
                    .read_to_string()
                    .ok()
            })
            .ok_or(TOKEN_REFUSED)?;
        parse_refresh_token(&answer)
    }
}

/// The answer's `refresh_token`, or the one refusal that names its absence.
fn parse_refresh_token(answer: &str) -> Result<String, String> {
    let document: serde_json::Value = serde_json::from_str(answer).map_err(|_| TOKEN_REFUSED)?;
    document
        .get("refresh_token")
        .and_then(serde_json::Value::as_str)
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .ok_or_else(|| REFRESH_REFUSED.to_string())
}

/// The URL the operator opens, with every parameter percent-encoded.
fn authorization_url(client_id: &str, redirect_uri: &str, verifier: &str, state: &str) -> String {
    let query = form(&[
        ("client_id", client_id),
        ("redirect_uri", redirect_uri),
        ("response_type", "code"),
        ("scope", FREEBUSY_SCOPE),
        ("code_challenge", &challenge(verifier)),
        ("code_challenge_method", "S256"),
        ("state", state),
        ("access_type", "offline"),
        ("prompt", "consent"),
    ]);
    format!("{AUTHORIZATION_ENDPOINT}?{query}")
}

/// The `S256` challenge: the base64url, unpadded, of the verifier's SHA-256.
fn challenge(verifier: &str) -> String {
    use sha2::Digest;
    base64url(&sha2::Sha256::digest(verifier.as_bytes()))
}

/// One high-entropy token, as the unreserved characters a verifier admits.
fn random_token() -> Result<String, String> {
    let mut bytes = [0u8; RANDOM_BYTES];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(&mut bytes))
        .map_err(|_| RANDOM_REFUSED)?;
    Ok(base64url(&bytes))
}

/// base64url with no padding, which is the encoding both the challenge and
/// the two random tokens are written in.
fn base64url(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// The single redirect, answered with a page and read for its code.
///
/// ONE CONNECTION AND ONE ONLY: this is a walk an operator is sitting in
/// front of, so the listener waits for the browser they were told to open
/// and the run ends with it either way.
fn await_code(listener: &TcpListener, state: &str) -> Result<String, String> {
    let (mut stream, _) = listener.accept().map_err(|_| REDIRECT_REFUSED)?;
    stream
        .set_read_timeout(Some(REDIRECT_READ_DEADLINE))
        .map_err(|_| REDIRECT_REFUSED)?;
    let mut line = String::new();
    std::io::BufReader::new((&stream).take(REDIRECT_READ_MAX))
        .read_line(&mut line)
        .map_err(|_| REDIRECT_REFUSED)?;
    let answered = code_of(&line, state);
    // THE BROWSER IS TOLD EITHER WAY, because the operator reads the outcome
    // in the window they opened rather than only in the terminal.
    let _ = stream.write_all(page(answered.is_ok()).as_bytes());
    answered
}

/// The code the request line carries, once its state agrees with this run's.
fn code_of(request: &str, state: &str) -> Result<String, String> {
    let query = request
        .split_whitespace()
        .nth(1)
        .and_then(|target| target.split_once('?'))
        .map(|(_, query)| query)
        .unwrap_or_default();
    let stated = |name: &str| {
        query
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .find(|(key, _)| *key == name)
            .map(|(_, value)| decoded(value))
    };
    if stated("state").as_deref() != Some(state) {
        return Err(STATE_REFUSED.to_string());
    }
    stated("code")
        .filter(|code| !code.is_empty())
        .ok_or_else(|| CODE_REFUSED.to_string())
}

/// One query value, percent-decoded, with `+` read as a space.
fn decoded(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                match u8::from_str_radix(&value[index + 1..index + 3], 16) {
                    Ok(byte) => {
                        out.push(byte);
                        index += 3;
                    }
                    Err(_) => {
                        out.push(b'%');
                        index += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// What the browser window is left showing.
fn page(granted: bool) -> String {
    let body = if granted {
        "pns has the consent. Close this window and read the terminal."
    } else {
        "pns refused this redirect. Close this window and read the terminal."
    };
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/plain; charset=utf-8\r\n\
         content-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[cfg(test)]
mod tests;
