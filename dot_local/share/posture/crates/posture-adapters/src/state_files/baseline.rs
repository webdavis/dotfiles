use crate::legacy_json::{ProjectionFields, ProjectionInput, compact_row, projected_field};
use posture_application::{SavedPollControl, SavedPollState};
use posture_domain::{Control, trusted_poll_baseline};
use serde_json::value::RawValue;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{fs, fs::OpenOptions, io::Read, path::Path};

pub(super) fn read(path: &Path, controls: &[Control]) -> Option<(SavedPollState, String)> {
    if !path.is_file() {
        return None;
    }
    // stat reads the link's own mode; the subsequent regular-file read follows it.
    let mode = fs::symlink_metadata(path).ok()?.permissions().mode() & 0o7777;
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
    let input = ProjectionInput::new(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes))?;
    let raw: &RawValue = serde_json::from_str(&input.text).ok()?;
    let fields: ProjectionFields<'_> = serde_json::from_str(raw.get()).ok()?;
    let values = ["firewall", "gatekeeper", "screenlock"]
        .map(|name| projected_field(&input, &fields, name).unwrap_or_default());
    trusted_poll_baseline(Some(mode), true, values.each_ref().map(String::as_str), &[])?;
    let controls = controls
        .iter()
        .map(|control| {
            let id = control.id();
            SavedPollControl {
                id: id.to_owned(),
                value: projected_field(&input, &fields, id).unwrap_or_default(),
                expect: projected_field(&input, &fields, &format!("{id}:expect"))
                    .unwrap_or_default(),
                target: projected_field(&input, &fields, &format!("{id}:target"))
                    .unwrap_or_default(),
            }
        })
        .collect();
    Some((
        SavedPollState { values, controls },
        compact_row(&input, raw)?,
    ))
}
