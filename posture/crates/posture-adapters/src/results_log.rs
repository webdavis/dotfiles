//! osquery's results log, and the two readings the alerter takes of it.
//!
//! ONE READING, TAKEN ONCE. The size measured here bounds the span read after
//! it, so a row osquery appends while this run is judging lands in the NEXT
//! batch instead of being consumed early and re-fired. Reading to end-of-file
//! instead would consume rows the cursor is then checkpointed past, which is
//! the same finding delivered twice.
//!
//! THE INODE IS HALF THE READING. The log rotates, and an offset alone would
//! keep reading a file that no longer exists at that path.

use posture_application::ResultsLog;
use posture_domain::LiveLog;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

pub struct ResultsFile {
    path: PathBuf,
}

impl ResultsFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ResultsLog for ResultsFile {
    fn reading(&self) -> Option<LiveLog> {
        // NO LOG IS NOT AN ERROR. osquery has not written one yet on a fresh
        // machine, and a run that refused would page about its own startup.
        let metadata = std::fs::metadata(&self.path).ok()?;
        Some(LiveLog {
            inode: metadata.ino(),
            size: metadata.size(),
        })
    }

    fn span(&self, from: u64, length: u64) -> String {
        let Ok(mut file) = File::open(&self.path) else {
            return String::new();
        };
        if file.seek(SeekFrom::Start(from)).is_err() {
            return String::new();
        }
        let mut bytes = Vec::new();
        // BOUNDED BY THE READING, not by end-of-file. `take` is what keeps a
        // concurrent append out of this batch.
        if file.take(length).read_to_end(&mut bytes).is_err() {
            return String::new();
        }
        // LOSSY ON PURPOSE. A row carrying a byte that is not UTF-8 still has
        // to reach the judge, which drops it as unparseable; refusing the whole
        // span would drop every other finding in the batch with it.
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

#[cfg(test)]
#[path = "results_log/tests.rs"]
mod tests;
