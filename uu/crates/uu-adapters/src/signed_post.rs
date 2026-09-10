//! uu's own signed-POST client: the one HTTP call this tool makes.
//!
//! DELIBERATELY UU'S OWN COPY, not a dependency on pns. This used to be
//! `pns-hermes`, reached by a path climbing out of the uu workspace into its
//! sibling, and the argument written here for that was that a second copy of an
//! HMAC (hash-based message authentication code) signer is a second thing to
//! get wrong.
//!
//! That argument is overruled by what these tools ARE (operator ruling
//! 2026-09-10). uu and pns ship as separate projects, installed independently
//! with `cargo install --git`, by people who do not have this repository. A
//! cargo dependency on a sibling workspace makes uu buildable only inside this
//! checkout and would leave a dangling reference behind any split into its own
//! repository. The test is not "does this touch pns" but "does uu still build
//! with pns absent from the filesystem".
//!
//! WHAT IS SHARED AT RUNTIME IS FINE AND UNCHANGED. uu still runs the deployed
//! `pns` binary for alerts, in `delivery.rs`, the same way it would run `git`.
//! That is one tool calling another's published command line, which couples
//! nothing at build time.

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

/// The status codes that mean the record reached the gateway. WRITTEN ONCE:
/// both the sentence and the verdict read it, so the rule cannot move for one
/// and stand for the other, which would print FAILED beside a run uu counted
/// as delivered.
const DELIVERED_STATUS: std::ops::Range<u16> = 200..300;

/// Whether one answer means the record arrived.
pub fn delivered(outcome: PostOutcome) -> bool {
    matches!(outcome, PostOutcome::Status(code) if DELIVERED_STATUS.contains(&code))
}

/// The sentence uu records for one outcome.
pub fn outcome_line(outcome: PostOutcome) -> String {
    match outcome {
        PostOutcome::Status(code) if delivered(outcome) => format!("posted HTTP {code}"),
        PostOutcome::Status(code) => format!("post FAILED HTTP {code}"),
        // NAMED IN UU'S OWN WORDS. The sentences inherited from pns spoke of
        // curl, which uu has never run, and of the hermes gateway by pns's
        // config key rather than uu's `[records] url`.
        PostOutcome::NoStatus => {
            "post FAILED, the request was never made (is `[records] url` a URL?)".to_string()
        }
        PostOutcome::NoResponse => {
            "post FAILED, no response at all (is the gateway up?)".to_string()
        }
    }
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

#[cfg(test)]
#[path = "signed_post/tests.rs"]
mod tests;
