//! Opening an envelope: the checks every versioned JSON contract shares
//! before its own fields are read. Bytes first, then the parse, then the
//! structural caps, then the schema. The typed decode that follows is each
//! envelope's own.

use serde::Serialize;
use serde_json::{Map, Value};

mod json;

use crate::bounds::{Violation, check, check_bytes};
use crate::identifiers::{RequestId, SchemaId};

/// Why an envelope was refused. Each variant maps to one stable diagnostic
/// code, which is what a result carries back; the payload is for the
/// operator's log line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// A cap was passed. See [`Violation`] for which.
    Bound(Violation),
    /// Not JSON, or JSON that is not an object. The text is the parser's own.
    Malformed(String),
    /// No `schema` string.
    SchemaMissing,
    /// A `schema` that names another envelope, or parses as no schema at all.
    SchemaUnknown(String),
    /// The right envelope at a major this crate does not speak.
    MajorUnsupported(u32),
    /// The envelope opened but a field would not decode: missing, mistyped,
    /// an unknown enum word, or an identifier the rules refuse.
    Invalid(String),
}

impl Rejection {
    /// The stable diagnostic code for this rejection.
    pub fn code(&self) -> &'static str {
        match self {
            Rejection::Bound(violation) => violation.code(),
            Rejection::Malformed(_) => "malformed_json",
            Rejection::SchemaMissing => "schema_missing",
            Rejection::SchemaUnknown(_) => "schema_unknown",
            Rejection::MajorUnsupported(_) => "major_unsupported",
            Rejection::Invalid(_) => "field_invalid",
        }
    }
}

/// A refused envelope, with the request id recovered where the bytes still
/// allowed it, so the result can be correlated to the request that earned
/// it. The id is `None` when the bytes were not an object within the caps,
/// or when the id itself broke the identifier rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejected {
    pub request_id: Option<RequestId>,
    pub reason: Rejection,
}

/// An envelope that passed every shared check: the object, and its id if it
/// carried a valid one.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Opened {
    pub(super) value: Value,
    pub(super) request_id: Option<RequestId>,
}

/// Open `bytes` as the envelope `expected` names.
pub(super) fn open(bytes: &[u8], expected: &SchemaId) -> Result<Opened, Rejected> {
    check_bytes(bytes).map_err(|violation| anonymous(Rejection::Bound(violation)))?;
    let value = json::parse(bytes).map_err(anonymous)?;
    let Value::Object(object) = &value else {
        return Err(anonymous(Rejection::Malformed(
            "the envelope is not a JSON object".to_string(),
        )));
    };
    check(&value).map_err(|violation| anonymous(Rejection::Bound(violation)))?;
    let request_id = recovered_id(object);
    let refuse = |reason| Rejected {
        request_id: request_id.clone(),
        reason,
    };
    let Some(Value::String(schema)) = object.get("schema") else {
        return Err(refuse(Rejection::SchemaMissing));
    };
    let parsed = SchemaId::parse(schema)
        .filter(|parsed| parsed.name == expected.name)
        .ok_or_else(|| refuse(Rejection::SchemaUnknown(schema.clone())))?;
    if parsed.major != expected.major {
        return Err(refuse(Rejection::MajorUnsupported(parsed.major)));
    }
    Ok(Opened { value, request_id })
}

pub(super) fn encode(value: &impl Serialize, schema: &SchemaId) -> Result<String, Rejected> {
    let bytes = serde_json::to_string(value)
        .map_err(|error| anonymous(Rejection::Invalid(error.to_string())))?;
    open(bytes.as_bytes(), schema)?;
    Ok(bytes)
}

fn anonymous(reason: Rejection) -> Rejected {
    Rejected {
        request_id: None,
        reason,
    }
}

fn recovered_id(object: &Map<String, Value>) -> Option<RequestId> {
    match object.get("request_id") {
        Some(Value::String(text)) => RequestId::new(text.as_str()).ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
