//! The alerter's end of the digest spool: one line appended per finding that
//! did not earn a page.
//!
//! THE OTHER END NEVER CALLS THIS. The daily digest claims the file by rename
//! and reads it; this appends to whatever file is at the path now. They agree
//! because both build their line from `posture_protocol`, not because either
//! knows the other exists.
//!
//! ONE `write` OF ONE LINE, on a descriptor opened `O_APPEND`. That is what
//! makes a torn line rare rather than routine: the kernel places the whole
//! write at the current end of file, so two alerters appending at once
//! interleave whole lines rather than halves of two. It is not a guarantee, and
//! the reader drops an unparseable line for the times it is not.
//!
//! A SPOOL FAILURE IS NOT A PAGE FAILURE. A finding that reaches here has
//! already been judged not worth waking anyone for, so a spool that cannot be
//! written costs tomorrow's summary line and must never cost this run's page or
//! its checkpoint.

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// Appends one encoded record per non-paging finding.
pub struct DigestAppendFile {
    store: PathBuf,
}

impl DigestAppendFile {
    pub fn new(store: PathBuf) -> Self {
        Self { store }
    }

    /// Append one record. Answers whether the line reached the file, for a
    /// caller that wants to say so; nothing about a page depends on it.
    pub fn append(&self, record: &posture_protocol::DigestRecord) -> bool {
        if super::prepare_spool_directory(&self.store).is_err() {
            return false;
        }
        // 0600 AT CREATION, not after. A file holding full filesystem paths must
        // never exist world-readable, not even for the moment between the two
        // calls, because that moment is when another process gets to open it.
        let Ok(mut file) = OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o600)
            .open(&self.store)
        else {
            return false;
        };
        // AND 0600 AGAIN FOR A FILE THAT ALREADY EXISTED, where `mode` said
        // nothing. This is cheap and it repairs a spool something else loosened.
        let _ = tighten(&self.store);
        let line = format!("{}\n", posture_protocol::encode(record));
        file.write_all(line.as_bytes()).is_ok()
    }
}

fn tighten(path: &Path) -> std::io::Result<()> {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(test)]
#[path = "digest_appender/tests.rs"]
mod tests;
