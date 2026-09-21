//! The access token: the cache on disk, and the exchange that fills it.

use super::{GOOGLE_BODY_CAP, GoogleCalendarSource};
use std::path::Path;

/// The cached token, beside the calendar's own state file and at the same
/// 0600 mode `publish_state_line` writes everything in that directory with.
pub(super) const TOKEN_STATE: &str = "quiet-calendar-google-token";

/// The most of it any reader pulls in. A Google access token measures a few
/// hundred bytes; this is generous and still a bound.
const TOKEN_STATE_READ_MAX: u64 = 4_096;

/// HOW CLOSE TO ITS EXPIRY A CACHED TOKEN IS EXCHANGED AGAIN. A token that
/// expires while the freeBusy call is in flight costs the whole poll, and a
/// poll is two minutes apart, so a minute of slack is cheap.
const TOKEN_MARGIN_SECS: u64 = 60;

/// ONE SENTENCE FOR EVERY WAY THE EXCHANGE PRODUCES NO TOKEN: a non-2xx, a
/// body that would not read, a body carrying no `access_token`. The
/// operator's next step is the same for each, and the body is never quoted
/// because it is where a refused exchange echoes the credentials back.
pub(super) const TOKEN_REFUSED: &str = "the token exchange was refused";

impl GoogleCalendarSource {
    /// The access token this poll uses: the cached one while it is still good,
    /// and a fresh exchange otherwise.
    pub(super) fn access_token(&self, state: &Path, now: u64) -> Result<String, String> {
        if let Some(cached) = read_cached_token(state, now) {
            return Ok(cached);
        }
        let (token, expires_in) = self.exchange()?;
        // A CACHE THAT WILL NOT WRITE IS NOT THIS POLL'S FAILURE: the token in
        // hand works, and the next poll exchanges again.
        let _ = write_cached_token(state, &token, now.saturating_add(expires_in));
        Ok(token)
    }

    /// One `refresh_token` grant, as `application/x-www-form-urlencoded`.
    fn exchange(&self) -> Result<(String, u64), String> {
        let body = form(&[
            ("grant_type", "refresh_token"),
            ("client_id", &self.settings.client_id),
            ("client_secret", &self.settings.client_secret),
            ("refresh_token", &self.settings.refresh_token),
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
        parse_token_answer(&answer)
    }
}

/// The answer's `access_token` and `expires_in`, or the one refusal above.
pub(super) fn parse_token_answer(answer: &str) -> Result<(String, u64), String> {
    let document: serde_json::Value = serde_json::from_str(answer).map_err(|_| TOKEN_REFUSED)?;
    let token = document
        .get("access_token")
        .and_then(serde_json::Value::as_str)
        .filter(|token| !token.is_empty())
        .ok_or(TOKEN_REFUSED)?;
    // AN ANSWER WITHOUT `expires_in` IS A TOKEN THIS POLL USES AND NEVER
    // CACHES: zero is already inside the margin, so the next poll exchanges.
    let expires_in = document
        .get("expires_in")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    Ok((token.to_string(), expires_in))
}

/// The cached token while it is still good, `None` when there is none, the
/// line is not the one this writes, or it is inside the margin.
fn read_cached_token(state: &Path, now: u64) -> Option<String> {
    let held = crate::readable_state_file(&state.join(TOKEN_STATE), TOKEN_STATE_READ_MAX).ok()?;
    let (expiry, token) = held.lines().next()?.split_once(' ')?;
    let expiry = pns_domain::count::parse_count(expiry)?;
    (!token.is_empty() && expiry > now.saturating_add(TOKEN_MARGIN_SECS)).then(|| token.to_string())
}

/// `<expiry> <token>`, one line, the way every other state file in this
/// directory is written.
fn write_cached_token(state: &Path, token: &str, expires_at: u64) -> std::io::Result<()> {
    crate::publish_state_line(&state.join(TOKEN_STATE), &format!("{expires_at} {token}"))
}

/// A form body, with every name and value percent-encoded.
///
/// ENCODED RATHER THAN INTERPOLATED: a credential carrying `&` or `=` would
/// otherwise compose a body stating fields nobody wrote.
fn form(fields: &[(&str, &str)]) -> String {
    fields
        .iter()
        .map(|(name, value)| format!("{}={}", encoded(name), encoded(value)))
        .collect::<Vec<_>>()
        .join("&")
}

/// One field, percent-encoded: the unreserved set rides through and every
/// other byte is written `%XX`.
fn encoded(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}
