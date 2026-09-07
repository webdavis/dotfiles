mod ports;
pub use ports::*;
use posture_domain::{
    AllowlistChange, AllowlistEntry, LaunchdIdentity, curate_allowlist,
    relativize_allowlist_identity, valid_allowlist_label,
};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowlistCommand<'a> {
    Add(&'a str),
    Deny(&'a str),
    List,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurationOutcome {
    Allowed { label: String, program: String },
    Denied(String),
    NotPresent(String),
    Listed(Vec<u8>),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurationFailure {
    Lock,
    InvalidLabel(String),
    Capture(CaptureRefusal),
    Source(SourceRefusal),
    InvalidLine(std::path::PathBuf),
    Publication(PublicationRefusal),
}
pub struct CurateAllowlist<'a, L, S, P, W> {
    pub home: &'a Path,
    pub launchd: &'a mut L,
    pub source: &'a mut S,
    pub publisher: &'a mut P,
    pub lock: &'a W,
}
impl<L: LaunchdTable, S: SourceAllowlist, P: Publisher, W: WriteLock>
    CurateAllowlist<'_, L, S, P, W>
{
    pub fn run(
        &mut self,
        command: AllowlistCommand<'_>,
    ) -> Result<CurationOutcome, CurationFailure> {
        let label = match command {
            AllowlistCommand::List => {
                return self
                    .source
                    .list()
                    .map(CurationOutcome::Listed)
                    .map_err(CurationFailure::Source);
            }
            AllowlistCommand::Add(label) | AllowlistCommand::Deny(label) => label,
        };
        let _guard = self.lock.acquire().map_err(|_| CurationFailure::Lock)?;
        if !valid_allowlist_label(label) {
            return Err(CurationFailure::InvalidLabel(label.to_owned()));
        }
        let captured = if matches!(command, AllowlistCommand::Add(_)) {
            Some(
                self.launchd
                    .capture(label)
                    .map_err(CurationFailure::Capture)?,
            )
        } else {
            None
        };
        let source = self.source.source_path().map_err(CurationFailure::Source)?;
        if captured.is_none() && !self.source.contains_label_text(&source, label) {
            return Ok(CurationOutcome::NotPresent(label.to_owned()));
        }
        let lines = self.source.read(&source).map_err(CurationFailure::Source)?;
        let borrowed: Vec<_> = lines.iter().map(SourceLine::borrowed).collect();
        let (path, program) = captured.as_ref().map_or_else(
            || (String::new(), String::new()),
            |agent| {
                self.home.to_str().map_or_else(
                    || (agent.path.clone(), agent.program.clone()),
                    |home| relativize_allowlist_identity(home, &agent.path, &agent.program),
                )
            },
        );
        let change = captured
            .as_ref()
            .map_or(AllowlistChange::Deny(label), |agent| {
                AllowlistChange::Allow(AllowlistEntry {
                    identity: LaunchdIdentity {
                        label,
                        path: &path,
                        program: &program,
                    },
                    sha256: &agent.sha256,
                })
            });
        let curated = curate_allowlist(&borrowed, change)
            .map_err(|_| CurationFailure::InvalidLine(source.clone()))?;
        self.publisher
            .publish(&source, &curated)
            .map_err(CurationFailure::Publication)?;
        Ok(match captured {
            Some(agent) => CurationOutcome::Allowed {
                label: label.to_owned(),
                program: agent.program,
            },
            None => CurationOutcome::Denied(label.to_owned()),
        })
    }
}
#[cfg(test)]
mod tests;
