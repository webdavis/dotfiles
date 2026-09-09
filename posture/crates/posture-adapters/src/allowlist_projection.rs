mod query;
use crate::legacy_json::{ProjectionInput, command_text};
use posture_application::SourceLine;
use serde_json::value::RawValue;
use std::collections::BTreeMap;

pub(super) fn source_line(raw: Vec<u8>) -> SourceLine {
    if raw.is_empty() || raw.starts_with(b"#") {
        return SourceLine::Preserved(raw);
    }
    let Some(input) = ProjectionInput::new(&raw) else {
        return SourceLine::Invalid;
    };
    let Ok(fields) = serde_json::from_str::<BTreeMap<String, &RawValue>>(&input.text) else {
        return SourceLine::Invalid;
    };
    let label = fields
        .get("label")
        .and_then(|value| input.scalar(value))
        .map(command_text);
    SourceLine::Object { label, raw }
}
pub(super) use query::identity as query_identity;

#[cfg(test)]
mod tests;
