use posture_application::{WatchdogState, WatchdogStateFailure, WatchdogStateStore};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
mod codec;

pub struct WatchdogStateFile {
    path: PathBuf,
}
impl WatchdogStateFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    fn temporary(&self) -> Result<TemporaryState, WatchdogStateFailure> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let parent = self.path.parent().ok_or(WatchdogStateFailure)?;
        fs::create_dir_all(parent).map_err(|_| WatchdogStateFailure)?;
        let mut name = self.path.as_os_str().to_owned();
        name.push(format!(
            ".watchdog-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let path = PathBuf::from(name);
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&path)
            .map_err(|_| WatchdogStateFailure)?;
        Ok(TemporaryState { path, file })
    }
}
struct TemporaryState {
    path: PathBuf,
    file: fs::File,
}
impl Drop for TemporaryState {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
impl WatchdogStateStore for WatchdogStateFile {
    fn load(&mut self) -> WatchdogState {
        let read = || -> Option<WatchdogState> {
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(&self.path)
                .ok()?;
            if !file.metadata().ok()?.is_file() {
                return None;
            }
            let mut bytes = Vec::new();
            file.take(1_048_577).read_to_end(&mut bytes).ok()?;
            if bytes.len() > 1_048_576 {
                return None;
            }
            codec::decode(&bytes)
        };
        read().unwrap_or_default()
    }
    fn writable(&mut self) -> bool {
        !self.path.is_dir() && self.temporary().is_ok()
    }
    fn publish(&mut self, state: &WatchdogState) -> Result<(), WatchdogStateFailure> {
        let mut temporary = self.temporary()?;
        temporary
            .file
            .write_all(&codec::encode(state))
            .map_err(|_| WatchdogStateFailure)?;
        temporary
            .file
            .sync_all()
            .map_err(|_| WatchdogStateFailure)?;
        fs::rename(&temporary.path, &self.path).map_err(|_| WatchdogStateFailure)
    }
}
#[cfg(test)]
mod tests;
