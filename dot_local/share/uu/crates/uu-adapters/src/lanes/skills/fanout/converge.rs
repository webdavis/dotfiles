use super::SkillsBuildMode;
use std::collections::BTreeSet;
use std::path::Path;
fn owned(target: &Path, prefix: &str, store: &Path) -> bool {
    let Some(target) = target.to_str() else {
        return false;
    };
    let Some(store) = store.to_str() else {
        return false;
    };
    [store, prefix].iter().any(|prefix| {
        target
            .strip_prefix(&format!("{prefix}/"))
            .is_some_and(|name| {
                name.as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                    && name
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
            })
    })
}
pub(super) fn directory(
    path: &Path,
    prefix: &str,
    store: &Path,
    desired: &BTreeSet<String>,
    mode: SkillsBuildMode,
) -> Result<Vec<String>, String> {
    super::super::generation::metadata::directory(path)?;
    let mut warnings = Vec::new();
    for name in desired {
        let link = path.join(name);
        let target = Path::new(prefix).join(name);
        if link.is_symlink() {
            let old = std::fs::read_link(&link).map_err(|e| e.to_string())?;
            if old == target {
                continue;
            }
            if mode == SkillsBuildMode::Full && owned(&old, prefix, store) {
                std::fs::remove_file(&link).map_err(|e| e.to_string())?;
                std::os::unix::fs::symlink(target, &link).map_err(|e| e.to_string())?;
            } else {
                warnings.push(format!(
                    "delivery {} has an existing link; leaving it",
                    link.display()
                ));
            }
        } else if !link.exists() {
            std::os::unix::fs::symlink(target, &link).map_err(|e| e.to_string())?;
        }
    }
    for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        if name.to_str().is_some_and(|s| desired.contains(s)) {
            continue;
        }
        let target = std::fs::read_link(entry.path()).map_err(|e| e.to_string())?;
        if owned(&target, prefix, store) {
            if mode == SkillsBuildMode::Full {
                std::fs::remove_file(entry.path()).map_err(|e| e.to_string())?;
            } else {
                warnings.push(format!(
                    "delivery {} is stale; additive mode leaves it",
                    entry.path().display()
                ));
            }
        }
    }
    Ok(warnings)
}
