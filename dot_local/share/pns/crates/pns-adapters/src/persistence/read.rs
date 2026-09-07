/// One of this tool's state files read back whole, or the reason it was
/// refused: nothing at the path, something there that is not a regular file,
/// too large to pull into memory, or bytes no reader can decode.
///
/// EVERY READER OF THESE FILES GOES THROUGH IT, the prune's read-back, the
/// doctor's two sections and the presence probe alike, because a raw `read_to_string` on a path an
/// operator, a backup tool or another program can reach is the same two bugs
/// wherever it is written. A FIFO parks the open forever, for READING as much
/// as for writing, which wedges the hook that appended or the command a human
/// is waiting on. A file some other hand grew to gigabytes is otherwise
/// learned about by allocating it.
///
/// `symlink_metadata`, so the link itself is judged rather than whatever it
/// points at, matching the append's own refusal a few lines up. The SIZE IS
/// CHECKED FIRST for the reason above; `read_max` is the CALLER'S ceiling, far
/// above anything that caller writes and far below a size worth reading, so
/// only a file some other hand left there can reach it.
///
/// THE REFUSALS ARE `io::Error`s rather than an absence, so a caller that has
/// to tell "there is no file" from "the file could not be read" still can:
/// the doctor says a different sentence for each, and the prune heals on
/// either.
pub fn readable_state_file(path: &std::path::Path, read_max: u64) -> std::io::Result<String> {
    let found = std::fs::symlink_metadata(path)?;
    if !found.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the state file is not a regular file",
        ));
    }
    if found.len() > read_max {
        return Err(std::io::Error::new(
            std::io::ErrorKind::FileTooLarge,
            "the state file is larger than this reads",
        ));
    }
    std::fs::read_to_string(path)
}
