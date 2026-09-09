use posture_domain::{CommandTrustRefusal, LiveAttributes, command_trust};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

#[derive(Debug, PartialEq, Eq)]
pub struct CommandRefusal {
    pub command: PathBuf,
    pub reason: CommandTrustRefusal,
}

pub fn resolve_osqueryctl(
    requested: Option<&Path>,
    path: &OsStr,
) -> Result<Option<PathBuf>, CommandRefusal> {
    use std::os::unix::fs::MetadataExt;
    resolve_with(
        requested,
        path,
        |path| std::fs::metadata(path).is_ok_and(|metadata| metadata.mode() & 0o111 != 0),
        |path| {
            std::fs::symlink_metadata(path)
                .ok()
                .map(|metadata| LiveAttributes {
                    mode: metadata.mode() & 0o7777,
                    uid: metadata.uid(),
                    gid: metadata.gid(),
                })
        },
    )
}

fn resolve_with(
    requested: Option<&Path>,
    path: &OsStr,
    mut executable: impl FnMut(&Path) -> bool,
    mut attributes: impl FnMut(&Path) -> Option<LiveAttributes>,
) -> Result<Option<PathBuf>, CommandRefusal> {
    let resolved = if let Some(requested) = requested.filter(|path| !path.as_os_str().is_empty()) {
        executable(requested).then(|| requested.to_path_buf())
    } else {
        std::env::split_paths(path)
            .map(|directory| directory.join("osqueryctl"))
            .find(|candidate| executable(candidate))
    };
    let Some(command) = resolved else {
        return Ok(None);
    };
    let parent = if command.is_absolute() {
        command.parent().and_then(&mut attributes)
    } else {
        None
    };
    command_trust(command.is_absolute(), parent).map_err(|reason| CommandRefusal {
        command: command.clone(),
        reason,
    })?;
    Ok(Some(command))
}

#[cfg(test)]
mod tests;
