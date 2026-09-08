use super::metadata::{Metadata, field, segment};
use super::{SkillsGenerationStore, SkillsPublication, exchange_skills_directories, metadata};
use std::collections::BTreeSet;

impl SkillsGenerationStore {
    pub(super) fn recover_publication(&self) -> Result<SkillsPublication, String> {
        let marker = metadata::document(&self.marker())?;
        let new = field(&marker, "new")?;
        let old = marker
            .get("old")
            .and_then(|v| v.as_str())
            .ok_or("exchange lost outgoing identity")?
            .to_owned();
        if !segment(&new) || (!old.is_empty() && !segment(&old)) || new == old {
            return Err("invalid exchange identity".into());
        }
        let outgoing = marker
            .get("outgoing")
            .and_then(|v| v.as_array())
            .ok_or("exchange lost outgoing ownership")?
            .iter()
            .map(|v| {
                v.as_str()
                    .filter(|s| segment(s))
                    .map(str::to_owned)
                    .ok_or_else(|| "invalid outgoing skill name".to_string())
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        let workspace = self.generations().join(&new).join("home/.agents");
        if old.is_empty() {
            if !self.current().exists() && !self.current().is_symlink() {
                std::fs::rename(&workspace, self.current()).map_err(|e| e.to_string())?;
            }
            if Metadata::read(&self.current())?.id != new {
                return Err("first publication identity changed".into());
            }
            return Ok(SkillsPublication {
                outgoing_names: outgoing,
                previous_id: old,
            });
        }
        let live = Metadata::read(&self.current())?;
        if live.id == old && Metadata::read(&workspace)?.id == new {
            exchange_skills_directories(&workspace, &self.current())?;
        }
        if Metadata::read(&self.current())?.id != new {
            return Err("exchange current identity changed".into());
        }
        let retained = self.generations().join(&old);
        if workspace.exists() {
            if Metadata::read(&workspace)?.id != old {
                return Err("exchange outgoing identity changed".into());
            }
            if retained.exists() {
                // A former candidate leaves only this empty shell after publication.
                // Removing directories singly refuses unrelated entries or a prior snapshot.
                metadata::remove_empty(&retained.join("home"))?;
                metadata::remove_empty(&retained)?;
            }
            std::fs::rename(&workspace, &retained)
                .map_err(|e| format!("prior generation retention: {e}"))?;
        }
        if Metadata::read(&retained)?.id != old {
            return Err("prior generation retention is unfinished".into());
        }
        Ok(SkillsPublication {
            outgoing_names: outgoing,
            previous_id: old,
        })
    }
}
