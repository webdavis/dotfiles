pub struct FileConfigPublisher {
    pub path: std::path::PathBuf,
}
impl pns_application::ConfigPublisher for FileConfigPublisher {
    fn path(&self) -> String {
        self.path.display().to_string()
    }
    fn check(&self, force: bool) -> Result<(), String> {
        crate::config_publication::check_config_path(&self.path, force)
    }
    fn publish(&self, composed: &str, force: bool) -> Result<Option<String>, String> {
        crate::config_publication::publish_config(&self.path, composed, force)
            .map(|backup| backup.map(|path| path.display().to_string()))
    }
}
