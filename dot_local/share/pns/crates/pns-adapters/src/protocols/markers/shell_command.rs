use super::shell::LIGHTS_SHELL_DIR;
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;

fn marker(pid: u32) -> io::Result<PathBuf> {
    // Only the process invoking this short-lived command can own its marker.
    // getppid has no arguments or memory preconditions and creates no effect.
    let parent = unsafe { libc::getppid() };
    if pid <= 1 || i32::try_from(pid).ok() != Some(parent) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "shell pid must name the calling process",
        ));
    }
    Ok(crate::state_dir()
        .join(LIGHTS_SHELL_DIR)
        .join(pid.to_string()))
}

pub fn begin_shell(pid: u32, command: &str) -> io::Result<()> {
    let path = marker(pid)?;
    if pns_domain::shell_is_interactive(command) {
        return Ok(());
    }
    let Some(epoch) = crate::now_secs() else {
        return Ok(());
    };
    let parent = path.parent().expect("a marker has a parent");
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)?;
    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    writeln!(file, "{epoch}")
}

pub fn end_shell(pid: u32) -> io::Result<()> {
    let path = marker(pid)?;
    // The old prompt ignored unlink errors and still reported the command.
    // Ownership refusal above is different: it must stop before any effect.
    let _ = std::fs::remove_file(path);
    Ok(())
}
