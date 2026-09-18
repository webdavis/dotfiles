//! The two file shapes a brief reads: a named file, and the newest file in a
//! directory.

use std::path::{Path, PathBuf};

/// The whole of a file, or why it could not be read.
pub fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
}

/// The most recently modified regular file in a directory.
///
/// Newest by MODIFICATION TIME rather than by name, so a recap keeps whatever
/// naming its writer chose.
pub fn newest(directory: &Path) -> Result<PathBuf, String> {
    let entries =
        std::fs::read_dir(directory).map_err(|e| format!("{}: {e}", directory.display()))?;
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        if metadata.is_file() && newest.as_ref().is_none_or(|(when, _)| modified > *when) {
            newest = Some((modified, entry.path()));
        }
    }
    newest
        .map(|(_, path)| path)
        .ok_or_else(|| format!("{} holds no file", directory.display()))
}

/// The first `limit` lines of a text, with a note when it was cut.
pub fn head(text: &str, limit: usize) -> Vec<String> {
    let mut lines: Vec<String> = text.lines().take(limit).map(str::to_string).collect();
    if text.lines().count() > limit {
        lines.push(format!("... {} more lines", text.lines().count() - limit));
    }
    lines
}

#[cfg(test)]
mod tests;
