//! The producer API as posture speaks it: the request it writes and the result
//! it reads back.
//!
//! THE PRODUCER API IS A PUBLISHED BOUNDARY, not this tool's invention: a
//! producer writes one JSON request on a command's standard input and reads one
//! JSON result plus an exit code back. posture is a producer; the engine on the
//! other side is whichever command the operator configured. The two ship as
//! separate projects, so neither can compile against the other, and each
//! carries its own reading of the same bytes. The golden documents under
//! `fixtures/` are what hold the two readings together: they are the same
//! documents the engine pins, so a change on either side that moves the bytes
//! fails a test rather than a delivery.
//!
//! THE SCHEMA STRINGS ARE THE CONTRACT'S OWN NAMES and stay verbatim
//! (`pns.request/1`, `pns.result/1`). They are the identifiers version 1 of the
//! producer API was published under, so they are data on the wire rather than
//! this tool's naming, and renaming them would speak a protocol no engine
//! answers.
//!
//! THE CLIENT'S HALF ONLY. This is plain serde over the two documents posture
//! exchanges, and nothing behind them: no policy, no transport, no persistence,
//! no view of the domain model. The egress envelope the contract also defines
//! is absent, because it carries a rendered event from the engine out to a
//! delivery destination, which posture is not. The generic envelope machinery
//! an ENGINE needs is absent too (a depth-limiting parser, duplicate-field
//! refusal, a per-field structural walk, a rejection taxonomy): posture never
//! receives a request nor answers with a result, so every one of those guarded
//! a direction posture does not speak.
//!
//! Every field of both documents is still carried, including the ones posture
//! neither writes nor reads, because a wire contract read half way is how two
//! programs quietly stop agreeing, and because carrying them is what lets the
//! golden documents round trip byte for byte.
//!
//! The compatibility policy, in one place:
//!
//! - The schema identifier is `<name>/<major>`, and only major 1 is spoken.
//!   Anything else is malformed to this build. Additive change within a major
//!   does not bump it.
//! - Unknown fields in a known major are ignored, so an older engine keeps
//!   working against a newer producer.
//! - Producer-specific data goes under `extensions`, carried verbatim and
//!   never interpreted here.
//! - Text is carried verbatim inside the caps. Sanitizing is the domain's job,
//!   where the destination it is bound for is known.

mod identifiers;
mod request;
mod result;

// Only the names other modules say out loud are lifted here. The rest of both
// documents is reached through the field it sits on, which is the only way
// posture ever touches it.
pub use identifiers::{Name, RequestId};
pub use request::{Request, Signal};
pub use result::{Status, decode_result};

/// The vocabulary only a test double says out loud: posture reads a result and
/// never builds one, so naming these outside a test would be naming a
/// direction it does not speak.
#[cfg(test)]
pub use result::{DeliveryOutcome, DestinationOutcome, ResultEnvelope};

/// The largest envelope, in bytes, judged before any parsing. Sixty-four KiB
/// carries one full text field in four-byte UTF-8 with room for metadata.
pub const MAX_BYTES: usize = 65_536;
/// Characters in one text field. It is the longest text the contract carries
/// anywhere, the 8,000-character reply cap, so a request can carry what an
/// agent harness hook already carries and nothing longer.
pub const MAX_TEXT_CHARS: usize = 8_000;

/// An envelope this build will not put on the wire because it passed a cap.
/// The producer's answer to one is to send something smaller, which is why the
/// caps are the only encoding failure a validated envelope has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Oversized;

/// Bytes that are not one version 1 envelope of the expected kind: over the
/// byte cap, not JSON, the wrong schema, or a field that would not decode.
/// posture's answer is the same to all four, so they are one state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Malformed;

/// The shared encoding step: serialize, then hold the result to the byte cap.
fn encoded(value: &impl serde::Serialize) -> Result<String, Oversized> {
    // Every type here serializes; only the caps can refuse one.
    let text = serde_json::to_string(value).map_err(|_| Oversized)?;
    if text.len() > MAX_BYTES {
        return Err(Oversized);
    }
    Ok(text)
}

/// The shared decoding step: hold the input to the byte cap before parsing it.
fn decoded<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, Malformed> {
    if bytes.len() > MAX_BYTES {
        return Err(Malformed);
    }
    serde_json::from_slice(bytes).map_err(|_| Malformed)
}

#[cfg(test)]
mod tests;
