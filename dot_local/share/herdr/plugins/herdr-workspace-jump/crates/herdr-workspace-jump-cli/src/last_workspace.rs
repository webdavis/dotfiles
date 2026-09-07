use std::path::Path;

use herdr_workspace_jump_adapters::{CliWorkspaceDirectory, DEADLINE, SocketWorkspaceDirectory};
use herdr_workspace_jump_application::{JumpError, WorkspaceHistory, bounce};
use herdr_workspace_jump_domain::Bounce;

pub(crate) fn run(
    socket_path: Option<String>,
    herdr_binary: Option<String>,
    history: &mut impl WorkspaceHistory,
) -> Result<Bounce, JumpError> {
    // A focus event can change history before its reply arrives.
    // Both transports must retry the target from the original snapshot.
    let target = history.read();
    let over_socket = match socket_path.filter(|path| !path.is_empty()) {
        Some(path) => {
            let mut directory = SocketWorkspaceDirectory::at(Path::new(&path), DEADLINE);
            bounce(&mut directory, history, &target)
        }
        None => Err(JumpError::Unreachable("HERDR_SOCKET_PATH is unset".into())),
    };
    let socket_failure = match over_socket {
        Ok(outcome) => return Ok(outcome),
        Err(failure) => failure,
    };
    let mut directory = CliWorkspaceDirectory::using(herdr_binary);
    bounce(&mut directory, history, &target).map_err(|cli_failure| JumpError::BothPathsFailed {
        cli: cli_failure.to_string(),
        socket: socket_failure.to_string(),
    })
}

#[cfg(test)]
mod tests;
