use posture_application::LiveTree;
use posture_domain::{
    ContentComparison, ConvergeDirectory, ConvergeFile, LiveAttributes, LiveEntry,
};
use std::fs::{self, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

pub struct InstalledTree {
    root: PathBuf,
}

impl InstalledTree {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl LiveTree for InstalledTree {
    fn directory(&mut self, directory: ConvergeDirectory) -> LiveEntry {
        read_entry(&self.root.join(directory.relative_path()))
    }

    fn file(&mut self, file: ConvergeFile, desired: &Path) -> (LiveEntry, ContentComparison) {
        let live = self.root.join(file.relative_path());
        let entry = read_entry(&live);
        let content = if matches!(entry, LiveEntry::File(_)) {
            compare(desired, &live)
        } else {
            ContentComparison::Unreadable
        };
        (entry, content)
    }
}

fn read_entry(path: &Path) -> LiveEntry {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return LiveEntry::Absent,
        Err(_) => return LiveEntry::Unreadable,
    };
    let attributes = LiveAttributes {
        // Preserve setuid, setgid and sticky bits, which are also drift (S323).
        mode: metadata.mode() & 0o7777,
        uid: metadata.uid(),
        gid: metadata.gid(),
    };
    if metadata.is_file() {
        LiveEntry::File(attributes)
    } else if metadata.is_dir() {
        LiveEntry::Directory(attributes)
    } else {
        LiveEntry::Irregular
    }
}

fn compare(desired: &Path, live: &Path) -> ContentComparison {
    match (read_regular(desired), read_regular(live)) {
        (Ok(desired), Ok(live)) if desired == live => ContentComparison::Equal,
        (Ok(_), Ok(_)) => ContentComparison::Different,
        _ => ContentComparison::Unreadable,
    }
}

fn read_regular(path: &Path) -> io::Result<Vec<u8>> {
    // A replacement symlink or pipe between the probe and open cannot redirect or block this read.
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests;
