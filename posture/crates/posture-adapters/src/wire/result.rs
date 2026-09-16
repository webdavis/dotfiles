//! The result an engine answers a producer with, in version 1 of the
//! `pns.result` envelope: whether the request was taken, what each destination
//! said, and the operator's decision where one was awaited. It never echoes
//! the event's own text: a producer that wants its detail back already has it.

use serde::{Deserialize, Serialize};

use super::identifiers::{Name, RequestId};
use super::{Malformed, decoded};

/// The envelope's name and the one major this build speaks.
const SCHEMA: &str = "pns.result/1";

/// Whether the request was taken: whole, in part, or not at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Accepted,
    Degraded,
    Rejected,
}

/// The answer to an awaited decision. `NoOpinion` is the expiry, the missing
/// forwarder and the surface that declined, all of which leave the producer
/// to prompt as usual; `Answered` passes the decider's own code through
/// untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InteractionResult {
    NoOpinion,
    Answered { code: i32 },
}

/// What one destination said, as a closed set of verdicts: the variant is
/// the verdict, never a word inside a sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryOutcome {
    Delivered,
    Failed,
    Silent,
    Unlaunched,
}

/// One destination's verdict, with the sentence it offered when it offered
/// one. The note is the destination's own words about itself, never the
/// event's text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestinationOutcome {
    pub destination: Name,
    pub outcome: DeliveryOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// One version 1 result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultEnvelope {
    /// The request's own id, or `None` when the bytes never carried one.
    #[serde(default)]
    pub request_id: Option<RequestId>,
    pub status: Status,
    #[serde(default)]
    pub decision_id: Option<String>,
    #[serde(default)]
    pub interaction: Option<InteractionResult>,
    #[serde(default)]
    pub destinations: Vec<DestinationOutcome>,
    /// Stable codes the engine chose to report.
    #[serde(default)]
    pub diagnostics: Vec<String>,
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
