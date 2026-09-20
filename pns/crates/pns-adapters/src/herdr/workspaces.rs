/// One workspace as `workspace list` reports it: the label the operator
/// recognises it by, whether the session is showing it, and the checkout it
/// was opened on.
///
/// A THIRD READER OF ONE ANSWER, beside `parse_focused_tab` and
/// `workspace_agent_statuses`: the visibility model reads the focused tab and
/// the lamps read the agent statuses, and neither has any business knowing
/// which checkout a workspace sits in.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WorkspaceRow {
    pub label: String,
    pub focused: bool,
    /// The worktree herdr opened this workspace on, EMPTY for a workspace
    /// that names none: a report says it does not know rather than guessing a
    /// path.
    pub checkout_path: String,
}

/// Every workspace herdr listed, in the order it listed them.
///
/// AN UNREADABLE ANSWER IS NO WORKSPACES, which every caller reads as an
/// unknown. A missing field is the empty string or `false` for the same
/// reason: a report that invented a label would name a workspace the operator
/// cannot find.
pub fn parse_workspaces(workspace_list_json: &str) -> Vec<WorkspaceRow> {
    serde_json::from_str::<serde_json::Value>(workspace_list_json)
        .ok()
        .as_ref()
        .and_then(|body| body.pointer("/result/workspaces"))
        .and_then(serde_json::Value::as_array)
        .map(|workspaces| workspaces.iter().map(row).collect())
        .unwrap_or_default()
}

fn row(workspace: &serde_json::Value) -> WorkspaceRow {
    let text = |pointer: &str| {
        workspace
            .pointer(pointer)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    WorkspaceRow {
        label: text("/label"),
        focused: workspace
            .get("focused")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        checkout_path: text("/worktree/checkout_path"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTED: &str = r#"{"result":{"workspaces":[
      {"label":"homelab","focused":false},
      {"label":"dotfiles","focused":true,
       "worktree":{"checkout_path":"/repo/dotfiles"}}]}}"#;

    #[test]
    fn every_listed_workspace_answers_its_label_focus_and_checkout() {
        assert_eq!(
            parse_workspaces(LISTED),
            vec![
                WorkspaceRow {
                    label: "homelab".into(),
                    focused: false,
                    checkout_path: String::new(),
                },
                WorkspaceRow {
                    label: "dotfiles".into(),
                    focused: true,
                    checkout_path: "/repo/dotfiles".into(),
                },
            ]
        );
    }

    #[test]
    fn an_answer_this_cannot_read_is_no_workspaces_at_all() {
        for answer in ["", "not json", "{}", r#"{"result":{"workspaces":{}}}"#] {
            assert!(parse_workspaces(answer).is_empty(), "{answer}");
        }
    }
}
