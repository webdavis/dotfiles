use crate::{current_uid, ssh_current_user};
use posture_application::{LockRefusal, RecordRefusal, WriteLock, WriteRecord};
use std::ffi::CStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

pub struct AllowlistWriteLock {
    path: PathBuf,
    audit: PathBuf,
}
impl AllowlistWriteLock {
    pub fn new(deployed: &Path) -> Self {
        let sibling = |suffix: &str| {
            let mut path = deployed.as_os_str().to_owned();
            path.push(suffix);
            PathBuf::from(path)
        };
        Self {
            path: sibling(".lock"),
            audit: sibling(".audit"),
        }
    }
}
pub struct WriteGuard {
    _file: File,
    audit: PathBuf,
}
impl WriteLock for AllowlistWriteLock {
    type Guard = WriteGuard;
    fn acquire(&self) -> Result<WriteGuard, LockRefusal> {
        let parent = self.path.parent().ok_or(LockRefusal)?;
        fs::create_dir_all(parent).map_err(|_| LockRefusal)?;
        let file = OpenOptions::new()
            .append(true)
            .create(true)
            .custom_flags(libc::O_CLOEXEC)
            .open(&self.path)
            .map_err(|_| LockRefusal)?;
        loop {
            // The owned descriptor stays open for the entire curation, and is closed on exec.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } == 0 {
                return Ok(WriteGuard {
                    _file: file,
                    audit: self.audit.clone(),
                });
            }
            if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                return Err(LockRefusal);
            }
        }
    }
}
/// One JSON line per write, appended and flushed to disk before the write it
/// covers is published, beside the allowlist and its lock.
impl WriteRecord for WriteGuard {
    fn record(&self, verb: &str, label: &str) -> Result<(), RecordRefusal> {
        let line = serde_json::json!({
            "time": utc_now().ok_or(RecordRefusal)?,
            "verb": verb,
            "label": label,
            "uid": current_uid(),
            "user": ssh_current_user(),
            "parent": parent_process_name(),
        });
        let mut bytes = serde_json::to_vec(&line).map_err(|_| RecordRefusal)?;
        bytes.push(b'\n');
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_CLOEXEC)
            .open(&self.audit)
            .map_err(|_| RecordRefusal)?;
        file.write_all(&bytes).map_err(|_| RecordRefusal)?;
        file.sync_all().map_err(|_| RecordRefusal)
    }
}
fn utc_now() -> Option<String> {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let timestamp: libc::time_t = seconds.try_into().ok()?;
    let mut day = std::mem::MaybeUninit::<libc::tm>::uninit();
    // gmtime_r writes a complete tm into our exclusive output when it returns non-null.
    if unsafe { libc::gmtime_r(&timestamp, day.as_mut_ptr()) }.is_null() {
        return None;
    }
    let day = unsafe { day.assume_init() };
    let mut bytes = [0_u8; 32];
    // strftime writes at most the supplied buffer size and NUL-terminates it.
    let size = unsafe {
        libc::strftime(
            bytes.as_mut_ptr().cast(),
            bytes.len(),
            c"%Y-%m-%dT%H:%M:%SZ".as_ptr(),
            &day,
        )
    };
    (size != 0).then(|| String::from_utf8_lossy(&bytes[..size]).into_owned())
}
/// The name of the process that invoked this one, where the kernel still has it.
fn parent_process_name() -> Option<String> {
    let mut bytes = [0_u8; 256];
    // proc_name writes at most the supplied buffer size and NUL-terminates it.
    let size = unsafe {
        libc::proc_name(
            libc::getppid(),
            bytes.as_mut_ptr().cast(),
            bytes.len() as u32,
        )
    };
    if size <= 0 {
        return None;
    }
    let name = CStr::from_bytes_until_nul(&bytes).ok()?.to_str().ok()?;
    (!name.is_empty()).then(|| name.to_owned())
}
#[cfg(test)]
mod tests;
