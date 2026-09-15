use posture_domain::{AuditBounds, AuditFile, AuditRefusal};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{self, Read},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    time::Instant,
};

pub(super) enum ObservedFile {
    Missing,
    Irregular,
    Unreadable,
    Regular {
        size: u64,
        mode: String,
        uid: String,
        digest: Option<String>,
    },
}
impl ObservedFile {
    pub fn borrowed(&self) -> AuditFile<'_> {
        match self {
            Self::Missing => AuditFile::Missing,
            Self::Irregular => AuditFile::Irregular,
            Self::Unreadable => AuditFile::Regular {
                size: None,
                mode: None,
                uid: None,
                digest: None,
            },
            Self::Regular {
                size,
                mode,
                uid,
                digest,
            } => AuditFile::Regular {
                size: Some(*size),
                mode: Some(mode),
                uid: Some(uid),
                digest: digest.as_deref(),
            },
        }
    }
}
pub(super) fn observe(
    path: &Path,
    bounds: AuditBounds,
    start: Instant,
    hash: bool,
) -> Result<ObservedFile, AuditRefusal> {
    let observed = match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() => return Ok(ObservedFile::Irregular),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(ObservedFile::Missing),
        Err(_) => return Ok(ObservedFile::Unreadable),
        Ok(metadata) => metadata,
    };
    let Ok(mut file) = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    else {
        return Ok(ObservedFile::Regular {
            size: observed.len(),
            mode: format!("{:04o}", observed.mode() & 0o7777),
            uid: observed.uid().to_string(),
            digest: None,
        });
    };
    let Ok(metadata) = file.metadata() else {
        return Ok(ObservedFile::Unreadable);
    };
    if !metadata.is_file() {
        return Ok(ObservedFile::Irregular);
    }
    let mut size = metadata.len();
    let mut digest = None;
    if hash && size <= bounds.bytes {
        let mut hasher = Sha256::new();
        let mut bytes = [0; 65536];
        let mut read = 0;
        loop {
            if start.elapsed().as_secs() >= bounds.seconds {
                return Err(AuditRefusal::Budget);
            }
            let length = ((bounds.bytes - read + 1) as usize).min(bytes.len());
            match file.read(&mut bytes[..length]) {
                Ok(0) => {
                    digest = Some(hex(&hasher.finalize()));
                    break;
                }
                Ok(count) => {
                    read += count as u64;
                    if read > bounds.bytes {
                        size = read;
                        break;
                    }
                    hasher.update(&bytes[..count]);
                }
                Err(_) => break,
            }
        }
    }
    Ok(ObservedFile::Regular {
        size,
        mode: format!("{:04o}", metadata.mode() & 0o7777),
        uid: metadata.uid().to_string(),
        digest,
    })
}
pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
