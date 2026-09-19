//! The producer request: what any producer, in any language, tells an engine
//! about one event, in version 1 of the `pns.request` envelope.
//!
//! The normalized [`State`] is what engine policy reads. Delivery scope is one
//! typed word, so a pair of independent flags cannot be spelled here. A
//! producer states `elapsed` and the engine decides the tier from it;
//! there is no field for a caller-decided tier.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::identifiers::{Name, RequestId};
use super::{MAX_TEXT_CHARS, Oversized, encoded};

/// The envelope's name and the one major this build speaks.
const SCHEMA: &str = "pns.request/1";

/// What happened, in the contract's own terms. A producer's event name never
/// controls routing, state or lighting directly; this does.
///
/// ONE CLOSED SET OF SIX WORDS, spelled as a plain word on the wire. posture
/// writes two of them and the other four are here because this is posture's
/// reading of the whole contract, which the golden document holds to the
/// engine's.
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
    /// The producer's session, for correlation: a plain id at the top level.
    #[serde(default)]
    pub session: Option<Name>,
    pub state: State,
    /// How long the work ran, written as `<count><s|m|h>`; the engine decides
    /// the tier from it. CARRIED AS THE TEXT IT IS ON THE WIRE, because
    /// posture measures no duration and never sends one: reading the spelling
    /// is the engine's job, and a parser here would be a second opinion about
    /// a value posture only ever writes as absent.
    #[serde(default)]
    pub elapsed: Option<String>,
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
    /// What this event is for delivery: which route it takes when it named
    /// none, and whether it passes a mute. The engine takes the same word
    /// from the `--delivery-class` flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_class: Option<Name>,
    /// Producer-specific data, carried verbatim and never read here.
    #[serde(default)]
    pub extensions: Map<String, Value>,
}

/// The request under its schema field, which lives on the wire and not on
/// the struct: the version is the envelope's, not the producer's to set.
#[derive(Serialize)]
struct Outgoing<'a> {
    schema: &'static str,
    #[serde(flatten)]
    request: &'a Request,
}

/// The same pairing on the way in. `flatten` buffers the object, so a field
/// version 1 does not define is ignored rather than refused: an older reader
/// keeps working against a newer producer.
#[cfg(test)]
#[derive(Deserialize)]
struct Incoming {
    schema: String,
    #[serde(flatten)]
    request: Request,
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
            delivery_class: None,
            extensions: Map::new(),
        }
    }

    /// The request as one JSON object, schema first.
    ///
    /// THE TEXT CAP IS CHECKED ON `detail` BY NAME. Every other string a
    /// posture request carries is a [`Name`] or a [`RequestId`], already held
    /// to its own shorter cap when it was constructed, so `detail` is the one
    /// field a caller can pass an unbounded value into and the byte cap in
    /// [`encoded`] catches the rest. A generic walk over every string at every
    /// depth is the ENGINE'S check on input it did not build.
    pub fn encode(&self) -> Result<String, Oversized> {
        if self.detail.chars().count() > MAX_TEXT_CHARS {
            return Err(Oversized);
        }
        encoded(&Outgoing {
            schema: SCHEMA,
            request: self,
        })
    }

    /// Decode one request from its bytes. posture WRITES requests and never
    /// reads one, so this is the test-only half of the pairing: it is what
    /// holds [`Request::encode`] and the golden document to the same reading.
    #[cfg(test)]
    pub(crate) fn decode(bytes: &[u8]) -> Result<Request, super::Malformed> {
        let incoming: Incoming = super::decoded(bytes)?;
        if incoming.schema != SCHEMA {
            return Err(super::Malformed);
        }
        Ok(incoming.request)
    }
}
