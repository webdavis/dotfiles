use posture_application::{ConvergeStaging, StagingRefusal};
use posture_domain::ConvergeFile;
use std::fs;
use std::path::PathBuf;

mod listing;
mod owned;

pub struct DesiredStaging {
    desired: PathBuf,
    scratch: PathBuf,
}

impl DesiredStaging {
    pub fn new(desired: PathBuf, scratch: PathBuf) -> Self {
        Self { desired, scratch }
    }
}

pub use owned::StagedTree;

impl ConvergeStaging for DesiredStaging {
    type Prepared = StagedTree;

    fn prepare(&self) -> Result<Self::Prepared, Vec<StagingRefusal>> {
        listing::validate(&self.desired)?;
        let staged = StagedTree::create(&self.scratch)
            .map_err(|_| vec![StagingRefusal::PrivateDirectory(self.scratch.clone())])?;
        for file in ConvergeFile::ALL {
            let source = self.desired.join(file.relative_path());
            // Copy as the invoking user. All later comparison/install reads use this owned copy.
            fs::copy(&source, staged.source(file))
                .map_err(|_| vec![StagingRefusal::CopyFailed(source)])?;
        }
        Ok(staged)
    }
}

#[cfg(test)]
mod tests;
