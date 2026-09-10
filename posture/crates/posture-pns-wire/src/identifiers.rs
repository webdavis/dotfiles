//! The validated identifiers every envelope carries: the producer's request
//! id, the short names (producer, event, route, destination, session), and
//! the schema identifier with its major version. Each is a newtype so an
//! invalid one cannot be constructed, on the wire or in code.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A request id rides an HTTP `Idempotency-Key` header and an environment
/// variable, so it is visible ASCII only. 128 characters holds any UUID or
/// hash spelling with room for a producer prefix.
pub const REQUEST_ID_MAX_CHARS: usize = 128;
/// A name is shown to the operator (`agent · state · project`) and used as a
/// key. Unicode is welcome; a control character is not. 64 characters matches
/// the daemon's own job-id cap.
pub const NAME_MAX_CHARS: usize = 64;

/// Why an identifier was refused. Three states, because a producer fixing
/// its request needs to know which rule it broke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidIdentifier {
    Empty,
    TooLong { chars: usize, max: usize },
    ForbiddenCharacter,
}

impl fmt::Display for InvalidIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InvalidIdentifier::Empty => write!(f, "empty"),
            InvalidIdentifier::TooLong { chars, max } => {
                write!(f, "{chars} characters where at most {max} are allowed")
            }
            InvalidIdentifier::ForbiddenCharacter => {
                write!(f, "carries a character the identifier rules refuse")
            }
        }
    }
}

impl std::error::Error for InvalidIdentifier {}

fn validated(
    text: String,
    max: usize,
    allowed: fn(char) -> bool,
) -> Result<String, InvalidIdentifier> {
    if text.is_empty() {
        return Err(InvalidIdentifier::Empty);
    }
    let chars = text.chars().count();
    if chars > max {
        return Err(InvalidIdentifier::TooLong { chars, max });
    }
    if !text.chars().all(allowed) {
        return Err(InvalidIdentifier::ForbiddenCharacter);
    }
    Ok(text)
}

/// The producer-generated identifier of one logical submission. With the
/// producer's name it is the idempotency key: a replay carries the ORIGINAL
/// id, never a fresh one.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RequestId(String);

impl RequestId {
    pub fn new(text: impl Into<String>) -> Result<Self, InvalidIdentifier> {
        validated(text.into(), REQUEST_ID_MAX_CHARS, |c| c.is_ascii_graphic()).map(RequestId)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RequestId {
    type Error = InvalidIdentifier;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        RequestId::new(text)
    }
}

impl From<RequestId> for String {
    fn from(id: RequestId) -> String {
        id.0
    }
}

/// A short name: the producer, the source event, a route, a destination, a
/// session. Any Unicode but control characters, non-empty, at most
/// [`NAME_MAX_CHARS`] characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Name(String);

impl Name {
    pub fn new(text: impl Into<String>) -> Result<Self, InvalidIdentifier> {
        validated(text.into(), NAME_MAX_CHARS, |c| !c.is_control()).map(Name)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Name {
    type Error = InvalidIdentifier;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Name::new(text)
    }
}

impl From<Name> for String {
    fn from(name: Name) -> String {
        name.0
    }
}

/// `<name>/<major>`: which envelope, and which major version of it. There is
/// no minor on the wire, because additive change never needs one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SchemaId {
    pub(super) name: String,
    pub(super) major: u32,
}

impl SchemaId {
    /// `None` for anything but a non-empty name, one slash, and an unsigned
    /// decimal major. A sign, a fraction or a second slash is not a version.
    pub(super) fn parse(text: &str) -> Option<Self> {
        let (name, major) = text.split_once('/')?;
        if name.is_empty() || major.is_empty() || !major.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        Some(SchemaId {
            name: name.to_string(),
            major: major.parse().ok()?,
        })
    }
}

impl fmt::Display for SchemaId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.name, self.major)
    }
}

#[cfg(test)]
mod tests;
