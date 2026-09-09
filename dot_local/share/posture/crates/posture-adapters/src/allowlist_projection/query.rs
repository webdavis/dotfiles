use crate::legacy_json::{ProjectionFields, ProjectionInput, projected_field};
use serde_json::value::RawValue;

pub(crate) fn identity(raw: &[u8]) -> Option<(String, String)> {
    let input = ProjectionInput::new(raw)?;
    let rows: Vec<&RawValue> = serde_json::from_str(&input.text).ok()?;
    let fields: ProjectionFields<'_> = serde_json::from_str(rows.first()?.get()).ok()?;
    Some((
        projected_field(&input, &fields, "path")?,
        projected_field(&input, &fields, "program")?,
    ))
}

#[cfg(test)]
mod tests;
