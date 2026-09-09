use super::{SkillsBuildMode, SkillsCandidate, SkillsRoster};
use serde_json::{Map, Value};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

fn read(path: &Path) -> Result<Value, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(serde_json::json!({})),
        Err(e) => return Err(format!("lock {}: {e}", path.display())),
    };
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("lock {}: {e}", path.display()))?;
    if !value.is_object() {
        return Err(format!("lock {} is not an object", path.display()));
    }
    Ok(value)
}
fn skills(value: &Value) -> Result<Map<String, Value>, String> {
    match value.get("skills") {
        None => Ok(Map::new()),
        Some(v) => v
            .as_object()
            .cloned()
            .ok_or_else(|| "lock skills is not an object".into()),
    }
}
pub(super) fn reconcile(
    candidate: &SkillsCandidate,
    roster: &SkillsRoster,
    mode: SkillsBuildMode,
) -> Result<(), String> {
    let path = candidate.agents().join(".skill-lock.json");
    let mut base = read(&path)?;
    let cli = read(&candidate.home.join(".local/state/skills/.skill-lock.json"))?;
    let mut merged = skills(&base)?;
    merged.extend(skills(&cli)?);
    if mode == SkillsBuildMode::Full {
        merged.retain(|name, _| roster.npx.contains_key(name));
    }
    if base.as_object().is_some_and(|o| o.is_empty()) {
        base = cli;
    }
    base["skills"] = Value::Object(merged);
    let tmp = path.with_extension("reconcile.tmp");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .map_err(|e| format!("lock temporary: {e}"))?;
    file.write_all(base.to_string().as_bytes())
        .and_then(|()| file.sync_all())
        .and_then(|()| std::fs::rename(&tmp, &path))
        .map_err(|e| format!("lock publish: {e}"))
}
