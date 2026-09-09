use posture_application::{LockRefusal, WriteLock};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

pub struct AllowlistWriteLock {
    path: PathBuf,
}
impl AllowlistWriteLock {
    pub fn new(deployed: &Path) -> Self {
        let mut path = deployed.as_os_str().to_owned();
        path.push(".lock");
        Self { path: path.into() }
    }
}
pub struct WriteGuard {
    _file: File,
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
                return Ok(WriteGuard { _file: file });
            }
            if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                return Err(LockRefusal);
            }
        }
    }
}
#[cfg(test)]
mod tests;
