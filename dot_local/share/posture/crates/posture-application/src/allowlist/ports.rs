use posture_domain::{AllowlistLine, CuratedLine};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedAgent {
    pub path: String,
    pub program: String,
    pub sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureRefusal {
    NoAgent,
    Hash(String),
}
pub trait LaunchdTable {
    fn capture(&mut self, label: &str) -> Result<CapturedAgent, CaptureRefusal>;
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceLine {
    Preserved(Vec<u8>),
    Object { label: Option<String>, raw: Vec<u8> },
    Invalid,
}
impl SourceLine {
    pub(super) fn borrowed(&self) -> AllowlistLine<'_, &[u8]> {
        match self {
            Self::Preserved(raw) => AllowlistLine::Preserved(raw),
            Self::Object { label, raw } => AllowlistLine::Object {
                label: label.as_deref(),
                raw,
            },
            Self::Invalid => AllowlistLine::Invalid,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceRefusal {
    Resolve,
    Read,
}
pub trait SourceAllowlist {
    fn source_path(&mut self) -> Result<PathBuf, SourceRefusal>;
    fn contains_label_text(&self, source: &Path, label: &str) -> bool;
    fn read(&self, source: &Path) -> Result<Vec<SourceLine>, SourceRefusal>;
    fn list(&self) -> Result<Vec<u8>, SourceRefusal>;
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicationRefusal {
    Backup,
    SourceWrite(String),
    Apply { rollback_error: Option<String> },
    ManifestMissing(Option<PathBuf>),
    ManifestRefresh(PathBuf),
}
pub trait Publisher {
    fn publish(
        &mut self,
        source: &Path,
        lines: &[CuratedLine<'_, &[u8]>],
    ) -> Result<(), PublicationRefusal>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockRefusal;
pub trait WriteLock {
    type Guard;
    fn acquire(&self) -> Result<Self::Guard, LockRefusal>;
}
