use crate::{CommandIo, CommandRunner, SystemRunner};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStringExt;
mod staged;
use posture_application::{PublicationRefusal, Publisher};
use posture_domain::CuratedLine;
use staged::Staged;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct AllowlistPublisher {
    chezmoi: PathBuf,
    deployed: PathBuf,
    manifest: Option<PathBuf>,
    budget: Duration,
}
impl AllowlistPublisher {
    pub fn new(
        chezmoi: PathBuf,
        deployed: PathBuf,
        manifest: Option<PathBuf>,
        budget: Duration,
    ) -> Self {
        Self {
            chezmoi,
            deployed,
            manifest,
            budget,
        }
    }
    fn publish_with(
        &self,
        runner: &mut impl CommandRunner,
        source: &Path,
        lines: &[CuratedLine<'_, &[u8]>],
    ) -> Result<(), PublicationRefusal> {
        let bytes = staged::encode(lines)
            .map_err(|error| PublicationRefusal::SourceWrite(error.to_string()))?;
        let staged = Staged::create(&bytes)
            .map_err(|error| PublicationRefusal::SourceWrite(error.to_string()))?;
        // Bash uses an empty backup when the original source cannot be read, including absence.
        let original = fs::read(source).unwrap_or_default();
        let backup = Staged::create(&original).map_err(|_| PublicationRefusal::Backup)?;
        fs::rename(&staged.0, source)
            .map_err(|_| PublicationRefusal::SourceWrite(source.display().to_string()))?;
        if runner
            .run(
                &self.chezmoi,
                &[
                    OsStr::new("apply"),
                    OsStr::new("--force"),
                    self.deployed.as_os_str(),
                ],
                CommandIo::InheritAll,
            )
            .is_err()
        {
            // Source rollback does not prove a failed external apply left the deployed file alone.
            let rollback_error = fs::read(&backup.0)
                .and_then(|bytes| fs::write(source, bytes))
                .err()
                .map(|error| error.to_string());
            return Err(PublicationRefusal::Apply { rollback_error });
        }
        let manifest = match &self.manifest {
            Some(path) => path.clone(),
            None => {
                let mut root = runner
                    .run(
                        &self.chezmoi,
                        &[OsStr::new("source-path")],
                        CommandIo::CaptureStdout,
                    )
                    .map_err(|_| PublicationRefusal::ManifestMissing(None))?;
                root.retain(|byte| *byte != 0);
                while root.last() == Some(&b'\n') {
                    root.pop();
                }
                root.extend_from_slice(
                    b"/.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh",
                );
                PathBuf::from(OsString::from_vec(root))
            }
        };
        if !manifest.is_file() {
            return Err(PublicationRefusal::ManifestMissing(Some(manifest)));
        }
        runner
            .run(
                Path::new("/bin/bash"),
                &[manifest.as_os_str()],
                CommandIo::InheritAll,
            )
            .map_err(|_| PublicationRefusal::ManifestRefresh(manifest))?;
        Ok(())
    }
}
impl Publisher for AllowlistPublisher {
    fn publish(
        &mut self,
        source: &Path,
        lines: &[CuratedLine<'_, &[u8]>],
    ) -> Result<(), PublicationRefusal> {
        // Start this total budget after lock acquisition and identity capture, never while waiting.
        self.publish_with(&mut SystemRunner::new(self.budget), source, lines)
    }
}
#[cfg(test)]
mod tests;
