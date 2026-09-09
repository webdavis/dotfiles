//! Where a snapshot of the results log stops being safe to read.
//!
//! osquery writes a row and then its newline, so the last line of a snapshot
//! taken mid-write is TORN: complete JSON is possible without the newline, and
//! so is half an object. Either way the bytes after the final newline are not a
//! record yet.
//!
//! A TORN LINE IS RETAINED, NEVER SKIPPED, and never processed early. Skipping
//! it loses a finding outright. Processing it early is worse than it sounds:
//! complete JSON without its newline would be handled now and then handled
//! AGAIN once the newline lands, so one finding pages twice and the second page
//! carries a byte range the first already covered.
//!
//! THE ANSWER IS IN BYTES, because the cursor is a byte offset. A character
//! count would drift from the file position on any row carrying a multi-byte
//! path, and osquery rows carry paths.

/// The part of a snapshot that is whole records, and how many bytes that is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompleteRecords<'a> {
    /// Every complete record, ending in the final newline. Empty when the
    /// snapshot holds no newline at all.
    pub text: &'a str,
    /// How far the cursor may advance, counted in bytes from the snapshot's
    /// start.
    pub bytes: u64,
}

/// Split a snapshot at its last newline.
///
/// The trailing bytes after that newline are dropped from `text` and excluded
/// from `bytes`, so a caller that checkpoints at `bytes` re-reads the torn line
/// next time and nothing else.
pub fn complete_records(snapshot: &str) -> CompleteRecords<'_> {
    match snapshot.rfind('\n') {
        // A SNAPSHOT WITH NO NEWLINE IS ALL TORN, however much of it there is,
        // so the cursor does not move and the whole thing is read again.
        None => CompleteRecords { text: "", bytes: 0 },
        Some(last) => {
            let end = last + 1;
            CompleteRecords {
                text: &snapshot[..end],
                bytes: end as u64,
            }
        }
    }
}

#[cfg(test)]
mod tests;
