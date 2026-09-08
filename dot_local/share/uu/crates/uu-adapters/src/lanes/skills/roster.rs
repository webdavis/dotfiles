use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
mod schema;
pub use schema::HermesRegistryEntry;
use schema::{profiles, registry, table, text};

#[derive(Debug)]
pub struct SkillsRoster {
    pub npx: BTreeMap<String, String>,
    pub clawhub: BTreeMap<String, (String, String)>,
    pub tiers: BTreeMap<String, String>,
    pub hermes_profiles: BTreeMap<String, Vec<String>>,
    pub hermes_registry: BTreeMap<String, HermesRegistryEntry>,
    pub claude_undelivered: BTreeSet<String>,
    pub hash: String,
    source: PathBuf,
}
impl SkillsRoster {
    pub fn read(path: &Path) -> Result<Self, String> {
        let read = || {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if !value.is_object() || value.get("version").and_then(Value::as_u64) != Some(2) {
                return Err("expected roster version 2".into());
            }
            let npx = table(&value, "npxTracked")?
                .into_iter()
                .map(|(name, row)| Ok((name, text(&row, "repo")?)))
                .collect::<Result<_, String>>()?;
            let clawhub = table(&value, "clawhubTracked")?
                .into_iter()
                .map(|(name, row)| {
                    let slug = text(&row, "slug")?;
                    if slug.split('/').any(|part| !schema::segment(part)) {
                        return Err(format!("unsafe clawhub slug `{slug}`"));
                    }
                    Ok((name, (slug, text(&row, "registry")?)))
                })
                .collect::<Result<_, String>>()?;
            let tiers = table(&value, "tiers")?
                .into_iter()
                .map(|(name, row)| match row.as_str() {
                    Some(tier @ ("core" | "on-demand")) => Ok((name, tier.into())),
                    _ => Err(format!("invalid tier for `{name}`")),
                })
                .collect::<Result<_, String>>()?;
            let hermes_profiles: BTreeMap<_, _> = table(&value, "hermesProfiles")?
                .into_iter()
                .map(|(name, row)| Ok((name, profiles(&row)?)))
                .collect::<Result<_, String>>()?;
            let hermes_registry = registry(&value)?;
            for name in hermes_registry.keys() {
                if hermes_profiles.get(name).is_some_and(|p| !p.is_empty()) {
                    return Err(format!(
                        "`{name}` is in hermesRegistry and non-empty hermesProfiles"
                    ));
                }
            }
            let claude_undelivered = table(&value, "claudeDelivery")?
                .into_iter()
                .map(|(name, row)| {
                    if row.as_str() != Some("none") {
                        return Err(format!("invalid claudeDelivery for `{name}`"));
                    }
                    Ok(name)
                })
                .collect::<Result<_, String>>()?;
            let roster = Self {
                npx,
                clawhub,
                tiers,
                hermes_profiles,
                hermes_registry,
                claude_undelivered,
                hash: digest(&bytes),
                source: path.into(),
            };
            if roster.tracked_names().is_empty() {
                return Err("roster tracks zero skills; refusing corruption".into());
            }
            roster.unchanged()?;
            Ok(roster)
        };
        read().map_err(|e: String| format!("roster {}: {e}", path.display()))
    }
    pub fn tracked_names(&self) -> BTreeSet<String> {
        self.npx
            .keys()
            .chain(self.clawhub.keys())
            .cloned()
            .collect()
    }
    pub fn unchanged(&self) -> Result<(), String> {
        let bytes = std::fs::read(&self.source)
            .map_err(|e| format!("roster {}: {e}", self.source.display()))?;
        if digest(&bytes) != self.hash {
            return Err(format!(
                "roster {} changed during the run",
                self.source.display()
            ));
        }
        Ok(())
    }
}
pub(super) fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
#[cfg(test)]
mod tests;
