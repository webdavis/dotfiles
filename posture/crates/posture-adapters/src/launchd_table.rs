use crate::{CommandIo, CommandRunner, SystemRunner, allowlist_projection::query_identity};
use posture_application::{CaptureRefusal, CapturedAgent, LaunchdTable};
use sha2::{Digest, Sha256};
use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct SystemLaunchdTable {
    osqueryi: PathBuf,
    budget: Duration,
}
impl SystemLaunchdTable {
    pub fn new(osqueryi: PathBuf, budget: Duration) -> Self {
        Self { osqueryi, budget }
    }
    fn capture_with(
        &self,
        label: &str,
        runner: &mut impl CommandRunner,
    ) -> Result<CapturedAgent, CaptureRefusal> {
        // The application validates the label before this query is assembled.
        let query = format!(
            "SELECT path, COALESCE(NULLIF(program,''), program_arguments) AS program FROM launchd WHERE label = '{label}';"
        );
        let bytes = runner
            .run(
                &self.osqueryi,
                &[OsStr::new("--json"), OsStr::new(&query)],
                CommandIo::Inspection {
                    merge_stderr: false,
                },
            )
            .map_err(|_| CaptureRefusal::NoAgent)?;
        let (path, program) = query_identity(&bytes)
            .filter(|(path, program)| !path.is_empty() && !program.is_empty())
            .ok_or(CaptureRefusal::NoAgent)?;
        let sha256 =
            plist_hash(Path::new(&path)).map_err(|_| CaptureRefusal::Hash(path.clone()))?;
        Ok(CapturedAgent {
            path,
            program,
            sha256,
        })
    }
}
impl LaunchdTable for SystemLaunchdTable {
    fn capture(&mut self, label: &str) -> Result<CapturedAgent, CaptureRefusal> {
        self.capture_with(label, &mut SystemRunner::new(self.budget))
    }
}
fn plist_hash(path: &Path) -> io::Result<String> {
    // Match the writer's regular-file check, including symlinks to regular plists. A raced
    // replacement with a FIFO must not block the open before we can check its actual kind.
    if !path.is_file() {
        return Err(io::Error::other("not a regular plist"));
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("not a regular plist"));
    }
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
#[cfg(test)]
mod tests;
