use super::generation::metadata;
use std::path::Path;
pub(super) fn copy(source: &Path, target: &Path) -> Result<(), String> {
    metadata::directory(target)?;
    for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let to = target.join(entry.file_name());
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_dir() {
            copy(&entry.path(), &to)?;
        } else if kind.is_symlink() {
            std::os::unix::fs::symlink(
                std::fs::read_link(entry.path()).map_err(|e| e.to_string())?,
                to,
            )
            .map_err(|e| e.to_string())?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), to).map_err(|e| e.to_string())?;
        } else {
            return Err("unsupported entry in store directory".into());
        }
    }
    Ok(())
}
