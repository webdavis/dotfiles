use super::*;

// --- the operator mute ------------------------------------------------------

/// The mute command, with its state file inside the sandbox.
///
/// `PNS_STATE_DIR` RIDES ON THE COMMAND, never through `set_var`: this binary
/// is threaded, and a process-wide mutation would decide another test's mute.
pub(super) fn quiet_command(sandbox: &Sandbox) -> std::process::Command {
    let mut command = sandbox.pns();
    command.env("PNS_STATE_DIR", sandbox.path("state"));
    command.arg("quiet");
    command
}

/// The state file the mute is published to.
pub(super) fn quiet_state(sandbox: &Sandbox) -> std::path::PathBuf {
    sandbox.path("state/quiet-until")
}

/// When the mute on disk was last written.
pub(super) fn modified_at(connection: &rusqlite::Connection) -> u64 {
    connection
        .pragma_query_value(None, "data_version", |row| row.get(0))
        .expect("the observer's database revision")
}

/// The state directory's mode, which is how a failed publish is reached
/// without a fault-injection point in the binary. ALWAYS PUT BACK before the
/// assertions: a directory left at 0500 is one the sandbox's own cleanup
/// cannot remove.
pub(super) fn set_state_mode(sandbox: &Sandbox, mode: u32) {
    std::fs::set_permissions(
        sandbox.path("state"),
        std::os::unix::fs::PermissionsExt::from_mode(mode),
    )
    .expect("the state directory's mode");
}
