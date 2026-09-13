use super::generation::metadata;
use serde_yaml_ng::{Mapping, Value};
use std::path::Path;

// Exact legacy emitter output, used only to remove our old appended blocks.
const POLICY: &str = "policy:\n  allow_implicit_invocation: false\n";
const ORIGINAL: &str = "# uu-original-openai: ";

enum Ownership {
    Upstream,
    Owned(Option<String>),
}

fn document(content: &str) -> Result<Mapping, String> {
    let mut value = if content.trim().is_empty() {
        Value::Mapping(Mapping::new())
    } else {
        serde_yaml_ng::from_str(content).map_err(|e| format!("metadata YAML: {e}"))?
    };
    value
        .apply_merge()
        .map_err(|e| format!("metadata YAML merge: {e}"))?;
    let Value::Mapping(mapping) = value else {
        return Err("metadata must be a YAML mapping".into());
    };
    if mapping.get("policy").is_some_and(|p| !p.is_mapping()) {
        return Err("metadata policy must be a YAML mapping".into());
    }
    Ok(mapping)
}

fn render(original: Option<&str>) -> Result<String, String> {
    let mut mapping = document(original.unwrap_or_default())?;
    let policy = mapping
        .entry("policy".into())
        .or_insert_with(|| Value::Mapping(Mapping::new()))
        .as_mapping_mut()
        .ok_or("metadata policy must be a YAML mapping")?;
    policy.insert("allow_implicit_invocation".into(), false.into());
    let yaml = serde_yaml_ng::to_string(&mapping).map_err(|e| e.to_string())?;
    // Keep the exact upstream bytes inside the file, not a second file that a
    // package hash would include. Stripping restores comments, aliases and spacing.
    let saved = serde_json::to_string(&original).map_err(|e| e.to_string())?;
    Ok(format!("{yaml}{ORIGINAL}{saved}\n"))
}

fn ownership(content: &str) -> Result<Ownership, String> {
    if let Some(saved) = content
        .lines()
        .last()
        .and_then(|s| s.strip_prefix(ORIGINAL))
    {
        let original: Option<String> =
            serde_json::from_str(saved).map_err(|e| format!("overlay original: {e}"))?;
        if render(original.as_deref())? != content {
            return Err("metadata changed after overlay; refusing to erase edits".into());
        }
        return Ok(Ownership::Owned(original));
    }
    // Decode only our known trailing legacy blocks, then parse the entire
    // remainder. Other duplicate keys and malformed YAML are never repaired.
    let mut stripped = content;
    while let Some(prefix) = stripped.strip_suffix(POLICY) {
        if !prefix.is_empty() && !prefix.ends_with('\n') {
            break;
        }
        stripped = prefix;
    }
    document(stripped)?;
    if stripped.len() != content.len() {
        Ok(Ownership::Owned(
            (!stripped.trim().is_empty()).then(|| stripped.to_string()),
        ))
    } else {
        Ok(Ownership::Upstream)
    }
}

pub(super) fn matches(path: &Path, on_demand: bool) -> Result<bool, String> {
    let content = read(path)?;
    // Parse the deployed document before decoding ownership: a legacy duplicate
    // must fail health/validation even if its trailing block says false.
    let mapping = document(&content)?;
    let owned = ownership(&content)?;
    Ok(if on_demand {
        mapping
            .get("policy")
            .and_then(|p| p.get("allow_implicit_invocation"))
            .and_then(Value::as_bool)
            == Some(false)
    } else {
        matches!(owned, Ownership::Upstream)
    })
}

pub(super) fn strip_owned(path: &Path) -> Result<Option<String>, String> {
    let content = read(path)?;
    let Ownership::Owned(original) = ownership(&content)? else {
        return Ok(None);
    };
    match original {
        Some(original) => metadata::write_bytes(path, original.as_bytes())?,
        None => std::fs::remove_file(path).map_err(|e| e.to_string())?,
    };
    Ok(Some(content))
}

pub(super) fn read(path: &Path) -> Result<String, String> {
    for part in [
        path.to_path_buf(),
        path.parent().ok_or("overlay has no parent")?.to_path_buf(),
        path.parent()
            .and_then(Path::parent)
            .ok_or("overlay has no skill")?
            .to_path_buf(),
    ] {
        match std::fs::symlink_metadata(&part) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(format!("overlay symlink refused: {}", part.display()));
            }
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
            _ => {}
        }
    }
    match std::fs::read_to_string(path) {
        Ok(t) => Ok(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.to_string()),
    }
}
impl super::SkillsCandidate {
    pub fn assert_overlays(&self, roster: &super::SkillsRoster) -> Result<(), String> {
        for (name, tier) in &roster.tiers {
            let skill = self.agents().join("skills").join(name);
            if !skill.exists() {
                continue;
            }
            let path = skill.join("agents/openai.yaml");
            if tier == "on-demand" {
                reassert(&path)?;
            } else if tier == "core" {
                strip_owned(&path)?;
            }
        }
        Ok(())
    }
}
pub(super) fn reassert(path: &Path) -> Result<(), String> {
    let content = read(path)?;
    let original = match ownership(&content)? {
        Ownership::Owned(original) => original,
        Ownership::Upstream => {
            if matches(path, true)? {
                return Ok(());
            }
            path.exists().then_some(content.clone())
        }
    };
    let rendered = render(original.as_deref())?;
    if rendered == content {
        return Ok(());
    }
    metadata::directory(path.parent().ok_or("overlay has no parent")?)?;
    metadata::write_bytes(path, rendered.as_bytes())
}
#[cfg(test)]
mod tests;
