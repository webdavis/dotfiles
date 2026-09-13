use posture_domain::{AuditBounds, AuditRefusal, ManifestAuthority, manifest_trustworthy};
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Read},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    time::Instant,
};

pub(super) struct ManifestLines {
    reader: BufReader<File>,
    entries: u32,
}
impl ManifestLines {
    pub fn open(path: &Path, authority: ManifestAuthority) -> Result<Self, AuditRefusal> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| AuditRefusal::Missing)?;
        let metadata = file.metadata().map_err(|_| AuditRefusal::Missing)?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err(AuditRefusal::Missing);
        }
        if !manifest_trustworthy(
            authority,
            Some(&metadata.uid().to_string()),
            Some((metadata.mode() & 0o7777) as u16),
        ) {
            return Err(AuditRefusal::Untrustworthy);
        }
        Ok(Self {
            reader: BufReader::new(file),
            entries: 0,
        })
    }
    pub fn next(
        &mut self,
        bounds: AuditBounds,
        start: Instant,
    ) -> Result<Option<String>, AuditRefusal> {
        let mut bytes = Vec::new();
        // A tuple path cannot consume unbounded memory before its grammar is checked.
        self.reader
            .by_ref()
            .take(16_385)
            .read_until(b'\n', &mut bytes)
            .map_err(|_| AuditRefusal::Missing)?;
        if bytes.is_empty() {
            return Ok(None);
        }
        self.entries += 1;
        if self.entries > bounds.entries {
            return Err(AuditRefusal::Overlong);
        }
        if start.elapsed().as_secs() >= bounds.seconds {
            return Err(AuditRefusal::Budget);
        }
        if bytes.len() > 16_384 {
            return Err(AuditRefusal::Malformed);
        }
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
        }
        String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| AuditRefusal::Malformed)
    }
}
