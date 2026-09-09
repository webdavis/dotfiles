//! The digest record shared by the alert writer and the daily digest reader.
//!
//! Two processes agree on this file and never call each other: the alerter
//! appends one line per non-paging finding, and once a day the digest claims
//! the batch and reads it back. The line is all they share, so its shape lives
//! here rather than in either of them.
//!
//! SIX DERIVED FIELDS, NEVER THE WHOLE COLUMNS OBJECT. That is a privacy
//! posture, not a size one: a finding's columns can carry a hash or a secret
//! column, and the spool is a file that outlives the alert. Widening this
//! record is how that would leak, so the record is closed and every field in it
//! was chosen.
//!
//! UNVERSIONED, deliberately. There is no version field to write and none to
//! read, because a reader that met a version it did not know could only drop
//! the line, which is what it does with an unparseable one anyway. What keeps
//! the two ends agreeing is that they are built from this one crate.
//!
//! File append, claim, restore and permissions belong to adapters. Derivation,
//! grouping, sanitization and caps belong to domain policy. Notification
//! request/result envelopes remain owned by the sibling pns-protocol crate;
//! this crate neither copies nor forwards them.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One spooled finding.
///
/// EVERY FIELD IS OPTIONAL, on both sides. The writer fills all six, but the
/// reader meets whatever is on disk: a line an older writer wrote, a line
/// interrupted mid-append, a line something else put there. A record that
/// cannot represent a missing field forces the reader to invent one, and an
/// invented value is indistinguishable downstream from a real one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DigestRecord {
    pub timestamp: Option<String>,
    pub detector: Option<String>,
    pub category: Option<String>,
    pub identity: Option<String>,
    pub action: Option<String>,
    pub summary: Option<String>,
}

/// One record as the line that goes on the wire, with no trailing newline.
///
/// FIELD ORDER IS THE DECLARED ORDER, which is the order the writer this
/// replaces emitted. Nothing reads by position, so the order is cosmetic, but a
/// spool whose lines all order their keys the same way is one a human can read
/// with their eyes.
pub fn encode(record: &DigestRecord) -> String {
    serde_json::to_string(record).unwrap_or_else(|_| String::from("{}"))
}

/// Every record a spool holds, with the lines that were not records dropped.
///
/// DROPPING IS THE WHOLE POINT. A digest that refused to render because one
/// line was bad would lose the day's other findings to that one line, and the
/// interrupted append that produces a torn line is a normal event: the alerter
/// can be killed mid-write by a logout at any moment. One unreadable line costs
/// one finding.
///
/// A LINE THAT IS VALID JSON BUT NOT AN OBJECT IS ALSO DROPPED, which is the
/// one place this differs from the renderer it replaces. There, such a line
/// reached `group_by` as a bare scalar and aborted the whole render, so a
/// single `42` in the spool silently swallowed every finding that day. Dropping
/// it is the same treatment a torn line already gets, and it is what the
/// dropping was for.
pub fn decode_spool(spool: &str) -> Vec<DigestRecord> {
    spool.lines().filter_map(decode_line).collect()
}

/// One line, or nothing if it was not a record.
fn decode_line(line: &str) -> Option<DigestRecord> {
    if line.trim().is_empty() {
        return None;
    }
    let object = match serde_json::from_str::<Value>(line) {
        Ok(Value::Object(object)) => object,
        _ => return None,
    };
    let field = |name: &str| coerce(object.get(name));
    Some(DigestRecord {
        timestamp: field("timestamp"),
        detector: field("detector"),
        category: field("category"),
        identity: field("identity"),
        action: field("action"),
        summary: field("summary"),
    })
}

/// What one JSON value means as a field.
///
/// ABSENT, NULL AND `false` ARE ALL "NOT CARRIED", which looks arbitrary until
/// you read the renderer this replaces: its fallback was jq's `//`, and `//`
/// takes both null and false. A field spelled `false` has never been written by
/// anything, so preserving the quirk costs nothing and changing it would be a
/// silent behavior change in a port.
///
/// EVERY OTHER VALUE BECOMES ITS TEXT, so a number written where a string
/// belonged still renders as what it says rather than dropping the finding. A
/// string becomes itself; anything else becomes its compact JSON, exactly what
/// jq's `tostring` produced.
fn coerce(value: Option<&Value>) -> Option<String> {
    match value {
        None | Some(Value::Null) | Some(Value::Bool(false)) => None,
        Some(Value::String(text)) => Some(text.clone()),
        Some(other) => Some(other.to_string()),
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
