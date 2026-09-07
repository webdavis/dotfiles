/// The most of the decision ring or the journal that is ever read into memory.
/// Their depths (5 and 25) at their field caps sit far under it; see
/// `missed_notifications::KEPT` for that arithmetic.
pub const RING_READ_MAX: u64 = 256 * 1024;
/// The mode every file this tool creates in its state directory is born with.
///
/// ONE RULE FOR THE DIRECTORY'S CONTENTS rather than a knob for one caller:
/// none of them has a reason to be world-readable, and the journal holds the
/// operator's own text. ACCEPTED LIMIT: an APPEND applies it at create, so a
/// ring an earlier build already left on disk keeps its umask mode until it is
/// next created, and nothing chmods a file it found there, in keeping with the
/// ring's refuse-rather-than-repair stance. THE PUBLISH IS THE ONE PLACE THAT
/// CHMODS, and it is not that case: the pending file it narrows is its own,
/// named for this process, and the rename is about to publish that file's mode
/// over the state file.
pub const STATE_FILE_MODE: u32 = 0o600;
