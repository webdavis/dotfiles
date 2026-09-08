use super::SkillsBuildMode;
use serde_json::{Value, json};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) fn segment(s: &str) -> bool {
    !s.is_empty() && s != "." && s != ".." && !s.contains('/') && !s.chars().any(char::is_control)
}
pub(super) fn directory(p: &Path) -> Result<(), String> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(p)
        .map_err(|e| e.to_string())
}
pub(super) fn write(p: &Path, value: &Value) -> Result<(), String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let temporary = p.with_extension(format!(
        "{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let result = || {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        f.write_all(value.to_string().as_bytes())?;
        f.sync_all()?;
        std::fs::rename(&temporary, p)
    };
    result().map_err(|e: std::io::Error| e.to_string())
}
pub(super) fn document(p: &Path) -> Result<Value, String> {
    let bytes = std::fs::read(p).map_err(|e| format!("metadata {}: {e}", p.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("metadata {}: {e}", p.display()))
}
pub(super) fn field(v: &Value, key: &str) -> Result<String, String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("metadata has no valid `{key}`"))
}
pub(super) struct Metadata {
    pub id: String,
    pub roster_hash: String,
    pub updater_hash: String,
    pub mode: SkillsBuildMode,
}
impl Metadata {
    pub fn read(p: &Path) -> Result<Self, String> {
        let v = document(&p.join("generation.json"))?;
        let id = field(&v, "id")?;
        if !segment(&id) {
            return Err("metadata has unsafe id".into());
        }
        field(&v, "createdAt")?;
        let mode = match field(&v, "buildMode")?.as_str() {
            "full" => SkillsBuildMode::Full,
            "additive" => SkillsBuildMode::Additive,
            _ => return Err("metadata has unknown buildMode".into()),
        };
        Ok(Self {
            id,
            roster_hash: field(&v, "customLockHash")?,
            updater_hash: field(&v, "updaterHash")?,
            mode,
        })
    }
    pub fn write(&self, p: &Path, created_at: &str) -> Result<(), String> {
        if !segment(&self.id) || created_at.is_empty() {
            return Err("invalid generation identity".into());
        }
        write(
            &p.join("generation.json"),
            &json!({"id":self.id,"createdAt":created_at,"customLockHash":self.roster_hash,"updaterHash":self.updater_hash,"buildMode":match self.mode {SkillsBuildMode::Full=>"full",SkillsBuildMode::Additive=>"additive"}}),
        )
    }
}

pub(super) fn remove_empty(path: &Path) -> Result<(), String> {
    match std::fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
