use super::super::{SkillsCandidate, SkillsGenerationStore, SkillsRoster, roster};
use super::metadata;
use std::collections::BTreeMap;
use std::path::Path;

pub(super) fn fingerprint(path: &Path) -> Result<String, String> {
    let mut entries = std::fs::read_dir(path)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    entries.sort_by_key(|e| e.file_name());
    let mut bytes = Vec::new();
    for entry in entries {
        let name = entry.file_name();
        bytes.extend_from_slice(&(name.as_encoded_bytes().len() as u64).to_le_bytes());
        bytes.extend_from_slice(name.as_encoded_bytes());
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let content = if kind.is_dir() {
            bytes.push(b'd');
            fingerprint(&entry.path())?.into_bytes()
        } else if kind.is_symlink() {
            bytes.push(b'l');
            std::fs::read_link(entry.path())
                .map_err(|e| e.to_string())?
                .as_os_str()
                .as_encoded_bytes()
                .to_vec()
        } else if kind.is_file() {
            bytes.push(b'f');
            std::fs::read(entry.path()).map_err(|e| e.to_string())?
        } else {
            return Err("unsupported entry in store directory".into());
        };
        bytes.extend_from_slice(&(content.len() as u64).to_le_bytes());
        bytes.extend(content);
    }
    Ok(roster::digest(&bytes))
}
fn copy(source: &Path, target: &Path) -> Result<(), String> {
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
impl SkillsCandidate {
    pub fn absorb_store_entries(
        &self,
        store: &SkillsGenerationStore,
        roster: &SkillsRoster,
    ) -> Result<(), String> {
        let mut absorbed = BTreeMap::new();
        for name in roster.tracked_names() {
            let source = store.agents.join("skills").join(&name);
            if !std::fs::symlink_metadata(&source).is_ok_and(|m| m.is_dir()) {
                continue;
            }
            let before = fingerprint(&source)?;
            let target = self.agents().join("skills").join(&name);
            if target.exists() || target.is_symlink() {
                let saved = self.home.join("before-absorption");
                metadata::directory(&saved)?;
                std::fs::rename(&target, saved.join(&name)).map_err(|e| e.to_string())?;
            }
            copy(&source, &target)?;
            if fingerprint(&source)? != before || fingerprint(&target)? != before {
                return Err(format!("store/{name} changed during absorption"));
            }
            absorbed.insert(name, before);
        }
        metadata::write(
            &self.agents().join(".reabsorbed.json"),
            &serde_json::json!(absorbed),
        )
    }
}
