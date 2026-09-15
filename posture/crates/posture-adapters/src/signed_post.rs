//! posture's own signed-POST client: the one HTTP call it makes to deliver a
//! page itself.
//!
//! DELIBERATELY POSTURE'S OWN COPY, not a dependency on any sibling workspace
//! (operator ruling 2026-09-10). posture ships as a separate project,
//! installed with `cargo install --git` by people who do not have the
//! repository it was written in, so a cargo path climbing into a sibling would
//! make posture buildable only inside that checkout and leave a dangling
//! reference behind any split into its own repository. The test is not "does
//! this touch another tool" but "does posture still build with every sibling
//! absent from the filesystem".
//!
//! The same shape is carried by the other tools in that repository. Two copies
//! of an HMAC (hash-based message authentication code) signer is the accepted
//! price of two programs that ship independently.

use std::time::Duration;

/// What one signed POST came back with: a status code, or no response at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostOutcome {
    Status(u16),
    /// A request that went out and got no HTTP status back.
    NoResponse,
    /// A request that could not even be attempted, such as a malformed URL.
    /// Told apart from the silent-gateway case because they send the operator
    /// to different places: one is a config to fix, the other is a service to
    /// start.
    NoStatus,
}

/// The POST seam: body plus signature header in, outcome out. The production
/// implementer honours the deadline; a test double records everything.
pub trait SignedPost {
    fn post(
        &self,
        url: &str,
        body: &str,
        signature_hex: &str,
        deadline: Option<Duration>,
    ) -> PostOutcome;
}

/// The lowercase hex HMAC-SHA256 of the body under the signing key, or `None`
/// when the key is empty, which is the not-set-up case rather than a failure.
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

/// The status codes that mean the page reached the gateway.
const DELIVERED_STATUS: std::ops::Range<u16> = 200..300;

/// Whether one answer means the page arrived.
pub fn delivered(outcome: PostOutcome) -> bool {
    matches!(outcome, PostOutcome::Status(code) if DELIVERED_STATUS.contains(&code))
}

/// The production POST: no redirects, because following one would send the
/// signed body to whatever host the gateway names, and the deadline per call.
/// An HTTP error status IS the answer, so a status-carrying error is unwrapped
/// rather than collapsed into "it failed".
pub struct UreqSignedPost;

impl SignedPost for UreqSignedPost {
    fn post(
        &self,
        url: &str,
        body: &str,
        signature_hex: &str,
        deadline: Option<Duration>,
    ) -> PostOutcome {
        let sent = ureq::Agent::config_builder()
            // `None` is no deadline at all, so the option passes straight
            // through rather than being defaulted back into one.
            .timeout_global(deadline)
            .max_redirects(0)
            .build()
            .new_agent()
            .post(url)
            .content_type("application/json")
            .header("X-Webhook-Signature", signature_hex)
            .send(body);
        match sent {
            Ok(response) => PostOutcome::Status(response.status().as_u16()),
            Err(ureq::Error::StatusCode(code)) => PostOutcome::Status(code),
            // The request never reached the wire: a URI ureq refuses, or a
            // header the http crate refuses to build.
            Err(ureq::Error::BadUri(_) | ureq::Error::Http(_)) => PostOutcome::NoStatus,
            Err(_) => PostOutcome::NoResponse,
        }
    }
}
