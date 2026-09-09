use super::*;
use crate::legacy_json::command_text;
use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;

impl PollStateFiles {
    pub fn covered(&self, gap: PollGap) -> Vec<String> {
        let path = self.marker(gap);
        if !path.is_file() {
            return vec![];
        }
        let Ok(mut file) = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
        else {
            return vec![];
        };
        if !file.metadata().is_ok_and(|m| m.is_file()) {
            return vec![];
        }
        // cat's printed prefix remains visible even when the read later fails.
        let mut bytes = Vec::new();
        let _ = file.read_to_end(&mut bytes);
        command_text(String::from_utf8_lossy(&bytes).into_owned())
            .split(' ')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect()
    }
    pub fn remember(&self, gap: PollGap, members: &[String]) -> io::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(self.marker(gap))?;
        writeln!(file, "{}", members.join(" "))
    }
    pub fn clear(&self, gap: PollGap) -> io::Result<()> {
        match fs::remove_file(self.marker(gap)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }
}

/// The port the application drives these three through.
///
/// A THIN BRIDGE ON PURPOSE, and separate from the inherent methods above
/// rather than replacing them. The inherent forms return `io::Result`, which is
/// what a caller inside this crate wants when it is deciding what to do about a
/// particular errno. The port returns the application's own opaque failure,
/// because policy has no business branching on an operating system's error
/// kinds: every way a marker fails to persist means the same thing to it.
impl posture_application::PollMarkers for PollStateFiles {
    fn covered(&self, gap: PollGap) -> Vec<String> {
        Self::covered(self, gap)
    }

    fn remember(
        &self,
        gap: PollGap,
        members: &[String],
    ) -> Result<(), posture_application::PollStateFailure> {
        Self::remember(self, gap, members).map_err(|_| posture_application::PollStateFailure)
    }

    fn clear(&self, gap: PollGap) -> Result<(), posture_application::PollStateFailure> {
        Self::clear(self, gap).map_err(|_| posture_application::PollStateFailure)
    }
}
