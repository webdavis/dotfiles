/// A workspace, reduced to the two fields a jump needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub workspace_id: String,
    pub label: String,
}

/// What the jump does once the live workspace list is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Jump {
    Focused(String),
    Created,
}

/// Resolve a chord's label to a jump.
///
/// The match is exact and takes the first hit. Both halves are load-bearing on
/// the real machine: labels are not unique (two workspaces are labelled
/// `dotfiles`), so something has to break the tie, and a workspace labelled
/// `dotfiles modernization` sits beside them, so a prefix or substring match
/// would jump to the wrong one.
pub fn decide(workspaces: &[Workspace], label: &str) -> Jump {
    match workspaces.iter().find(|workspace| workspace.label == label) {
        Some(workspace) => Jump::Focused(workspace.workspace_id.clone()),
        None => Jump::Created,
    }
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

    #[test]
    fn decide_focuses_an_exact_label_match() {
        let workspaces = vec![workspace("wA", "netpulse"), workspace("wB", "dotfiles")];
        assert_eq!(decide(&workspaces, "dotfiles"), Jump::Focused("wB".into()));
    }
    #[test]
    fn decide_takes_the_first_of_two_workspaces_sharing_a_label() {
        // The live session really does hold two workspaces labelled `dotfiles`.
        let workspaces = vec![
            workspace("wW", "dotfiles modernization"),
            workspace("w11", "dotfiles"),
            workspace("w1C", "dotfiles"),
        ];
        assert_eq!(decide(&workspaces, "dotfiles"), Jump::Focused("w11".into()));
    }
    #[test]
    fn decide_does_not_match_a_label_that_merely_contains_the_chords_label() {
        let workspaces = vec![workspace("wW", "dotfiles modernization")];
        assert_eq!(decide(&workspaces, "dotfiles"), Jump::Created);
    }
    #[test]
    fn decide_is_case_sensitive_and_ignores_an_empty_label() {
        let workspaces = vec![workspace("wA", "ivy"), workspace("wB", "")];
        assert_eq!(decide(&workspaces, "Ivy"), Jump::Created);
        assert_eq!(decide(&workspaces, ""), Jump::Focused("wB".into()));
    }
    #[test]
    fn decide_creates_when_nothing_matches_or_nothing_exists() {
        assert_eq!(decide(&[], "homelab"), Jump::Created);
        assert_eq!(
            decide(&[workspace("wA", "netpulse")], "homelab"),
            Jump::Created
        );
    }
}
