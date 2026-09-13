use super::{KnownGoodManifests, OwnedTriage};
use posture_domain::{UpgradeRecordRefusal, recorded_hash, upgrade_correlation};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

const UPGRADE_BYTES: u64 = 262_144;

pub fn file_integrity_triage(
    manifests: &KnownGoodManifests,
    upgrade_record: &Path,
    target: &str,
    now: Option<u64>,
    diagnostics: &mut impl Write,
) -> OwnedTriage {
    OwnedTriage {
        recorded: manifest_hash(manifests.governing(target), target),
        ondisk: disk_hash(Path::new(target)),
        upgrade: upgrade_line(upgrade_record, target, now, diagnostics),
    }
}

fn manifest_hash(manifest: &Path, target: &str) -> String {
    let text = regular_file(manifest).and_then(|mut file| {
        let mut text = String::new();
        file.read_to_string(&mut text)?;
        Ok(text)
    });
    match text {
        Ok(text) if !text.is_empty() => {
            recorded_hash(&text, target).unwrap_or_else(|| "not in the manifest".into())
        }
        _ => "manifest unreadable".into(),
    }
}

fn disk_hash(path: &Path) -> String {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_symlink() => return "a symbolic link".into(),
        Ok(metadata) if !metadata.is_file() => return "not a regular file".into(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return "absent".into(),
        Err(_) => return "unreadable".into(),
        _ => {}
    }
    let digest = || -> io::Result<String> {
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::other("not a regular file"));
        }
        let mut hash = Sha256::new();
        let mut buffer = [0; 16_384];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hash.update(&buffer[..read]);
        }
        Ok(hash.finalize()[..6]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    };
    digest().unwrap_or_else(|_| "unreadable".into())
}

fn regular_file(path: &Path) -> io::Result<File> {
    // Nonblocking open, then fstat: a replacement FIFO cannot hold the alert lock.
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("not a regular file"));
    }
    Ok(file)
}

fn upgrade_line(
    path: &Path,
    target: &str,
    now: Option<u64>,
    diagnostics: &mut impl Write,
) -> String {
    let file = match regular_file(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return "no upgrade record on this machine".into();
        }
        Err(_) => return refused(diagnostics, "not a readable regular file"),
    };
    let mut snapshot = Vec::new();
    if file
        .take(UPGRADE_BYTES + 1)
        .read_to_end(&mut snapshot)
        .is_err()
        || snapshot.len() as u64 > UPGRADE_BYTES
    {
        return refused(diagnostics, "exceeds the read bound or could not be read");
    }
    let snapshot = match std::str::from_utf8(&snapshot) {
        Ok(text) if !text.contains('\0') => text,
        _ => return refused(diagnostics, "not valid text"),
    };
    let basename = target.rsplit('/').next().unwrap_or_default();
    match upgrade_correlation(snapshot, basename, now) {
        Ok(line) => line,
        Err(reason) => refused(
            diagnostics,
            match reason {
                UpgradeRecordRefusal::Header => "missing or malformed run timestamp",
                UpgradeRecordRefusal::ClockUnavailable => "current time unavailable",
                UpgradeRecordRefusal::TooManyRows => "more than 500 rows",
                UpgradeRecordRefusal::Row => "malformed package row",
            },
        ),
    }
}

fn refused(diagnostics: &mut impl Write, reason: &str) -> String {
    let _ = writeln!(
        diagnostics,
        "posture triage: upgrade record refused ({reason}); this page carries no correlation"
    );
    "the upgrade record could not be read".into()
}

#[cfg(test)]
mod tests;
