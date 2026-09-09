use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(super) fn is_archive(log: &Path) -> bool {
    log.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".gz"))
        .and_then(|name| name.rsplit_once('.'))
        .is_some_and(|(_, index)| digits(index))
}

pub(super) fn archive(log: &Path, index: u64) -> PathBuf {
    PathBuf::from(format!("{}.{index}.gz", log.display()))
}

pub(super) fn shift(log: &Path, keep: u64) -> io::Result<()> {
    let parent = log
        .parent()
        .ok_or_else(|| io::Error::other("log has no parent"))?;
    let prefix = format!(
        "{}.",
        log.file_name()
            .ok_or_else(|| io::Error::other("log has no filename"))?
            .to_string_lossy()
    );
    let mut retained = Vec::new();
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(index) = name
            .to_str()
            .and_then(|name| name.strip_prefix(&prefix))
            .and_then(|name| name.strip_suffix(".gz"))
            .filter(|index| digits(index))
        else {
            continue;
        };
        if !entry.file_type()?.is_file() {
            return Err(io::Error::other(format!(
                "archive {} is not a regular file",
                entry.path().display()
            )));
        }
        match index.parse::<u64>() {
            Ok(index) if index >= 1 && index < keep => retained.push((index, entry.path())),
            // An all-digit index outside u64 is necessarily outside this window too.
            _ => fs::remove_file(entry.path())?,
        }
    }
    retained.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    for (index, path) in retained {
        fs::rename(path, archive(log, index + 1))?;
    }
    Ok(())
}

fn digits(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}
