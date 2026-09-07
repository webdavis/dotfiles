use herdr_workspace_jump_domain::{Jump, Workspace, decide};

use crate::JumpError;

/// Somewhere that workspaces can be listed, focused and created.
///
/// Two technologies implement it: the socket directly, and the herdr CLI as the
/// fallback. Both are exercised by the same jump, and a fake stands in for both
/// in the tests below.
pub trait WorkspaceDirectory {
    fn list(&mut self) -> Result<Vec<Workspace>, JumpError>;
    fn focus(&mut self, workspace_id: &str) -> Result<(), JumpError>;
    fn create(&mut self, label: &str, cwd: &str) -> Result<(), JumpError>;
}

/// Run one create-or-focus against a directory, reporting which branch ran.
pub fn jump(
    directory: &mut impl WorkspaceDirectory,
    label: &str,
    cwd: &str,
) -> Result<Jump, JumpError> {
    let outcome = decide(&directory.list()?, label);
    match &outcome {
        Jump::Focused(workspace_id) => directory.focus(workspace_id)?,
        Jump::Created => directory.create(label, cwd)?,
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(workspace_id: &str, label: &str) -> Workspace {
        Workspace {
            workspace_id: workspace_id.to_string(),
            label: label.to_string(),
        }
    }

    /// Records every call so a test can assert what reached the directory, and
    /// can be told to fail any one of the three.
    #[derive(Default)]
    struct FakeWorkspaceDirectory {
        workspaces: Vec<Workspace>,
        calls: Vec<String>,
        list_failure: Option<JumpError>,
        focus_failure: Option<JumpError>,
        create_failure: Option<JumpError>,
    }

    impl FakeWorkspaceDirectory {
        fn holding(workspaces: Vec<Workspace>) -> Self {
            Self {
                workspaces,
                ..Self::default()
            }
        }
    }

    impl WorkspaceDirectory for FakeWorkspaceDirectory {
        fn list(&mut self) -> Result<Vec<Workspace>, JumpError> {
            self.calls.push("list".to_string());
            match self.list_failure.clone() {
                Some(failure) => Err(failure),
                None => Ok(self.workspaces.clone()),
            }
        }

        fn focus(&mut self, workspace_id: &str) -> Result<(), JumpError> {
            self.calls.push(format!("focus {workspace_id}"));
            match self.focus_failure.clone() {
                Some(failure) => Err(failure),
                None => Ok(()),
            }
        }

        fn create(&mut self, label: &str, cwd: &str) -> Result<(), JumpError> {
            self.calls.push(format!("create {label} {cwd}"));
            match self.create_failure.clone() {
                Some(failure) => Err(failure),
                None => Ok(()),
            }
        }
    }

    #[test]
    fn jump_lists_then_focuses_the_matching_workspace_and_never_creates() {
        let mut directory = FakeWorkspaceDirectory::holding(vec![workspace("w11", "dotfiles")]);
        let outcome = jump(&mut directory, "dotfiles", "/home/me/dotfiles");
        assert_eq!(outcome, Ok(Jump::Focused("w11".into())));
        assert_eq!(directory.calls, vec!["list", "focus w11"]);
    }
    #[test]
    fn jump_lists_then_creates_with_the_label_and_cwd_when_nothing_matches() {
        let mut directory = FakeWorkspaceDirectory::holding(vec![workspace("wA", "netpulse")]);
        let outcome = jump(&mut directory, "homelab", "/home/me/homelab");
        assert_eq!(outcome, Ok(Jump::Created));
        assert_eq!(
            directory.calls,
            vec!["list", "create homelab /home/me/homelab"]
        );
    }
    #[test]
    fn jump_stops_at_a_failed_list_without_touching_anything() {
        let mut directory = FakeWorkspaceDirectory {
            list_failure: Some(JumpError::Transport("timed out".to_string())),
            ..FakeWorkspaceDirectory::default()
        };
        assert_eq!(
            jump(&mut directory, "dotfiles", "/tmp"),
            Err(JumpError::Transport("timed out".to_string()))
        );
        assert_eq!(directory.calls, vec!["list"]);
    }
    #[test]
    fn jump_reports_a_failed_focus_and_a_failed_create() {
        let refused = JumpError::Server {
            code: "not_found".to_string(),
            message: "gone".to_string(),
        };
        let mut focusing = FakeWorkspaceDirectory {
            focus_failure: Some(refused.clone()),
            ..FakeWorkspaceDirectory::holding(vec![workspace("w11", "dotfiles")])
        };
        assert_eq!(
            jump(&mut focusing, "dotfiles", "/tmp"),
            Err(refused.clone())
        );

        let mut creating = FakeWorkspaceDirectory {
            create_failure: Some(refused.clone()),
            ..FakeWorkspaceDirectory::default()
        };
        assert_eq!(jump(&mut creating, "homelab", "/tmp"), Err(refused));
    }
}
