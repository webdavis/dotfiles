use crate::legacy_json::{ProjectionFields, ProjectionInput, projected_field};
use posture_domain::{Control, ControlRecord, ControlsInput, ControlsRefusal, validate_controls};
use serde_json::value::RawValue;
use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

pub fn read_controls(path: &Path) -> Result<Vec<Control>, ControlsRefusal> {
    if !path.is_file() {
        return validate_controls(ControlsInput::Missing(&path.to_string_lossy()));
    }
    let loaded = read_file(path).and_then(|bytes| projected_controls(&bytes));
    loaded.unwrap_or_else(|| validate_controls(ControlsInput::Malformed))
}

fn read_file(path: &Path) -> Option<Vec<u8>> {
    // Bash follows a regular-file symlink. Nonblocking open also prevents a
    // replacement FIFO between the path check and open from parking the reader.
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn projected_controls(bytes: &[u8]) -> Option<Result<Vec<Control>, ControlsRefusal>> {
    // jq accepts a leading byte-order mark. The captured deployed-file reader
    // consumes exactly one complete array, including all trailing bytes.
    let input = ProjectionInput::new(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes))?;
    let rows: Vec<&RawValue> = serde_json::from_str(&input.text).ok()?;
    let projected: Vec<[String; 7]> = rows
        .iter()
        .map(|row| {
            let fields = serde_json::from_str::<ProjectionFields<'_>>(row.get()).ok();
            [
                "id",
                "tier",
                "reader",
                "expect",
                "target",
                "description",
                "remedy",
            ]
            .map(|name| {
                fields
                    .as_ref()
                    .and_then(|fields| projected_field(&input, fields, name))
                    .unwrap_or_default()
            })
        })
        .collect();
    let records: Vec<_> = projected
        .iter()
        .map(
            |[id, tier, reader, expect, target, description, remedy]| ControlRecord {
                id,
                tier,
                reader,
                expect,
                target,
                description,
                remedy,
            },
        )
        .collect();
    // Validation owns the whole set. An invalid later row never exposes the
    // valid prefix to a poller deciding which probes it may run.
    Some(validate_controls(ControlsInput::Records(&records)))
}

#[cfg(test)]
mod tests;
