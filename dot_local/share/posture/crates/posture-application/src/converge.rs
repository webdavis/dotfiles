use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StagingRefusal {
    RelativeDirectory(PathBuf),
    SymlinkComponent(PathBuf),
    MissingDirectory(PathBuf),
    IncompleteListing(PathBuf),
    SymlinkEntry(PathBuf),
    UnlistedEntry(PathBuf),
    MissingFile(PathBuf),
    CopyFailed(PathBuf),
    PrivateDirectory(PathBuf),
}

// The prepared value owns the private copy until its consumer finishes reading it.
// No live target or privileged operation belongs to this preparation boundary.
pub trait ConvergeStaging {
    type Prepared;

    fn prepare(&self) -> Result<Self::Prepared, Vec<StagingRefusal>>;
}
