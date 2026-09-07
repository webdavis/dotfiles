mod input;
mod number;
mod query;
use input::ProjectionInput;
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
fn command_text(mut value: String) -> String {
    value.retain(|character| character != '\0');
    value.truncate(value.trim_end_matches('\n').len());
    value
}
pub(super) use query::identity as query_identity;

#[cfg(test)]
mod tests;
