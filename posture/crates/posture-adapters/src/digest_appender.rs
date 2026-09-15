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
//! A CLAIM CAN RENAME THE FILE OUT FROM UNDER AN OPEN HANDLE. The digest takes
//! a batch by rename, and a descriptor opened before that rename keeps writing
//! into the renamed claim, where the line lands after the digest has read it
//! and is lost. So a write that finds a different file at the spool path when
//! it finishes is written once more, into whatever file is there now. That can
//! leave one line in two digests. Repeating a finding is cheaper than dropping
//! one, which is the whole reason for the trade.
//!
//! A SPOOL FAILURE IS NOT A PAGE FAILURE. A finding that reaches here has
//! already been judged not worth waking anyone for, so a spool that cannot be
//! written costs tomorrow's summary line and must never cost this run's page or
//! its checkpoint.

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// Appends one encoded record per non-paging finding.
pub struct DigestAppendFile {
    store: PathBuf,
}

impl DigestAppendFile {
    pub fn new(store: PathBuf) -> Self {
        Self { store }
    }

    /// Append one record, reporting failure through the caller's diagnostics
    /// sink without finding contents. Answers whether the line reached the
    /// file; nothing about a page depends on it.
    ///
    /// THE SINK IS THE CALLER'S, never `std::io::stderr()`. Every other
    /// diagnostic on this path is written through the one sink the command
    /// threads down, and a line that reaches around it is a line no test can
    /// read without a subprocess.
    pub fn append(
        &self,
        record: &posture_protocol::DigestRecord,
        diagnostics: &mut dyn Write,
    ) -> bool {
        let result = self.write_record(record);
        if result.is_err() {
            let _ = writeln!(
                diagnostics,
                "posture: could not append a digest line to {}",
                self.store.display()
            );
        }
        result.is_ok()
    }

    fn write_record(&self, record: &posture_protocol::DigestRecord) -> std::io::Result<()> {
        super::prepare_spool_directory(&self.store)?;
        let line = format!("{}\n", posture_protocol::encode(record));
        if self.append_line(line.as_bytes())? {
            // EXACTLY ONE RETRY, never a loop. A second claim landing in the
            // same window is the next digest's batch either way, and a loop
            // that kept chasing renames could spin for as long as claims keep
            // arriving while the finding it holds is already written down.
            self.append_line(line.as_bytes())?;
        }
        Ok(())
    }

    /// Append the line, answering whether the file it landed in had been
    /// renamed away from the spool path by the time the write finished.
    fn append_line(&self, line: &[u8]) -> std::io::Result<bool> {
        // 0600 AT CREATION, not after. A file holding full filesystem paths must
        // never exist world-readable, not even for the moment between the two
        // calls, because that moment is when another process gets to open it.
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o600)
            .open(&self.store)?;
        // AND 0600 AGAIN FOR A FILE THAT ALREADY EXISTED, where `mode` said
        // nothing. This is cheap and it repairs a spool something else loosened.
        let _ = tighten(&self.store);
        let written = identity(&file.metadata()?);
        file.write_all(line)?;
        // THE HANDLE IS ASKED, NOT THE PATH, for what was written to: the two
        // are the same file only while nobody renamed it.
        Ok(match std::fs::metadata(&self.store) {
            Ok(current) => identity(&current) != written,
            // A spool that is not there at all was certainly renamed away.
            Err(_) => true,
        })
    }
}

/// Which file this is, across a rename: the pair the kernel identifies it by.
fn identity(metadata: &std::fs::Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}

fn tighten(path: &Path) -> std::io::Result<()> {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(test)]
#[path = "digest_appender/tests.rs"]
mod tests;
