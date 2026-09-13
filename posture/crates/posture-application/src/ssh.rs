use crate::InspectionFailure;
use posture_domain::{IncludeRefusal, SshJudgment, SshRecord, SshTreeRefusal};
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub struct SshCompleted {
    pub status: i32,
    pub output: Vec<u8>,
}
pub type SshCommandResult = Result<SshCompleted, InspectionFailure>;

pub trait Sshd {
    fn available(&self) -> bool;
    fn global(&mut self) -> SshCommandResult;
    fn connection(&mut self, specification: &str) -> SshCommandResult;
    fn syntax(&mut self) -> SshCommandResult;
}

pub trait SshLaunchctl {
    fn probe(&mut self) -> SshCommandResult;
    fn restart(&mut self) -> SshCommandResult;
}

pub trait SshBannerProbe {
    fn available(&self) -> bool;
    fn probe(&mut self, port: u16, timeout: u32) -> SshCommandResult;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshFile {
    Target,
    Legacy,
    Staging,
    SavedTarget,
    SavedLegacy,
}
impl SshFile {
    pub fn name(self) -> &'static str {
        match self {
            Self::Target => "000-ssh-hardening.conf",
            Self::Legacy => "50-no-password-auth.conf",
            Self::Staging => ".000-ssh-hardening.conf.staging",
            Self::SavedTarget => ".000-ssh-hardening.conf.saved",
            Self::SavedLegacy => ".50-no-password-auth.conf.saved",
        }
    }
}
pub trait SshInstallFiles {
    fn path(&self, file: SshFile) -> PathBuf;
    fn directory_exists(&self) -> bool;
    fn exists(&self, file: SshFile) -> Result<bool, InspectionFailure>;
    fn prime(&mut self) -> SshCommandResult;
    fn stage(&mut self, bytes: &[u8]) -> SshCommandResult;
    fn chmod(&mut self) -> SshCommandResult;
    fn save(&mut self) -> SshCommandResult;
    fn rename(&mut self, from: SshFile, to: SshFile) -> SshCommandResult;
    fn remove(&mut self, files: &[SshFile]) -> SshCommandResult;
}

#[derive(Debug, PartialEq, Eq)]
pub enum SshScanFailure {
    File(SshTreeRefusal),
    Include {
        path: Vec<u8>,
        reason: IncludeRefusal,
    },
    Directive {
        path: Vec<u8>,
        judgment: SshJudgment,
    },
}

pub trait SshTree {
    fn scan(&self) -> Vec<SshScanFailure>;
    fn observe(&self) -> Result<Vec<SshRecord>, SshScanFailure>;
}
