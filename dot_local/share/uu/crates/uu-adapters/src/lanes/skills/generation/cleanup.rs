use super::{Metadata, SkillsGenerationStore, garbage, metadata, segment};
impl SkillsGenerationStore {
    pub fn finish_publication(&self, previous_id: &str) -> Result<(), String> {
        let marker = metadata::document(&self.marker())?;
        if marker.get("old").and_then(|v| v.as_str()) != Some(previous_id)
            || (!previous_id.is_empty()
                && Metadata::read(&self.generations().join(previous_id))?.id != previous_id)
        {
            return Err("publication retention is unfinished".into());
        }
        let new_id = metadata::field(&marker, "new")?;
        if (!previous_id.is_empty() && !segment(previous_id))
            || !segment(&new_id)
            || new_id == previous_id
        {
            return Err("invalid publication cleanup identity".into());
        }
        let workspace = self.generations().join(new_id);
        // Called only after pruning: the retained generation and marker still carry
        // outgoing ownership while this private installer workspace is reclaimed.
        garbage::destroy(&workspace)?;
        self.sweep_completed(previous_id)?;
        std::fs::remove_file(self.marker()).map_err(|e| e.to_string())
    }
    pub fn sweep(&self, previous_id: &str) -> Result<(), String> {
        if self.marker().exists() {
            return Err("publication in flight; retaining its evidence".into());
        }
        self.sweep_completed(previous_id)
    }
    fn sweep_completed(&self, previous_id: &str) -> Result<(), String> {
        for entry in std::fs::read_dir(self.generations()).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                continue;
            }
            let path = entry.path();
            if path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|name| name.contains(".garbage."))
            {
                std::fs::remove_dir_all(path).map_err(|e| e.to_string())?;
                continue;
            }
            if let Ok(m) = Metadata::read(&path) {
                if path.file_name().and_then(|s| s.to_str()) == Some(&m.id) && m.id != previous_id {
                    garbage::destroy(&path)?;
                }
            }
        }
        Ok(())
    }
}
