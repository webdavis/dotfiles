//! The producer request: what any producer, in any language, tells pns about
//! one event, in version 1 of the `pns.request` envelope.
//!
//! The source's own event name (`event`) is carried as metadata; the
//! normalized [`Signal`] is what pns policy reads. Delivery scope is one
//! typed word, so the legacy pair of independent flags cannot be spelled
//! here (decision 0007). A producer states `elapsed_secs` and pns decides the
//! tier from it; there is no field for a caller-decided tier.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::envelope::{Opened, Rejected, Rejection, encode, open};
use crate::identifiers::{Name, RequestId, SchemaId};

/// The envelope's name on the wire.
const SCHEMA_NAME: &str = "pns.request";
/// The one major this crate speaks.
const SCHEMA_MAJOR: u32 = 1;

/// Every top-level field version 1 defines, `schema` included. A key not in
/// this list is ignored and named, never refused: additive fields from a
/// newer producer must not break an older pns.
const KNOWN_FIELDS: [&str; 15] = [
    "schema",
    "request_id",
    "producer",
    "session",
    "event",
    "signal",
    "occurred_at",
    "elapsed_secs",
    "detail",
    "context",
    "scope",
    "route",
    "class",
    "interaction",
    "extensions",
];

fn schema() -> SchemaId {
    SchemaId {
        name: SCHEMA_NAME.to_string(),
        major: SCHEMA_MAJOR,
    }
}

/// What happened, in pns's own terms. A producer's event name never controls
/// routing, state or lighting directly; this does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Signal {
    Succeeded,
    Failed,
    NeedsAttention,
    ApprovalRequested,
    Resolved,
    Observation,
    Progress,
}

/// Where the event may go. One word, three values, no fourth for "both".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryScope {
    #[default]
    Automatic,
    LocalOnly,
    RemoteOnly,
}

/// Whether the producer is waiting on an answer. `AwaitDecision` is the
/// blocking approval: the submission does not return until the operator's
/// decision arrives or the bounded wait expires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Interaction {
    #[default]
    None,
    AwaitDecision,
}

/// The producer's session, and the turn within it when the producer counts
/// turns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub id: Name,
    #[serde(default)]
    pub turn: Option<u64>,
}

/// Where the work was happening. Every part is optional because not every
/// producer has a project, a branch or a pane.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Context {
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub pane: Option<String>,
}

/// One version 1 request. Construct with [`Request::new`] and set what the
/// producer knows beyond the four required parts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub request_id: RequestId,
    pub producer: Name,
    #[serde(default)]
    pub session: Option<Session>,
    pub event: Name,
    pub signal: Signal,
    /// Epoch seconds, when the producer knows when it happened.
    #[serde(default)]
    pub occurred_at: Option<u64>,
    /// How long the work ran. pns decides the tier from it.
    #[serde(default)]
    pub elapsed_secs: Option<u64>,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub context: Context,
    #[serde(default)]
    pub scope: DeliveryScope,
    #[serde(default)]
    pub route: Option<Name>,
    /// An operator-configured delivery class, independent of producer and route.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<Name>,
    #[serde(default)]
    pub interaction: Interaction,
    /// Producer-specific data, carried verbatim and never read here.
    #[serde(default)]
    pub extensions: Map<String, Value>,
}

/// The request under its schema field, which lives on the wire and not on
/// the struct: the version is the envelope's, not the producer's to set.
#[derive(Serialize)]
struct Wire<'a> {
    schema: String,
    #[serde(flatten)]
    request: &'a Request,
}

impl Request {
    /// A request with the four required parts set and every optional part at
    /// its default.
    pub fn new(request_id: RequestId, producer: Name, event: Name, signal: Signal) -> Self {
        Request {
            request_id,
            producer,
            session: None,
            event,
            signal,
            occurred_at: None,
            elapsed_secs: None,
            detail: String::new(),
            context: Context::default(),
            scope: DeliveryScope::default(),
            route: None,
            class: None,
            interaction: Interaction::default(),
            extensions: Map::new(),
        }
    }

    /// The request as one bounded JSON object, schema first. Oversized
    /// constructed values return the same refusal as oversized input.
    pub fn encode(&self) -> Result<String, Rejected> {
        let wire = Wire {
            schema: schema().to_string(),
            request: self,
        };
        encode(&wire, &schema())
    }
}

/// A decoded request plus the top-level fields version 1 does not define,
/// so the result can name them as diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct Decoded {
    pub request: Request,
    pub ignored: Vec<String>,
}

/// Decode one request from its bytes, applying every shared envelope check
/// first.
pub fn decode(bytes: &[u8]) -> Result<Decoded, Rejected> {
    let Opened { value, request_id } = open(bytes, &schema())?;
    let ignored = ignored_fields(&value);
    let request = serde_json::from_value(value).map_err(|error| Rejected {
        request_id,
        reason: Rejection::Invalid(error.to_string()),
    })?;
    Ok(Decoded { request, ignored })
}

fn ignored_fields(value: &Value) -> Vec<String> {
    value
        .as_object()
        .map(|object| {
            object
                .keys()
                .filter(|key| !KNOWN_FIELDS.contains(&key.as_str()))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
