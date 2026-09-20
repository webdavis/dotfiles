//! The result pns answers a producer with, in version 1 of the `pns.result`
//! envelope: whether the request was taken and what each destination said.
//! It never echoes the event's own text: a producer that wants its detail
//! back already has it.

use serde::{Deserialize, Serialize};

use crate::bounds::MAX_ITEMS;
use crate::envelope::{Opened, Rejected, Rejection, encode, open};
use crate::identifiers::{Name, RequestId, SchemaId};

/// The envelope's name on the wire.
const SCHEMA_NAME: &str = "pns.result";
/// The one major this crate speaks.
const SCHEMA_MAJOR: u32 = 1;

fn schema() -> SchemaId {
    SchemaId {
        name: SCHEMA_NAME.to_string(),
        major: SCHEMA_MAJOR,
    }
}

/// WHAT THE REQUEST WAS DELIVERED TO, not what was stored: every destination,
/// some of them, none of them, or the request refused before any was tried.
/// Whether the ledger row committed is a diagnostic of its own, so a committed
/// row that reached nobody reads as `Undelivered` rather than as a success.
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
/// never a one-key wrapper object, because there is no second field to name
/// inside the wrapper.
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
    pub name: Name,
    pub outcome: DeliveryOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// One version 1 result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultEnvelope {
    /// The request's own id, or `None` when the bytes never yielded one.
    #[serde(default)]
    pub request_id: Option<RequestId>,
    pub status: Status,
    /// The ledger row this request committed as, stringified, or `None` when
    /// no row committed.
    #[serde(default)]
    pub ledger_sequence: Option<String>,
    #[serde(default)]
    pub destinations: Vec<DestinationOutcome>,
    /// Stable codes, at most [`MAX_ITEMS`] of them on the wire.
    #[serde(default)]
    pub diagnostics: Vec<String>,
    /// The request's own top-level fields this envelope does not define, by
    /// name, empty when it carried none. Advisory like the diagnostics, and
    /// bounded the same way, but a list of names rather than of codes.
    #[serde(default)]
    pub ignored_fields: Vec<String>,
}

#[derive(Serialize)]
struct Wire<'a> {
    schema: String,
    #[serde(flatten)]
    result: &'a ResultEnvelope,
}

impl ResultEnvelope {
    /// The result for a request that was refused at the envelope: rejected,
    /// correlated where the id could be recovered, with the one code.
    pub fn rejected(rejected: &Rejected) -> Self {
        ResultEnvelope {
            request_id: rejected.request_id.clone(),
            status: Status::Rejected,
            ledger_sequence: None,
            destinations: Vec::new(),
            diagnostics: vec![rejected.reason.code().to_string()],
            ignored_fields: Vec::new(),
        }
    }

    /// The result as one JSON object, schema first, diagnostics and ignored
    /// field names each bounded at the item cap. Other bound violations return
    /// a refusal; destination outcomes are never silently discarded.
    pub fn encode(&self) -> Result<String, Rejected> {
        let mut bounded = self.clone();
        bounded.diagnostics.truncate(MAX_ITEMS);
        bounded.ignored_fields.truncate(MAX_ITEMS);
        let wire = Wire {
            schema: schema().to_string(),
            result: &bounded,
        };
        encode(&wire, &schema())
    }
}

/// Decode one result from its bytes, applying every shared envelope check
/// first. This is the producer's side of the contract.
pub fn decode(bytes: &[u8]) -> Result<ResultEnvelope, Rejected> {
    let Opened { value, request_id } = open(bytes, &schema())?;
    serde_json::from_value(value).map_err(|error| Rejected {
        request_id,
        reason: Rejection::Invalid(error.to_string()),
    })
}

#[cfg(test)]
mod tests;
