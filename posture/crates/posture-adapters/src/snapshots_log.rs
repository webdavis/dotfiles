use crate::legacy_json::ProjectionInput;
use posture_application::{SnapshotReadFailure, SnapshotsLog};
use posture_domain::CanaryEpoch;
use serde_json::value::RawValue;
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
pub struct SnapshotsFile {
    path: PathBuf,
}
impl SnapshotsFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}
impl SnapshotsLog for SnapshotsFile {
    fn newest_canary(&mut self) -> Result<Option<CanaryEpoch>, SnapshotReadFailure> {
        // Opening nonblocking prevents a replaced log FIFO from waiting for a writer.
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&self.path)
            .map_err(|_| SnapshotReadFailure)?;
        let metadata = file.metadata().map_err(|_| SnapshotReadFailure)?;
        if !metadata.is_file() {
            return Err(SnapshotReadFailure);
        }
        // Observe one byte window. Concurrent appends belong to the next observation.
        newest(BufReader::new(file.take(metadata.len())))
    }
}
fn newest(reader: impl BufRead) -> Result<Option<CanaryEpoch>, SnapshotReadFailure> {
    let mut latest = None;
    for line in reader.split(b'\n') {
        let line = line.map_err(|_| SnapshotReadFailure)?;
        if let Some(value) = projected(&line) {
            // tail -1 precedes command substitution and validation in the Bash reader.
            // A last invalid value masks an older valid row; null and torn rows emit nothing.
            let mut last_line = value.rsplit('\n').next().unwrap_or("").to_owned();
            last_line.retain(|character| character != '\0');
            latest = CanaryEpoch::parse(&last_line);
        }
    }
    Ok(latest)
}
fn projected(line: &[u8]) -> Option<String> {
    let input = ProjectionInput::new(line)?;
    let fields: BTreeMap<String, &RawValue> = serde_json::from_str(&input.text).ok()?;
    if serde_json::from_str::<String>(fields.get("name")?.get())
        .ok()?
        .as_str()
        != "heartbeat_canary"
    {
        return None;
    }
    let present = |value: &&RawValue| {
        input.number(value).is_some() || !matches!(value.get(), "null" | "false")
    };
    let value = if let Some(value) = fields.get("unixTime").copied().filter(present) {
        value
    } else {
        let rows: Vec<&RawValue> = serde_json::from_str(fields.get("snapshot")?.get()).ok()?;
        let row: BTreeMap<String, &RawValue> = serde_json::from_str(rows.first()?.get()).ok()?;
        row.get("unix_time").copied().filter(present)?
    };
    // A container emits a closing bracket/brace on its last line, never a decimal epoch.
    Some(input.scalar(value).unwrap_or_default())
}
#[cfg(test)]
mod tests;
