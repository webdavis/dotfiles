use lights_application::PositionStore;
use lights_domain::RoomName;
use std::{
    cell::Cell,
    io,
    path::{Path, PathBuf},
};

/// One TOML key per full room name, rewritten by rename so a reader never sees
/// half a file. A file that is missing, unreadable or unparseable counts as no
/// memory: the rotation falls back the way it did before the file existed, and
/// the command still succeeds.
pub struct FilePositionStore {
    path: PathBuf,
    complained: Cell<bool>,
}

impl FilePositionStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            complained: Cell::new(false),
        }
    }
    fn table(&self) -> toml::Table {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => text.parse().unwrap_or_else(|_| {
                self.complain("cannot parse");
                toml::Table::new()
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => toml::Table::new(),
            Err(_) => {
                self.complain("cannot read");
                toml::Table::new()
            }
        }
    }
    /// One line per run. A rotation that restarted is worth saying once; a line
    /// per keypress would be noise, and a failure here is never the command's.
    fn complain(&self, trouble: &str) {
        if !self.complained.replace(true) {
            eprintln!(
                "lights: {trouble} {}; the rotation starts from its fallback",
                self.path.display()
            );
        }
    }
    fn write(&self, table: &toml::Table) -> io::Result<()> {
        let directory = self
            .path
            .parent()
            .ok_or_else(|| io::Error::other("no parent directory"))?;
        std::fs::create_dir_all(directory)?;
        // ponytail: one temp name per process, because one keypress at a time
        // is the whole workload; a lock would buy nothing here.
        let temp = temporary(&self.path);
        std::fs::write(&temp, table.to_string())?;
        std::fs::rename(&temp, &self.path).inspect_err(|_| {
            let _ = std::fs::remove_file(&temp);
        })
    }
}

impl PositionStore for FilePositionStore {
    fn remembered(&self, room: &RoomName) -> Option<String> {
        self.table().get(room.as_str())?.as_str().map(str::to_owned)
    }
    fn remember(&self, room: &RoomName, scene: &str) {
        let mut table = self.table();
        table.insert(
            room.as_str().to_owned(),
            toml::Value::String(scene.to_owned()),
        );
        if self.write(&table).is_err() {
            self.complain("cannot write");
        }
    }
}

fn temporary(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.tmp", std::process::id()));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests;
