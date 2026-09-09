use crate::{CommandIo, CommandRunner};
use posture_application::SourceRefusal;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::path::Path;

fn listed_bytes(path: &Path) -> Result<Vec<u8>, SourceRefusal> {
    if fs::metadata(path).map_or(true, |metadata| metadata.len() == 0) {
        return Ok(Vec::new());
    }
    let mut bytes = fs::read(path).map_err(|_| SourceRefusal::Read)?;
    // Bash read discards NUL bytes before classifying comments and blanks.
    bytes.retain(|byte| *byte != 0);
    let mut result = Vec::new();
    for line in bytes.split(|byte| *byte == b'\n') {
        if line.is_empty() || line.starts_with(b"#") {
            continue;
        }
        result.extend_from_slice(line);
        result.push(b'\n');
    }
    Ok(result)
}
fn contains_label_text(path: &Path, label: &str) -> bool {
    if !path.is_file() {
        return false;
    }
    let needle = format!("\"label\":\"{label}\"");
    fs::read(path).is_ok_and(|bytes| {
        bytes
            .windows(needle.len())
            .any(|window| window == needle.as_bytes())
    })
}

pub struct AllowlistFile {
    deployed: std::path::PathBuf,
    chezmoi: std::path::PathBuf,
    budget: std::time::Duration,
}
impl AllowlistFile {
    pub fn new(
        deployed: std::path::PathBuf,
        chezmoi: std::path::PathBuf,
        budget: std::time::Duration,
    ) -> Self {
        Self {
            deployed,
            chezmoi,
            budget,
        }
    }
    fn resolve_with(
        &self,
        runner: &mut impl CommandRunner,
    ) -> Result<std::path::PathBuf, SourceRefusal> {
        let mut bytes = runner
            .run(
                &self.chezmoi,
                &[OsStr::new("source-path"), self.deployed.as_os_str()],
                CommandIo::CaptureStdout,
            )
            .map_err(|_| SourceRefusal::Resolve)?;
        bytes.retain(|byte| *byte != 0);
        while bytes.last() == Some(&b'\n') {
            bytes.pop();
        }
        if bytes.is_empty() {
            return Err(SourceRefusal::Resolve);
        }
        Ok(OsString::from_vec(bytes).into())
    }
}
impl posture_application::SourceAllowlist for AllowlistFile {
    fn source_path(&mut self) -> Result<std::path::PathBuf, SourceRefusal> {
        self.resolve_with(&mut crate::SystemRunner::new(self.budget))
    }
    fn contains_label_text(&self, source: &Path, label: &str) -> bool {
        contains_label_text(source, label)
    }
    fn read(&self, source: &Path) -> Result<Vec<posture_application::SourceLine>, SourceRefusal> {
        if !source.is_file() {
            return Ok(Vec::new());
        }
        let mut bytes = fs::read(source).map_err(|_| SourceRefusal::Read)?;
        bytes.retain(|byte| *byte != 0);
        if bytes.is_empty() {
            return Ok(Vec::new());
        }
        let bytes = bytes.strip_suffix(b"\n").unwrap_or(&bytes);
        Ok(bytes
            .split(|byte| *byte == b'\n')
            .map(|line| crate::allowlist_projection::source_line(line.to_vec()))
            .collect())
    }
    fn list(&self) -> Result<Vec<u8>, SourceRefusal> {
        listed_bytes(&self.deployed)
    }
}

#[cfg(test)]
mod tests;
