use crate::legacy_json::{
    ProjectionFields, ProjectionInput, command_text, compact_row, selected_text,
};
use serde_json::value::RawValue;

pub(super) fn values(bytes: &[u8]) -> Option<([String; 3], String)> {
    let text = command_text(String::from_utf8_lossy(bytes).into_owned());
    let bytes = text.as_bytes();
    let input = ProjectionInput::new(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes))?;
    let documents = serde_json::Deserializer::from_str(&input.text)
        .into_iter::<&RawValue>()
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let mut rows = Vec::new();
    for document in documents {
        if document.get() == "null" {
            continue;
        }
        // jq's first projection refuses any non-array document. Even earlier valid
        // arrays are discarded if a later document fails that projection.
        let array: Vec<&RawValue> = serde_json::from_str(document.get()).ok()?;
        if let Some(row) = array.first().filter(|row| !empty(&input, row)) {
            rows.push(*row);
        }
    }
    let baseline_rows = rows
        .iter()
        .map(|row| compact_row(&input, row))
        .collect::<Option<Vec<_>>>()?
        .join("\n");
    let values = ["firewall", "gatekeeper", "screenlock"].map(|name| {
        let mut output = String::new();
        for row in &rows {
            // A scalar selected row reports a jq error, but later stream rows still print.
            // The shell preserves those outputs and strips trailing newlines.
            let Ok(fields) = serde_json::from_str::<ProjectionFields<'_>>(row.get()) else {
                continue;
            };
            if let Some((_, value)) = fields.0.iter().find(|(key, _)| key == name) {
                if empty(&input, value) {
                    continue;
                }
                let Some(text) = selected_text(&input, value) else {
                    break;
                };
                output.push_str(&text);
                output.push('\n');
            }
        }
        command_text(output)
    });
    Some((values, baseline_rows))
}
fn empty(input: &ProjectionInput, value: &RawValue) -> bool {
    matches!(value.get(), "null" | "false") || input.number(value) == Some("null")
}
