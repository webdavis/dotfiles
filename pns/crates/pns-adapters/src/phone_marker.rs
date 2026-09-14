use crate::{LoadOutcome, config_path, load_config};
use std::ffi::{CString, OsStr};
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug)]
pub struct PhoneMarkerPath {
    pub path: PathBuf,
    pub source: &'static str,
    pub config_file: PathBuf,
}

#[derive(Debug)]
pub struct TapFailure {
    pub code: &'static str,
    pub message: String,
}

impl TapFailure {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub fn phone_marker_path(
    home: &str,
    environment: Option<&OsStr>,
) -> Result<PhoneMarkerPath, TapFailure> {
    let config_file = config_path(home);
    let (path, source) = if let Some(path) = environment.filter(|path| !path.is_empty()) {
        (PathBuf::from(path), "environment")
    } else {
        let configured = match load_config(&config_file) {
            Ok(LoadOutcome::Missing) => None,
            Ok(LoadOutcome::Loaded(config)) => config.phone_marker_file,
            Err(error) => return Err(TapFailure::new("config_error", error.detail())),
        };
        match configured {
            Some(path) => {
                let path = match path.strip_prefix("~/") {
                    Some(tail) if !home.is_empty() => Path::new(home).join(tail),
                    Some(_) => return Err(TapFailure::new("path_error", "HOME is unavailable")),
                    None => PathBuf::from(path),
                };
                (path, "config")
            }
            None if !home.is_empty() => (
                Path::new(home).join(".local/state/pns/phone-attention.marker"),
                "default",
            ),
            None => return Err(TapFailure::new("path_error", "HOME is unavailable")),
        }
    };
    if path.as_os_str().as_bytes().contains(&0) {
        return Err(TapFailure::new(
            "path_error",
            "the marker path contains a null byte",
        ));
    }
    Ok(PhoneMarkerPath {
        path,
        source,
        config_file,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerReading {
    Missing,
    Present(u64),
    InvalidTimestamp,
    Unreadable(io::ErrorKind),
}

impl MarkerReading {
    pub fn mtime(self) -> Option<u64> {
        match self {
            Self::Present(time) => Some(time),
            _ => None,
        }
    }

    pub fn exists(self) -> Option<bool> {
        match self {
            Self::Missing => Some(false),
            Self::Present(_) | Self::InvalidTimestamp => Some(true),
            Self::Unreadable(_) => None,
        }
    }
}

pub fn read_phone_marker(path: &Path) -> MarkerReading {
    match fs::symlink_metadata(path) {
        Ok(metadata) => match metadata.modified() {
            Ok(time) => match time.duration_since(UNIX_EPOCH) {
                Ok(age) => MarkerReading::Present(age.as_secs()),
                Err(_) => MarkerReading::InvalidTimestamp,
            },
            Err(error) => MarkerReading::Unreadable(error.kind()),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => MarkerReading::Missing,
        Err(error) => MarkerReading::Unreadable(error.kind()),
    }
}

pub fn record_phone_tap(path: &Path) -> Result<(), TapFailure> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)
            .map_err(|error| {
                TapFailure::new(
                    "mkdir_failed",
                    format!("cannot create the marker directory {parent:?} ({error})"),
                )
            })?;
    }
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
    {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(touch_failure(path, error)),
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| touch_failure(path, error))?;
    if !(metadata.is_file() || metadata.file_type().is_symlink()) {
        return Err(TapFailure::new(
            "touch_failed",
            "the marker is not a regular file or symlink",
        ));
    }
    let name = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| TapFailure::new("path_error", "the marker path contains a null byte"))?;
    let times = [
        libc::timespec {
            tv_sec: 0,
            tv_nsec: libc::UTIME_OMIT,
        },
        libc::timespec {
            tv_sec: 0,
            tv_nsec: libc::UTIME_NOW,
        },
    ];
    // SAFETY: name is NUL-terminated and times contains two initialized entries.
    // NOFOLLOW updates the same object symlink_metadata reads, even if it is replaced.
    let result = unsafe {
        libc::utimensat(
            libc::AT_FDCWD,
            name.as_ptr(),
            times.as_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result != 0 {
        return Err(touch_failure(path, io::Error::last_os_error()));
    }
    Ok(())
}

/// The path is NAMED, because the only reader of this line is a phone that
/// showed an SSH failure: the marker it could not write is the whole diagnosis,
/// and it is a path the operator configured rather than a secret. The whole
/// error is formatted rather than its kind, which carries the errno and keeps
/// the errnos std has no variant for out of "uncategorized error".
fn touch_failure(path: &Path, error: io::Error) -> TapFailure {
    TapFailure::new(
        "touch_failed",
        format!("cannot update the tap marker {path:?} ({error})"),
    )
}
