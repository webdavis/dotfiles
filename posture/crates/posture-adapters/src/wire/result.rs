//! The result an engine answers a producer with, in version 1 of the
//! `pns.result` envelope: whether the request was taken and what each
//! destination said. It never echoes the event's own text: a producer that
//! wants its detail back already has it.

use serde::{Deserialize, Serialize};

use super::identifiers::{Name, RequestId};
use super::{Malformed, decoded};

/// The envelope's name and the one major this build speaks.
const SCHEMA: &str = "pns.result/1";

/// WHAT THE REQUEST WAS DELIVERED TO, not what the engine stored: every
/// destination, some of them, none of them, or the request refused before any
/// was tried. Whether a durable row committed is a diagnostic of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Delivered,
    Partial,
    Undelivered,
    Rejected,
}

/// What one destination said, as a closed set of verdicts: the variant is
/// the verdict, never a word inside a sentence. A plain word on the wire,
/// never a one-key wrapper object, so a producer that still wraps it is
/// refused rather than read as a delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryOutcome {
    Delivered,
    Failed,
    /// The channel ran and had nothing to say, which is its ordinary success.
    Silent,
    Unlaunched,
    /// The engine never learned how the attempt ended and is still retrying it.
    Unknown,
}

/// One destination's verdict: the route it was submitted on, the sentence it
/// offered when it offered one, and when the engine will try it again. The
/// note is the destination's own words about itself, never the event's text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestinationOutcome {
    /// REQUIRED, so a producer still writing the retired `destination` fails
    /// loudly here rather than answering with a nameless leg.
    pub name: Name,
    pub outcome: DeliveryOutcome,
    /// The named route this leg was submitted on, absent on a leg submitted
    /// to the destination's own default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<Name>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Unix seconds at which the engine will retry this leg, absent when it
    /// will not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_at: Option<u64>,
}

/// One version 1 result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultEnvelope {
    /// The request's own id, or `None` when the bytes never carried one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<RequestId>,
    pub status: Status,
    /// The engine's durable row for this request, stringified, or `None` when
    /// no row committed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ledger_sequence: Option<String>,
    #[serde(default)]
    pub destinations: Vec<DestinationOutcome>,
    /// Stable codes the engine chose to report.
    #[serde(default)]
    pub diagnostics: Vec<String>,
    /// The request's own top-level fields the envelope does not define, by
    /// name, empty when it carried none.
    #[serde(default)]
    pub ignored_fields: Vec<String>,
}

#[cfg(test)]
#[derive(Serialize)]
struct Outgoing<'a> {
    schema: &'static str,
    #[serde(flatten)]
    result: &'a ResultEnvelope,
}

/// The same pairing on the way in. `flatten` buffers the object, so a field
/// version 1 does not define is ignored rather than refused.
#[derive(Deserialize)]
struct Incoming {
    schema: String,
    #[serde(flatten)]
    result: ResultEnvelope,
}

impl ResultEnvelope {
    /// The result as one JSON object, schema first. posture READS results and
    /// never writes one, so this is the test-only half of the pairing: it is
    /// what holds [`decode_result`] and the golden document to the same
    /// reading.
    #[cfg(test)]
    pub(crate) fn encode(&self) -> Result<String, super::Oversized> {
        super::encoded(&Outgoing {
            schema: SCHEMA,
            result: self,
        })
    }
}

/// Decode one result from its bytes. This is the producer's side of the
/// contract, and the one direction posture speaks in production.
pub fn decode_result(bytes: &[u8]) -> Result<ResultEnvelope, Malformed> {
    let incoming: Incoming = decoded(bytes)?;
    if incoming.schema != SCHEMA {
        return Err(Malformed);
    }
    Ok(incoming.result)
}
