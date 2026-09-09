use super::generation::metadata;
use crate::lanes::changes::{Listing, section::change_section};
use std::path::Path;
use uu_domain::LaneReport;
pub(super) struct Snapshot {
    npx: Result<Listing, String>,
    clawhub: Result<Listing, String>,
}
impl Snapshot {
    pub fn read(agents: &Path) -> Self {
        Self {
            npx: npx(agents),
            clawhub: clawhub(agents),
        }
    }
    pub fn report(&self, after: &Self, report: &mut LaneReport) {
        report.noted(change_section(
            &self.npx,
            &after.npx,
            "npx-tracked skills",
            "The change unit is the skill folder hash; no version number is knowable here.",
            "reading the generation lock",
        ));
        report.noted(change_section(
            &self.clawhub,
            &after.clawhub,
            "clawhub-tracked skills",
            "The origin marker records an installed version.",
            "reading clawhub origin markers",
        ));
        report.noted(
            "These comparisons do not cover the app-owned pack or Hermes registry installs.".into(),
        );
    }
}
fn npx(agents: &Path) -> Result<Listing, String> {
    let lock = agents.join(".skills-current/.skill-lock.json");
    if !lock.exists() {
        return Ok(Listing::new());
    }
    let value = metadata::document(&lock)?;
    let rows = value
        .get("skills")
        .and_then(|v| v.as_object())
        .ok_or("lock has no skills object")?;
    Ok(rows
        .iter()
        .map(|(name, value)| {
            (
                name.clone(),
                value
                    .get("skillFolderHash")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-")
                    .into(),
            )
        })
        .collect())
}
fn clawhub(agents: &Path) -> Result<Listing, String> {
    let store = agents.join("skills");
    if !store.exists() {
        return Ok(Listing::new());
    }
    let mut result = Listing::new();
    for entry in std::fs::read_dir(store).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let origin = entry.path().join(".clawhub/origin.json");
        if !origin.exists() {
            continue;
        }
        let value = metadata::document(&origin)?;
        let version = match value.get("installedVersion") {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(v) => v.to_string(),
            None => "-".into(),
        };
        result.push((entry.file_name().to_string_lossy().into_owned(), version));
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
