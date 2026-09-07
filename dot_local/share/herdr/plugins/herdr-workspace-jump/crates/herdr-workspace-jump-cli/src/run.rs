use std::path::Path;

use herdr_workspace_jump_adapters::{CliWorkspaceDirectory, DEADLINE, SocketWorkspaceDirectory};
use herdr_workspace_jump_application::{JumpError, jump};
use herdr_workspace_jump_domain::Jump;

/// Try the socket, then the CLI.
///
/// Re-list before retrying the action. If a create's answer was lost but its
/// workspace is visible in that second list, the retry focuses it. Concurrent
/// changes are not covered by an exactly-once guarantee.
pub(crate) fn run(
    socket_path: Option<String>,
    herdr_binary: Option<String>,
    label: &str,
    cwd: &str,
) -> Result<Jump, JumpError> {
    let over_socket = match socket_path.filter(|path| !path.is_empty()) {
        Some(path) => attempt_socket(Path::new(&path), label, cwd),
        None => Err(JumpError::Unreachable(
            "HERDR_SOCKET_PATH is unset".to_string(),
        )),
    };
    let socket_failure = match over_socket {
        Ok(outcome) => return Ok(outcome),
        Err(failure) => failure,
    };
    let mut directory = CliWorkspaceDirectory::using(herdr_binary);
    jump(&mut directory, label, cwd).map_err(|cli_failure| JumpError::BothPathsFailed {
        cli: cli_failure.to_string(),
        socket: socket_failure.to_string(),
    })
}

fn attempt_socket(path: &Path, label: &str, cwd: &str) -> Result<Jump, JumpError> {
    let mut directory = SocketWorkspaceDirectory::at(path, DEADLINE);
    jump(&mut directory, label, cwd)
}

#[cfg(test)]
mod tests;
