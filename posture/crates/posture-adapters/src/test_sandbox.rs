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
    /// A fresh empty directory, named for its subject, this process and a
    /// counter, so that tests running in parallel never share one.
    pub(crate) fn new(subject: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "posture-{subject}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    pub(crate) fn path(&self) -> &Path {
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
