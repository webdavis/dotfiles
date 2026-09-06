//! The fallback path: the same three operations through the `herdr` CLI.
//!
//! herdr's own guidance is to prefer `HERDR_BIN_PATH` over the raw socket,
//! because the socket's transport is platform specific and the CLI carries a
//! client/server compatibility layer the raw protocol does not. This plugin
//! takes the socket first for speed and keeps the CLI for every case where the
//! socket does not produce an answer.

use std::process::Command;

use crate::jump::WorkspaceDirectory;
use crate::protocol::{self, JumpError, Workspace};

pub struct CliWorkspaceDirectory {
    binary: String,
}

impl CliWorkspaceDirectory {
    /// `HERDR_BIN_PATH` is injected into every plugin command; the bare name is
    /// the fallback for a run started by hand.
    pub fn using(binary: Option<String>) -> Self {
        let binary = binary.filter(|path| !path.is_empty());
        Self {
            binary: binary.unwrap_or_else(|| "herdr".to_string()),
        }
    }

    /// Run one `herdr` subcommand and unwrap the envelope it prints.
    ///
    /// The exit code alone is not trusted: `herdr` can print an error envelope
    /// and still exit 0, which is why the body is parsed either way.
    fn run(&self, arguments: &[&str]) -> Result<serde_json::Value, JumpError> {
        let output = Command::new(&self.binary)
            .args(arguments)
            .output()
            .map_err(|error| JumpError::Unreachable(format!("{}: {error}", self.binary)))?;
        let body = String::from_utf8_lossy(&output.stdout);
        let result = protocol::result_of(&body);
        if !output.status.success() && result.is_ok() {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(JumpError::Transport(format!(
                "{} {} exited {}: {detail}",
                self.binary,
                arguments.join(" "),
                output.status
            )));
        }
        result
    }
}

impl WorkspaceDirectory for CliWorkspaceDirectory {
    fn list(&mut self) -> Result<Vec<Workspace>, JumpError> {
        let result = self.run(&["workspace", "list"])?;
        protocol::workspaces_of(&result)
    }

    fn focus(&mut self, workspace_id: &str) -> Result<(), JumpError> {
        self.run(&["workspace", "focus", workspace_id])?;
        Ok(())
    }

    fn create(&mut self, label: &str, cwd: &str) -> Result<(), JumpError> {
        self.run(&[
            "workspace",
            "create",
            "--cwd",
            cwd,
            "--label",
            label,
            "--focus",
        ])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn using_prefers_the_injected_path_and_falls_back_to_the_bare_name() {
        assert_eq!(
            CliWorkspaceDirectory::using(Some("/opt/herdr".to_string())).binary,
            "/opt/herdr"
        );
        assert_eq!(CliWorkspaceDirectory::using(None).binary, "herdr");
        assert_eq!(
            CliWorkspaceDirectory::using(Some(String::new())).binary,
            "herdr"
        );
    }

    #[test]
    fn a_binary_that_cannot_be_spawned_is_unreachable() {
        let mut directory =
            CliWorkspaceDirectory::using(Some("/nonexistent/herdr-binary".to_string()));
        assert!(matches!(directory.list(), Err(JumpError::Unreachable(_))));
    }

    #[test]
    fn a_non_zero_exit_with_no_envelope_is_a_transport_failure() {
        // `false` prints nothing and exits 1, which is the shape of a herdr that
        // died before it could answer.
        let mut directory = CliWorkspaceDirectory::using(Some("false".to_string()));
        assert!(matches!(directory.list(), Err(JumpError::Transport(_))));
    }

    #[test]
    fn an_error_envelope_is_reported_even_when_the_command_exits_zero() {
        // `echo` exits 0 and replays its arguments, so only the body can carry
        // the failure. This is the documented herdr behavior the exit code misses.
        let directory = CliWorkspaceDirectory::using(Some("echo".to_string()));
        let failure = directory.run(&[r#"{"error":{"code":"not_found","message":"gone"}}"#]);
        assert_eq!(
            failure,
            Err(JumpError::Server {
                code: "not_found".to_string(),
                message: "gone".to_string(),
            })
        );
    }

    #[test]
    fn a_successful_envelope_is_parsed_into_workspaces() {
        let directory = CliWorkspaceDirectory::using(Some("echo".to_string()));
        // `echo` replays its arguments, so this stands in for a herdr that
        // answered with a one-workspace list.
        let replayed = directory
            .run(&[r#"{"result":{"workspaces":[{"workspace_id":"wA","label":"netpulse"}]}}"#]);
        let result = match replayed {
            Ok(result) => result,
            Err(error) => panic!("expected the envelope to parse: {error}"),
        };
        assert_eq!(
            protocol::workspaces_of(&result),
            Ok(vec![Workspace {
                workspace_id: "wA".to_string(),
                label: "netpulse".to_string(),
            }])
        );
    }
}
