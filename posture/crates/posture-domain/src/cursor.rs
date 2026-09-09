//! How far the alerter has read, and what to do when that answer is missing.
//!
//! The alerter reads new rows out of osquery's results log, delivers them, and
//! only then records where it stopped. The recorded position is an inode and a
//! byte offset, because the log rotates: an offset alone would keep reading a
//! file that no longer exists, and an inode alone would re-read it from the top
//! every tick.
//!
//! A LOST CURSOR IS AN ALERTING FAILURE, not a fresh start. Seeking quietly to
//! the end of the log would make deleting one state file a way to suppress a
//! queued batch, which is the first thing anyone tampering with this machine
//! would reach for. So a missing or malformed cursor replays the WHOLE current
//! log and says out loud that it happened.
//!
//! DISPLAY-FREE AND IO-FREE. Nothing here opens the log, stats it, or writes
//! the cursor back; it is handed two readings and answers where to start.

/// Where the alerter recorded that it stopped.
///
/// Both halves are required and both must be numbers. A file holding anything
/// else is not a cursor that has been half-written, it is a cursor that cannot
/// be trusted, and the caller passes `None` for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredCursor {
    pub inode: u64,
    pub offset: u64,
}

/// The log as it is right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveLog {
    pub inode: u64,
    pub size: u64,
}

/// What this tick should read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advance {
    /// The log has not grown since the cursor. Read nothing, write nothing.
    ///
    /// WRITING NOTHING MATTERS: a tick that rewrote an unchanged cursor would
    /// turn every quiet minute into a write, and a crash mid-write would lose a
    /// position that was already correct.
    Nothing,
    /// Read from `from`, and say so loudly first when `reset` is set.
    Read { from: u64, reset: bool },
}

/// Where to start reading, given what was recorded and what the log looks like.
///
/// THE ORDER OF THESE THREE RULES IS THE BEHAVIOUR. A lost cursor is repaired
/// to "this inode, offset zero" BEFORE rotation is considered, so it reads the
/// whole current log rather than tripping the rotation rule as well; and the
/// nothing-to-do check comes LAST, so an empty log with a lost cursor is quiet
/// rather than raising an alarm about a reset that has nothing to replay.
pub fn advance(stored: Option<StoredCursor>, live: LiveLog) -> Advance {
    let (mut from, reset) = match stored {
        Some(cursor) => (cursor.offset, false),
        // A FULL REPLAY IS BOUNDED, which is what makes it safe to do on every
        // lost cursor: osquery caps the results log by rotation, and the page
        // renderer caps a batch into one page with a "and N more" marker, so
        // the worst case is one page rather than a per-finding storm.
        None => (0, true),
    };
    // A DIFFERENT INODE IS A ROTATION and a smaller file is a truncation.
    // Either way the recorded offset points into a file that is gone, so
    // reading from it would skip real rows or replay rows already delivered.
    let rotated = stored.is_some_and(|cursor| cursor.inode != live.inode);
    if rotated || live.size < from {
        from = 0;
    }
    if live.size == from {
        return Advance::Nothing;
    }
    Advance::Read { from, reset }
}

/// The cursor a state file holds, or `None` when it does not hold one.
///
/// CAPTURE THEN VALIDATE, never branch on the read. A state file missing its
/// trailing newline still yields both fields, and treating that as a failure
/// would skip a whole batch on a file that was perfectly readable.
pub fn parse(recorded: &str) -> Option<StoredCursor> {
    let mut fields = recorded.split_whitespace();
    let inode = fields.next()?.parse().ok()?;
    let offset = fields.next()?.parse().ok()?;
    // A THIRD FIELD MEANS THIS IS NOT OUR FILE. Two numbers and a third token
    // is not a cursor with something appended, it is a file whose shape we do
    // not recognize, and guessing at it is how a wrong offset gets trusted.
    if fields.next().is_some() {
        return None;
    }
    Some(StoredCursor { inode, offset })
}

/// The line a cursor is recorded as.
pub fn render(cursor: StoredCursor) -> String {
    format!("{} {}", cursor.inode, cursor.offset)
}

#[cfg(test)]
mod tests;
