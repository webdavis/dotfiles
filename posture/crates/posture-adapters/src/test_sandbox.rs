//! A temporary directory that removes itself when the test drops it.
//!
//! Adapters are the layer that touches real files, so their tests make real
//! ones. Without an owner each run leaves its directories in the system
//! temporary directory forever, and a suite that writes a multi-megabyte
//! fixture leaves that too. Nothing already in this crate's dependency set
//! offers the type, and it is small enough not to earn a new dependency in a
//! tool whose job is judging integrity.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub(crate) struct Sandbox(PathBuf);

impl Sandbox {
    /// A fresh empty directory, named for its subject, this process, a
    /// counter and the epoch nanosecond, so that tests running in parallel
    /// never share one and a RECYCLED process id never meets an earlier
    /// run's leftover: nothing prunes what the guard below misses (a test
    /// that aborts instead of unwinding), so a pid-only name eventually
    /// collides with `AlreadyExists`.
    pub(crate) fn new(subject: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "posture-{subject}-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos()),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl std::ops::Deref for Sandbox {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Sandbox {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        // Best effort on purpose: a panicking drop during a failing test would
        // abort the run and hide the assertion that actually failed.
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn two_sandboxes_made_back_to_back_never_share_a_path() {
        let one = Sandbox::new("dedup");
        let two = Sandbox::new("dedup");
        assert_ne!(one.path(), two.path());
    }
    #[test]
    fn a_sandbox_directory_is_gone_once_the_guard_drops() {
        let path = Sandbox::new("gone").path().to_path_buf();
        assert!(!path.exists());
    }
}
