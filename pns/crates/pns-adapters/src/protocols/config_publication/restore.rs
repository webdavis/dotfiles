use std::path::Path;

pub(super) fn restore_after_failure(path: &Path, kept: Option<&Path>) -> String {
    let Some(backup) = kept else {
        return String::new();
    };
    // An exclusive link restores only an empty name. Retain the backup on
    // every outcome, and never replace a config another writer published.
    match std::fs::hard_link(backup, path) {
        Ok(()) => format!(
            "; the previous config was restored; its backup is kept at {}",
            backup.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => format!(
            "; the current config was left untouched; the previous config is kept at {}",
            backup.display()
        ),
        Err(error) => format!(
            "; the previous config could not be restored ({error}); it is kept at {}",
            backup.display()
        ),
    }
}

pub(super) fn secure_backup(
    path: &Path,
    backup: &Path,
    secure: impl FnOnce(&Path) -> std::io::Result<()>,
) -> Result<(), String> {
    secure(backup).map_err(|error| {
        format!(
            "{} could not be secured: {error}{}",
            backup.display(),
            restore_after_failure(path, Some(backup))
        )
    })
}

#[cfg(test)]
mod tests;
