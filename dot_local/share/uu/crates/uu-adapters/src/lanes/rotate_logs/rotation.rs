use std::fs::{self, File, Metadata, OpenOptions};
use std::io;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use crate::{CommandRunner, RotateLogsLane};

mod retention;

pub(super) enum Observed {
    Skipped,
    UnderThreshold,
    Rotated(u64),
}

pub(super) fn rotate(
    log: &Path,
    config: &RotateLogsLane,
    runner: &dyn CommandRunner,
) -> io::Result<Observed> {
    let metadata = match fs::symlink_metadata(log) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Observed::Skipped),
        Err(error) => return Err(error),
    };
    if !metadata.is_file() || retention::is_archive(log) {
        return Ok(Observed::Skipped);
    }
    if metadata.len() < config.rotate_at_bytes {
        return Ok(Observed::UnderThreshold);
    }
    // Keep this descriptor through compression and truncation. A path replacement
    // must never redirect the truncation to a different file or a symlink target.
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(log)?;
    let opened = file.metadata()?;
    if !opened.is_file() || !same_file(&metadata, &opened) {
        return Err(io::Error::other("log changed before it could be opened"));
    }
    if opened.len() < config.rotate_at_bytes {
        return Ok(Observed::UnderThreshold);
    }
    retention::shift(log, config.archives_kept)?;
    let archive = retention::archive(log, 1);
    let partial = PathBuf::from(format!("{}.partial", archive.display()));
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&partial)?;
    let owned = output.metadata()?;
    let result = compress(&file, output, &partial, &owned, &archive, config, runner);
    if result.is_err() && still_owned(&partial, &owned) {
        fs::remove_file(&partial)?;
    }
    result?;
    // Copy then truncate deliberately preserves an open writer's inode. A line
    // appended between those operations can still be lost, as in the Bash job.
    file.set_len(0)?;
    Ok(Observed::Rotated(opened.len()))
}

fn compress(
    input: &File,
    output: File,
    partial: &Path,
    owned: &Metadata,
    archive: &Path,
    config: &RotateLogsLane,
    runner: &dyn CommandRunner,
) -> io::Result<()> {
    let observe = output.try_clone()?;
    runner
        .run_to_file(&config.compressor, &["-c"], input.try_clone()?, output)
        .map_err(io::Error::other)?;
    if observe.metadata()?.len() == 0 {
        return Err(io::Error::other("compressor produced an empty archive"));
    }
    if !still_owned(partial, owned) {
        return Err(io::Error::other(
            "partial archive changed during compression",
        ));
    }
    fs::rename(partial, archive)
}

fn still_owned(path: &Path, owned: &Metadata) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && same_file(owned, &metadata))
}

fn same_file(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino()
}
