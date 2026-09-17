//! What one inbound webhook request is, decided before anything reads it.
//!
//! THE BODY IS NEVER PARSED. The push transport is a doorbell rather than a
//! second source: a verified delivery makes the poll happen now, and the poll
//! is what reads GitHub. So the only question this module answers is whether
//! the bytes on the socket really came from GitHub, which keeps an untrusted
//! JSON parser off the one path the internet can reach.
//!
//! SYNTAX AND THE SIGNATURE, and nothing about what to do next: the caller
//! owns the socket and the poll.

use hmac::{KeyInit, Mac};

/// The one path a delivery may arrive on. Anything else is a refusal, so a
/// scanner walking the hostname meets a 404 rather than a signature check.
pub const WEBHOOK_PATH: &str = "/webhooks/github";

/// The signature header GitHub states: "This is the HMAC hex digest of the
/// request body, and is generated using the SHA-256 hash function."
const SIGNATURE_HEADER: &str = "x-hub-signature-256";

/// The prefix that digest carries: `"sha256=" + hexdigest`.
const SIGNATURE_PREFIX: &str = "sha256=";

/// The header naming the event, required so a request that is not a delivery
/// at all is refused before the secret is touched.
const EVENT_HEADER: &str = "x-github-event";

/// The header naming the delivery, required for the same reason.
const DELIVERY_HEADER: &str = "x-github-delivery";

/// The most body this reads.
///
/// ONE MEBIBYTE against GitHub's own "Payloads are capped at 25 MB", because
/// nothing here reads the body and a doorbell does not need the whole push: a
/// delivery over this is refused and the SCHEDULED POLL still reports it,
/// which is the floor every other failure falls back to as well. The bound is
/// what keeps a request the internet can reach from choosing this process's
/// allocation.
pub const WEBHOOK_BODY_MAX: usize = 1024 * 1024;

/// What one request is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// Really from GitHub. Ring the doorbell.
    Verified,
    /// Not, for this reason, which is logged and never answered with: a
    /// refusal that told the sender WHICH check it failed is a probe oracle.
    Refused(&'static str),
}

impl Delivery {
    /// The status one verdict answers with.
    ///
    /// 204 FOR A DELIVERY, because GitHub reads any 2xx as accepted and there
    /// is nothing to say back. 403 for everything else, including a request
    /// whose shape was wrong: one answer for every refusal is one fewer thing
    /// a prober learns.
    pub fn status(self) -> u16 {
        match self {
            Delivery::Verified => 204,
            Delivery::Refused(_) => 403,
        }
    }
}

/// What this request is, given the secret the webhook was created with.
///
/// THE SIGNATURE IS CHECKED IN CONSTANT TIME, through `hmac`'s own
/// `verify_slice`, which the documentation asks for by name: "Never use a
/// plain `==` operator."
///
/// AN EMPTY SECRET VERIFIES NOTHING. A receiver armed without one would accept
/// whatever arrived, so it is a refusal here rather than a check that passes
/// trivially.
pub fn delivery(request: &[u8], secret: &str) -> Delivery {
    if secret.is_empty() {
        return Delivery::Refused("no webhook secret is configured");
    }
    let Some(head_end) = head_end(request) else {
        return Delivery::Refused("no header block");
    };
    let head = String::from_utf8_lossy(&request[..head_end]);
    let mut lines = head.lines();
    let Some(request_line) = lines.next() else {
        return Delivery::Refused("no request line");
    };
    let mut words = request_line.split(' ');
    if words.next() != Some("POST") {
        return Delivery::Refused("not a POST");
    }
    if words.next() != Some(WEBHOOK_PATH) {
        return Delivery::Refused("not the webhook path");
    }
    let headers: Vec<(String, &str)> = lines.filter_map(header).collect();
    let stated = |wanted: &str| {
        headers
            .iter()
            .find(|(name, _)| name == wanted)
            .map(|(_, value)| *value)
    };
    if stated(EVENT_HEADER).is_none() || stated(DELIVERY_HEADER).is_none() {
        return Delivery::Refused("not a GitHub delivery");
    }
    let Some(signature) = stated(SIGNATURE_HEADER)
        .and_then(|value| value.strip_prefix(SIGNATURE_PREFIX))
        .and_then(hex)
    else {
        return Delivery::Refused("no usable signature header");
    };
    let body = &request[head_end..];
    if body.len() > WEBHOOK_BODY_MAX {
        return Delivery::Refused("body over the ceiling");
    }
    match signed(secret, body, &signature) {
        true => Delivery::Verified,
        false => Delivery::Refused("the signature does not match"),
    }
}

/// Whether this body carries this digest under this secret, compared in
/// constant time.
fn signed(secret: &str, body: &[u8], digest: &[u8]) -> bool {
    let Ok(mut mac) = hmac::Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(digest).is_ok()
}

/// Where the body starts, or nothing when the header block never ended.
pub fn head_end(request: &[u8]) -> Option<usize> {
    request
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| position + 4)
}

/// One header line, name lowercased so the match is case-insensitive the way
/// HTTP states it.
fn header(line: &str) -> Option<(String, &str)> {
    let (name, value) = line.split_once(':')?;
    Some((name.trim().to_ascii_lowercase(), value.trim()))
}

/// One hex digest as bytes, or nothing when it is not one.
fn hex(text: &str) -> Option<Vec<u8>> {
    (text.len().is_multiple_of(2) && !text.is_empty()).then_some(())?;
    text.as_bytes()
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
        .collect()
}

/// How many body bytes this request's `Content-Length` states.
///
/// THE READER'S OWN BOUND, and it is applied to the header rather than to
/// what arrived: a request claiming more than the ceiling is refused without
/// reading it, so nothing allocates on a number a stranger chose.
pub fn content_length(head: &str) -> Option<usize> {
    head.lines()
        .filter_map(header)
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, value)| value.parse().ok())
}

#[cfg(test)]
#[path = "webhook/tests.rs"]
mod webhook_tests;
