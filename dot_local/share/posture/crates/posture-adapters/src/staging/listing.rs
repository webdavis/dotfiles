use posture_application::StagingRefusal;
use posture_domain::ConvergeFile;
use std::fs::{self, FileType};
use std::path::{Path, PathBuf};

pub(super) fn validate(desired: &Path) -> Result<(), Vec<StagingRefusal>> {
    check_components(desired).map_err(|refusal| vec![refusal])?;
    if !desired.is_dir() {
        return Err(vec![StagingRefusal::MissingDirectory(
            desired.to_path_buf(),
        )]);
    }
    // Finish every directory iterator before judging the listing or copying any desired bytes.
    let entries = complete_listing(desired).map_err(|refusal| vec![refusal])?;
    let mut refused = Vec::new();
    for (path, kind) in entries {
        if kind.is_symlink() {
            refused.push(StagingRefusal::SymlinkEntry(path));
        } else if !kind.is_dir()
            && path
                .strip_prefix(desired)
                .ok()
                .and_then(Path::to_str)
                .and_then(ConvergeFile::from_relative_path)
                .is_none()
        {
            refused.push(StagingRefusal::UnlistedEntry(path));
        }
    }
    for file in ConvergeFile::ALL {
        let path = desired.join(file.relative_path());
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let refusal = StagingRefusal::SymlinkEntry(path);
                if !refused.contains(&refusal) {
                    refused.push(refusal);
                }
            }
            Ok(metadata) if metadata.is_file() => {}
            _ => refused.push(StagingRefusal::MissingFile(path)),
        }
    }
    if refused.is_empty() {
        Ok(())
    } else {
        Err(refused)
    }
}

fn check_components(desired: &Path) -> Result<(), StagingRefusal> {
    if !desired.is_absolute() {
        return Err(StagingRefusal::RelativeDirectory(desired.to_path_buf()));
    }
    let mut walked = PathBuf::new();
    for component in desired.components() {
        walked.push(component);
        if fs::symlink_metadata(&walked).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return Err(StagingRefusal::SymlinkComponent(walked));
        }
    }
    Ok(())
}

fn complete_listing(desired: &Path) -> Result<Vec<(PathBuf, FileType)>, StagingRefusal> {
    let mut directories = vec![desired.to_path_buf()];
    let mut entries = Vec::new();
    while let Some(directory) = directories.pop() {
        let listing = fs::read_dir(&directory)
            .map_err(|_| StagingRefusal::IncompleteListing(directory.clone()))?;
        for entry in listing {
            let entry = entry.map_err(|_| StagingRefusal::IncompleteListing(directory.clone()))?;
            let path = entry.path();
            let kind = fs::symlink_metadata(&path)
                .map_err(|_| StagingRefusal::IncompleteListing(path.clone()))?
                .file_type();
            if kind.is_dir() {
                directories.push(path.clone());
            }
            entries.push((path, kind));
        }
    }
    Ok(entries)
}
