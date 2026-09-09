use posture_domain::CuratedLine;
use std::ffi::{CString, OsString};
use std::fs::{self, File};
use std::io::{self, Write};
use std::os::fd::FromRawFd;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

pub(super) struct Staged(pub(super) PathBuf);
impl Staged {
    pub(super) fn create(bytes: &[u8]) -> io::Result<Self> {
        let path = std::env::temp_dir().join("posture-allowlist.XXXXXX");
        let mut template = CString::new(path.as_os_str().as_bytes())?.into_bytes_with_nul();
        // mkstemp edits this writable NUL-terminated template and returns one new mode-600 fd.
        let descriptor = unsafe { libc::mkstemp(template.as_mut_ptr().cast()) };
        if descriptor == -1 {
            return Err(io::Error::last_os_error());
        }
        // The successful descriptor is adopted once and always closes before an external spawn.
        let mut file = unsafe { File::from_raw_fd(descriptor) };
        template.pop();
        let staged = Self(PathBuf::from(OsString::from_vec(template)));
        file.write_all(bytes)?;
        Ok(staged)
    }
}
impl Drop for Staged {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
pub(super) fn encode(lines: &[CuratedLine<'_, &[u8]>]) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for line in lines {
        match line {
            CuratedLine::Preserved(raw) => bytes.extend_from_slice(raw),
            CuratedLine::Added(entry) => {
                bytes.push(b'{');
                for (index, (key, value)) in [
                    ("label", entry.identity.label),
                    ("path", entry.identity.path),
                    ("program", entry.identity.program),
                    ("sha256", entry.sha256),
                ]
                .into_iter()
                .enumerate()
                {
                    if index > 0 {
                        bytes.push(b',');
                    }
                    serde_json::to_writer(&mut bytes, key)?;
                    bytes.push(b':');
                    serde_json::to_writer(&mut bytes, value)?;
                }
                bytes.push(b'}');
            }
        }
        bytes.push(b'\n');
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
