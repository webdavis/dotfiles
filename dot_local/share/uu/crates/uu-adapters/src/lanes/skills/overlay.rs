use std::path::Path;

pub(super) const POLICY: &str = "policy:\n  allow_implicit_invocation: false\n";
pub(super) fn strip_owned(path: &Path) -> Result<Option<String>, String> {
    let original = read(path)?;
    if !original.contains(POLICY) {
        return Ok(None);
    }
    let stripped = original.replace(POLICY, "");
    if stripped.trim().is_empty() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    } else {
        std::fs::write(path, stripped).map_err(|e| e.to_string())?;
    }
    Ok(Some(original))
}

pub(super) fn read(path: &Path) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(t) => Ok(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.to_string()),
    }
}
pub(super) fn reassert(path: &Path) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let content = read(path)?;
    if content.contains(POLICY) {
        return Ok(());
    }
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path.parent().ok_or("overlay has no parent")?)
        .map_err(|e| e.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| e.to_string())?;
    if !content.is_empty() && !content.ends_with('\n') {
        file.write_all(b"\n").map_err(|e| e.to_string())?;
    }
    file.write_all(POLICY.as_bytes()).map_err(|e| e.to_string())
}
