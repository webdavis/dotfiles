use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub(in crate::lanes::skills) fn destroy(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
        Ok(m) if !m.is_dir() => return Err("generation cleanup refused a non-directory".into()),
        _ => {}
    }
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("generation cleanup has no name")?;
    let garbage = path.with_file_name(format!(
        "{name}.garbage.{}.{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::rename(path, &garbage).map_err(|e| e.to_string())?;
    std::fs::remove_dir_all(garbage).map_err(|e| e.to_string())
}
