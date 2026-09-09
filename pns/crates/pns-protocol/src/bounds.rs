//! The limits enforced at the wire boundary, and nothing about what a field
//! means. One generic walk over the parsed JSON applies every cap at every
//! level, so a limit cannot be forgotten on a new field or bypassed by burying
//! the excess under `extensions`.

use serde_json::Value;

/// The largest envelope, in bytes, judged before any parsing. Sixty-four KiB
/// carries one full text field in four-byte UTF-8 with room for metadata.
/// The total byte cap still applies when multiple fields each fit their caps.
pub const MAX_BYTES: usize = 65_536;
/// Keys in one object, counted per object at every level.
pub const MAX_FIELDS: usize = 64;
/// Characters in one string, keys included. It is the longest text pns keeps
/// anywhere, the 8,000-character reply cap, so a request can carry what the
/// hooks already carry and nothing longer.
pub const MAX_TEXT_CHARS: usize = 8_000;
/// Elements in one array.
pub const MAX_ITEMS: usize = 64;
/// Nested containers, the envelope object itself being the first.
pub const MAX_DEPTH: usize = 8;

/// Which cap was passed, and by how much. The number is the measured size,
/// so the operator reading a diagnostic sees the offending figure rather than
/// the limit they already know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    Bytes { bytes: usize },
    Fields { count: usize },
    Text { chars: usize },
    Items { count: usize },
    Depth { depth: usize },
}

impl Violation {
    /// The stable diagnostic code a result carries for this violation.
    pub fn code(&self) -> &'static str {
        match self {
            Violation::Bytes { .. } => "bytes_over_cap",
            Violation::Fields { .. } => "fields_over_cap",
            Violation::Text { .. } => "text_over_cap",
            Violation::Items { .. } => "items_over_cap",
            Violation::Depth { .. } => "depth_over_cap",
        }
    }
}

/// The byte cap, applied to the raw input before it is parsed.
pub(super) fn check_bytes(bytes: &[u8]) -> Result<(), Violation> {
    if bytes.len() > MAX_BYTES {
        return Err(Violation::Bytes { bytes: bytes.len() });
    }
    Ok(())
}

/// Every structural cap, applied to the parsed value at every level. The
/// first violation found is the one reported: an object is judged by its
/// field count, then its keys, then its values in order.
pub(super) fn check(value: &Value) -> Result<(), Violation> {
    walk(value, 1)
}

/// `depth` is the number of containers enclosing `value`, itself included
/// when it is one.
fn walk(value: &Value, depth: usize) -> Result<(), Violation> {
    match value {
        Value::String(text) => text_within(text),
        Value::Object(map) => {
            depth_within(depth)?;
            if map.len() > MAX_FIELDS {
                return Err(Violation::Fields { count: map.len() });
            }
            for (key, inner) in map {
                text_within(key)?;
                walk(inner, depth + 1)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            depth_within(depth)?;
            if items.len() > MAX_ITEMS {
                return Err(Violation::Items { count: items.len() });
            }
            for inner in items {
                walk(inner, depth + 1)?;
            }
            Ok(())
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
    }
}

fn text_within(text: &str) -> Result<(), Violation> {
    let chars = text.chars().count();
    if chars > MAX_TEXT_CHARS {
        return Err(Violation::Text { chars });
    }
    Ok(())
}

fn depth_within(depth: usize) -> Result<(), Violation> {
    if depth > MAX_DEPTH {
        return Err(Violation::Depth { depth });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
