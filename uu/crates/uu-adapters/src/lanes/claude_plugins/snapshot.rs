use super::super::changes::Listing;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::sync::atomic::{AtomicU64, Ordering};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub(super) fn baseline(home: &str, name: &str) -> Result<Option<Listing>, String> {
    let destination = path(home, name);
    if let Some(text) = read(&destination)? {
        return parse(&text).map(Some);
    }
    let legacy =
        Path::new(home).join(".local/state/report-plugin-updates/installed-plugins.snapshot");
    let Some(text) = read(&legacy)? else {
        return Ok(None);
    };
    let rows = parse(&text)?;
    publish(&destination, &text)?;
    Ok(Some(rows))
}

pub(super) fn path(home: &str, name: &str) -> PathBuf {
    Path::new(home)
        .join(".local/state/uu/lanes")
        .join(name)
        .join("snapshot.tsv")
}

pub(super) fn read(path: &Path) -> Result<Option<String>, String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("could not inspect {}: {error}", path.display())),
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(format!("{} is not a regular file", path.display()));
        }
        Ok(_) => {}
    }
    fs::read_to_string(path)
        .map(Some)
        .map_err(|error| format!("could not read {}: {error}", path.display()))
}

fn parse(text: &str) -> Result<Listing, String> {
    let mut rows = Vec::new();
    let mut previous: Option<&str> = None;
    for line in text.split_terminator('\n') {
        let (name, fingerprint) = line
            .split_once('\t')
            .ok_or("snapshot row has no tab separator")?;
        if fingerprint.contains('\t') || line.contains('\r') {
            return Err("snapshot row is not one name/fingerprint pair".into());
        }
        if previous.is_some_and(|before| before > line) {
            return Err("snapshot rows are not sorted".into());
        }
        previous = Some(line);
        rows.push((name.into(), fingerprint.into()));
    }
    Ok(rows)
}

pub(super) fn advance(home: &str, name: &str, rows: &Listing) -> Result<(), String> {
    let mut text = String::new();
    for (name, fingerprint) in rows {
        text.push_str(&format!("{name}\t{fingerprint}\n"));
    }
    publish(&path(home, name), &text)
}

fn publish(destination: &Path, text: &str) -> Result<(), String> {
    let parent = destination
        .parent()
        .ok_or("snapshot has no parent directory")?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let temporary = parent.join(format!(
        ".snapshot-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|error| format!("could not create {}: {error}", temporary.display()))?;
    let result = file
        .write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .and_then(|()| fs::rename(&temporary, destination));
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "could not publish {}: {error}",
            destination.display()
        ));
    }
    Ok(())
}
