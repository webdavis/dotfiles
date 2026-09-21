//! The producer request: what any producer, in any language, tells pns about
//! one event, in version 1 of the `pns.request` envelope.
//!
//! The normalized [`State`] is what pns policy reads. Delivery scope is one
//! typed word, so the legacy pair of independent flags cannot be spelled
//! here (decision 0007). A producer states `elapsed` and pns decides the
//! tier from it; there is no field for a caller-decided tier.

use std::time::Duration;

use serde::{Deserialize, Deserializer, Serialize, de};
use serde_json::{Map, Value};

use crate::envelope::{Opened, Rejected, Rejection, encode, open};
use crate::identifiers::{Name, RequestId, SchemaId};

/// The envelope's name on the wire.
const SCHEMA_NAME: &str = "pns.request";
/// The one major this crate speaks.
const SCHEMA_MAJOR: u32 = 1;

/// Every top-level field version 1 defines, `schema` included. A key not in
/// this list is REFUSED and named, the same way the flag path refuses a word
/// that is no flag of pns's: a field pns drops is a producer saying something
/// that goes nowhere.
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
    "delivery_class",
    "remind",
    "extensions",
];

/// Every field version 1 RETIRED, paired with the one that replaced it.
///
/// The refusal names the replacement, where an unknown field is only named,
/// which is the same split the flag path makes between `--kind` and a word it
/// never defined.
const RETIRED_FIELDS: [(&str, &str); 2] = [("class", "delivery_class"), ("kind", "delivery_class")];

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

/// Where the event may go. One word, three values, no fourth for "both".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryScope {
    #[default]
    Automatic,
    LocalOnly,
    RemoteOnly,
}

/// What a producer asked for about the reminder on this request, in the same
/// three statements the flag path spells.
///
/// ONE SWITCH FOR BOTH PATHS, which is why the type lives here rather than
/// beside the parser: `--remind`, `--remind=<duration>` and `--no-remind`
/// produce this same value, so a request that states it in JSON and a hook
/// that types the flag hand one resolution one answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remind {
    /// `true`, or `--remind`: armed, at the delay config carries.
    Configured,
    /// A duration string, or `--remind=<duration>`: armed, at this delay.
    After(Duration),
    /// `false`, or `--no-remind`: disarmed, whatever config says.
    Off,
}

impl Serialize for Remind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Configured => serializer.serialize_bool(true),
            Self::Off => serializer.serialize_bool(false),
            Self::After(delay) => serializer.serialize_str(&pns_domain::duration::spelled(*delay)),
        }
    }
}

impl<'de> Deserialize<'de> for Remind {
    /// A BOOLEAN OR A DURATION, and anything else names the field in its
    /// refusal, because a producer that meant to arm a reminder and spelled
    /// the value wrong must not be delivered as one that asked for nothing.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::Bool(true) => Ok(Self::Configured),
            Value::Bool(false) => Ok(Self::Off),
            Value::String(text) => pns_domain::duration::parse_duration(
                "remind",
                &text,
                pns_domain::remind::DELAY_RANGE,
            )
            .map(Self::After)
            .map_err(de::Error::custom),
            _ => Err(de::Error::custom(
                "pns: remind is not a boolean or a duration like \"5m\"",
            )),
        }
    }
}

/// One version 1 request. Construct with [`RequestEnvelope::new`] and set
/// what the producer knows beyond the four required parts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestEnvelope {
    pub request_id: RequestId,
    pub producer: Name,
    /// The producer's session, for correlation. A plain id: it is one name at
    /// the top level, the same way the flag spells it, and the turn count
    /// inside the old wrapper was stored and never read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<Name>,
    pub state: State,
    /// How long the work ran, written as `<count><s|m|h>`. pns decides the
    /// tier from it.
    #[serde(default, with = "elapsed", skip_serializing_if = "Option::is_none")]
    pub elapsed: Option<Duration>,
    #[serde(default)]
    pub detail: String,
    /// Where the work was happening, at the top level and one field per part,
    /// because not every producer has a project, a branch or a pane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane: Option<String>,
    #[serde(default)]
    pub scope: DeliveryScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<Name>,
    /// What this event is for delivery: which route it takes when it named
    /// none, and whether it passes a mute. ONE FIELD AND ONE VOCABULARY, the
    /// same words the `--delivery-class` flag takes, because a producer
    /// stating it in JSON and one typing the flag are saying the same thing.
    /// Absent is a producer that said nothing, and it is omitted when
    /// encoding so a request that names no class keeps its canonical bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_class: Option<Name>,
    /// Whether an approval this request reports waits for a second card, and
    /// how long it waits. Absent is a producer that said nothing, which falls
    /// through to the producer's own config entry and then to off, and it is
    /// omitted when encoding so a request that says nothing keeps its
    /// canonical bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remind: Option<Remind>,
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
    request: &'a RequestEnvelope,
}

impl RequestEnvelope {
    /// A request with the four required parts set and every optional part at
    /// its default.
    pub fn new(request_id: RequestId, producer: Name, state: State) -> Self {
        RequestEnvelope {
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
            delivery_class: None,
            remind: None,
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

/// A decoded request plus the top-level fields this envelope recognizes but
/// acts on nowhere, which the result names in its own `ignored_fields` list.
/// Every field version 1 defines is acted on today, so the list is empty on
/// every accepted request; a field version 1 does not define is refused.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedRequest {
    pub request: RequestEnvelope,
    pub ignored: Vec<String>,
}

/// Decode one request from its bytes, applying every shared envelope check
/// first.
pub fn decode(bytes: &[u8]) -> Result<DecodedRequest, Rejected> {
    let Opened { value, request_id } = open(bytes, &schema())?;
    if let Some((retired, replacement)) = retired_field(&value) {
        return Err(Rejected {
            request_id,
            reason: Rejection::Invalid(format!("`{retired}` was replaced by `{replacement}`")),
        });
    }
    if let Some(unknown) = unknown_field(&value) {
        return Err(Rejected {
            request_id,
            reason: Rejection::Invalid(format!("`{unknown}` is not a field pns takes")),
        });
    }
    let request = serde_json::from_value(value).map_err(|error| Rejected {
        request_id,
        reason: Rejection::Invalid(error.to_string()),
    })?;
    Ok(DecodedRequest {
        request,
        ignored: Vec::new(),
    })
}

fn retired_field(value: &Value) -> Option<(&'static str, &'static str)> {
    let object = value.as_object()?;
    RETIRED_FIELDS
        .into_iter()
        .find(|(retired, _)| object.contains_key(*retired))
}

fn unknown_field(value: &Value) -> Option<String> {
    value
        .as_object()?
        .keys()
        .find(|key| !KNOWN_FIELDS.contains(&key.as_str()))
        .cloned()
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
