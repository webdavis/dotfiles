//! The producer request: what any producer, in any language, tells pns about
//! one event, in version 1 of the `pns.request` envelope.
//!
//! The normalized [`State`] is what pns policy reads. Delivery scope is one
//! typed word, so the legacy pair of independent flags cannot be spelled
//! here (decision 0007). A producer states `elapsed` and pns decides the
//! tier from it; there is no field for a caller-decided tier.

use std::time::Duration;

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
    "state",
    "elapsed",
    "detail",
    "project",
    "branch",
    "pane",
    "scope",
    "route",
    "kind",
    "class",
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
///
/// ONE CLOSED SET OF SIX WORDS, AND THE SAME SET THE FLAG PATH TAKES: a
/// producer that spells `state` in JSON and one that types `--state` are
/// saying the same thing, so a word either path refuses is a word both
/// refuse. It is a plain word on the wire rather than a wrapper object,
/// because there was never a second field inside the wrapper to name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Done,
    Failed,
    Blocked,
    Resolved,
    Observation,
    Progress,
}

impl State {
    /// The state a producer spelled, or `None` for a word outside the set.
    pub fn from_word(word: &str) -> Option<Self> {
        match word {
            "done" => Some(Self::Done),
            "failed" => Some(Self::Failed),
            "blocked" => Some(Self::Blocked),
            "resolved" => Some(Self::Resolved),
            "observation" => Some(Self::Observation),
            "progress" => Some(Self::Progress),
            _ => None,
        }
    }

    /// The word this state is spelled with, on either path.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
            Self::Resolved => "resolved",
            Self::Observation => "observation",
            Self::Progress => "progress",
        }
    }

    /// Whether this state is an update nobody is waiting on, which is what
    /// makes it quiet on both paths.
    pub fn quiet(self) -> bool {
        matches!(self, Self::Observation | Self::Progress)
    }

    /// Every word `from_word` accepts, for a usage line and for a refusal.
    pub const WORDS: &'static [&'static str] = &[
        "done",
        "failed",
        "blocked",
        "resolved",
        "observation",
        "progress",
    ];
}

/// What the event IS, which is what pns maps to a route when the producer
/// named none. TWO WORDS AND NO MORE: a producer says what kind of thing
/// happened and never which route or channel it lands on, because a route is
/// one deployment's gateway and a producer is a tool other people install
/// (operator ruling, 2026-09-15).
///
/// NOT A SECOND SPELLING OF `state`. The state says how the work ended and
/// this says whose work it was, and pns needs both: a health event that is
/// done is not a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A session event: a harness hook, the shell notifier, a daemon job.
    Agent,
    /// A machine's own health, such as an unattended upgrade that failed
    /// while nobody was watching.
    Health,
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

/// One version 1 request. Construct with [`Request::new`] and set what the
/// producer knows beyond the four required parts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub request_id: RequestId,
    pub producer: Name,
    /// The producer's session, for correlation. A plain id: it is one name at
    /// the top level, the same way the flag spells it, and the turn count
    /// inside the old wrapper was stored and never read.
    #[serde(default)]
    pub session: Option<Name>,
    pub state: State,
    /// How long the work ran, written as `<count><s|m|h>`. pns decides the
    /// tier from it.
    #[serde(default, with = "elapsed")]
    pub elapsed: Option<Duration>,
    #[serde(default)]
    pub detail: String,
    /// Where the work was happening, at the top level and one field per part,
    /// because not every producer has a project, a branch or a pane.
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub pane: Option<String>,
    #[serde(default)]
    pub scope: DeliveryScope,
    #[serde(default)]
    pub route: Option<Name>,
    /// What the event is, for a producer that states it. ABSENT IS NOT
    /// `Agent`: an absent kind is a producer that said nothing, and it is
    /// omitted when encoding so the canonical bytes of a request written
    /// before this field existed do not move.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<Kind>,
    /// An operator-configured delivery class, independent of producer and route.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<Name>,
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
    pub fn new(request_id: RequestId, producer: Name, state: State) -> Self {
        Request {
            request_id,
            producer,
            session: None,
            state,
            elapsed: None,
            detail: String::new(),
            project: None,
            branch: None,
            pane: None,
            scope: DeliveryScope::default(),
            route: None,
            kind: None,
            class: None,
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

/// `elapsed` on the wire: one duration spelling, the parser every other pns
/// duration goes through, and the field naming itself in the refusal.
mod elapsed {
    use super::Duration;
    use serde::{Deserialize, Deserializer, Serializer, de};

    pub(super) fn serialize<S: Serializer>(
        value: &Option<Duration>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(duration) => serializer.serialize_str(&pns_domain::duration::spelled(*duration)),
            None => serializer.serialize_none(),
        }
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Duration>, D::Error> {
        Option::<String>::deserialize(deserializer)?
            .map(|text| {
                pns_domain::duration::parse_duration("elapsed", &text, pns_domain::elapsed::RANGE)
                    .map_err(de::Error::custom)
            })
            .transpose()
    }
}

#[cfg(test)]
mod tests;
