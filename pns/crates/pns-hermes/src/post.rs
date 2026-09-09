use std::time::Duration;

/// What one signed POST came back with: a status code, or no response at
/// all, which sync mode reports as its own distinct failure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PostOutcome {
    Status(u16),
    /// A request that went out and got no HTTP status back: curl's 000.
    NoResponse,
    /// A request that could not even be attempted (a malformed URL): curl's
    /// empty status, reported with its own wording.
    NoStatus,
}

/// The POST seam: body plus signature header in, outcome out. The production
/// impl honors the deadline; a fake records everything.
pub trait SignedPost {
    /// `deadline` None means NO deadline, curl's `-m 0`: explicit caller
    /// intent, not a default.
    fn post(
        &self,
        url: &str,
        body: &str,
        signature_hex: &str,
        idempotency_key: Option<&str>,
        deadline: Option<Duration>,
    ) -> PostOutcome;
}

/// The lowercase hex HMAC-SHA256 of the body under the signing key, or None
/// when the key is empty, which is the not-set-up case.
pub fn sign(secret: &str, body: &str) -> Option<String> {
    use hmac::{Hmac, KeyInit, Mac};
    if secret.is_empty() {
        return None;
    }
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(body.as_bytes());
    Some(
        mac.finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

/// The production POST: one agent, no redirects (following one would send the
/// signed body to whatever host the gateway names), the deadline per call.
/// An HTTP error status IS the answer sync mode prints, so a status-carrying
/// error is unwrapped rather than collapsed, matching the bash's missing -f.
pub struct UreqSignedPost;

impl SignedPost for UreqSignedPost {
    fn post(
        &self,
        url: &str,
        body: &str,
        signature_hex: &str,
        idempotency_key: Option<&str>,
        deadline: Option<Duration>,
    ) -> PostOutcome {
        let mut request = ureq::Agent::config_builder()
            // None is no deadline at all, so the option passes straight
            // through rather than being defaulted back into one.
            .timeout_global(deadline)
            .max_redirects(0)
            .build()
            .new_agent()
            .post(url)
            .content_type("application/json")
            .header("X-Webhook-Signature", signature_hex);
        if let Some(key) = idempotency_key {
            request = request.header("Idempotency-Key", key);
        }
        let sent = request.send(body);
        match sent {
            Ok(response) => PostOutcome::Status(response.status().as_u16()),
            Err(ureq::Error::StatusCode(code)) => PostOutcome::Status(code),
            // The request was never put on the wire: a URI ureq refuses, or a
            // header the http crate refuses to build. Curl prints no status
            // at all for these, which is a different report from a silent
            // gateway.
            Err(ureq::Error::BadUri(_) | ureq::Error::Http(_)) => PostOutcome::NoStatus,
            Err(_) => PostOutcome::NoResponse,
        }
    }
}

#[cfg(test)]
mod tests;
