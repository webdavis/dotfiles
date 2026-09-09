use std::{
    fs::OpenOptions,
    io::{self, Read},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};
// Import never follows links or blocks opening a pipe. The existing rings keep
// their exact read ceilings; scalar parsers keep their existing acceptance.
pub(super) fn read(path: &Path, limit: Option<u64>) -> io::Result<String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular state file",
        ));
    }
    if limit.is_some_and(|limit| metadata.len() > limit) {
        return Err(io::ErrorKind::FileTooLarge.into());
    }
    let mut body = String::new();
    match limit {
        Some(limit) => {
            file.take(limit + 1).read_to_string(&mut body)?;
            if body.len() as u64 > limit {
                return Err(io::ErrorKind::FileTooLarge.into());
            }
        }
        None => {
            file.take(u64::MAX).read_to_string(&mut body)?;
        }
    }
    Ok(body)
}
