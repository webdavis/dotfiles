use crate::legacy_json::{ProjectionFields, ProjectionInput, command_text};
use posture_domain::{AllowFunnel, FunnelReadFailure, FunnelReading, classify_funnel};
use serde_json::value::RawValue;

pub(super) fn read(bytes: &[u8]) -> Result<FunnelReading, FunnelReadFailure> {
    let text = command_text(String::from_utf8_lossy(bytes).into_owned());
    if text.is_empty() {
        return Err(FunnelReadFailure::Empty);
    }
    let bytes = text.as_bytes();
    let input = ProjectionInput::new(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes))
        .ok_or(FunnelReadFailure::InvalidJson)?;
    let documents = serde_json::Deserializer::from_str(&input.text)
        .into_iter::<&RawValue>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| FunnelReadFailure::InvalidJson)?;
    // The Bash classifier emits one word per document; zero or multiple words cannot match its case.
    let [document] = documents.as_slice() else {
        return Err(FunnelReadFailure::UnexpectedShape);
    };
    let mut pending = vec![*document];
    let mut maps = Vec::new();
    while let Some(value) = pending.pop() {
        match value.get().as_bytes().first() {
            Some(b'[') => pending.extend(
                serde_json::from_str::<Vec<&RawValue>>(value.get())
                    .map_err(|_| FunnelReadFailure::InvalidJson)?,
            ),
            Some(b'{') => {
                let fields: ProjectionFields<'_> = serde_json::from_str(value.get())
                    .map_err(|_| FunnelReadFailure::InvalidJson)?;
                for (key, value) in &fields.0 {
                    if key != "AllowFunnel" || matches!(value.get(), "null" | "false") {
                        continue;
                    }
                    let entries: ProjectionFields<'_> = serde_json::from_str(value.get())
                        .map_err(|_| FunnelReadFailure::UnexpectedShape)?;
                    maps.push(
                        entries
                            .0
                            .into_iter()
                            .map(|(key, value)| {
                                (
                                    key,
                                    match value.get() {
                                        "true" => Some(true),
                                        "false" => Some(false),
                                        _ => None,
                                    },
                                )
                            })
                            .collect::<Vec<_>>(),
                    );
                }
                pending.extend(fields.0.into_iter().map(|(_, value)| value));
            }
            _ => {}
        }
    }
    let borrowed: Vec<Vec<_>> = maps
        .iter()
        .map(|entries| {
            entries
                .iter()
                .map(|(key, value)| (key.as_str(), *value))
                .collect()
        })
        .collect();
    let values: Vec<_> = borrowed
        .iter()
        .map(|entries| AllowFunnel::Map(entries))
        .collect();
    match classify_funnel(&[&values]) {
        FunnelReading::Gap => Err(FunnelReadFailure::UnexpectedShape),
        reading => Ok(reading),
    }
}
