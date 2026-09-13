use crate::private_directory::PrivateDirectory;
use posture_domain::ConvergeFile;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct StagedTree {
    root: PrivateDirectory,
}

impl StagedTree {
    pub(super) fn create(parent: &Path) -> io::Result<Self> {
        let staged = Self {
            root: PrivateDirectory::create(parent)?,
        };
        fs::create_dir(staged.root.path().join("packs"))?;
        Ok(staged)
    }

    pub fn source(&self, file: ConvergeFile) -> PathBuf {
        self.root.path().join(file.relative_path())
    }
}

impl posture_application::DesiredTree for StagedTree {
    fn source(&self, file: ConvergeFile) -> PathBuf {
        self.source(file)
    }
}
