use super::*;

fn strings(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_string()).collect()
}

fn workspace(label: &str, focused: bool, checkout_path: &str) -> WorkspaceRow {
    workspace_in(label, focused, checkout_path, "")
}

fn workspace_in(label: &str, focused: bool, checkout_path: &str, repo_name: &str) -> WorkspaceRow {
    WorkspaceRow {
        label: label.to_string(),
        focused,
        checkout_path: checkout_path.to_string(),
        repo_name: repo_name.to_string(),
    }
}

#[test]
fn each_flag_names_one_form_and_anything_else_is_refused() {
    assert_eq!(Form::of(&strings(&[])), Some(Form::Page));
    assert_eq!(Form::of(&strings(&["--json"])), Some(Form::Json));
    assert_eq!(Form::of(&strings(&["--notify"])), Some(Form::Notify));
    for tail in [
        strings(&["--json", "--notify"]),
        strings(&["--jsn"]),
        strings(&["now"]),
    ] {
        assert_eq!(Form::of(&tail), None, "{tail:?}");
    }
}

#[test]
fn the_branchs_own_worktree_wins_over_the_focused_workspaces_checkout() {
    let workspaces = [
        workspace("dotfiles", true, "/repo/dotfiles"),
        workspace("lane", false, "/worktrees/dotfiles/feat-resume"),
    ];
    assert_eq!(
        worktree_of(&workspaces, "feat/resume", "dotfiles"),
        "/worktrees/dotfiles/feat-resume"
    );
}

#[test]
fn a_branch_no_workspace_was_opened_on_is_not_known_even_with_a_focused_workspace() {
    let workspaces = [workspace("dotfiles", true, "/repo/dotfiles")];
    assert_eq!(worktree_of(&workspaces, "main", "dotfiles"), "");
}

#[test]
fn a_herdr_that_answered_nothing_is_not_known() {
    assert_eq!(worktree_of(&[], "feat/resume", "dotfiles"), "");
}

#[test]
fn a_slug_shared_by_two_repositories_is_broken_by_the_sessions_project() {
    let workspaces = [
        workspace_in("other", false, "/worktrees/other/feat-resume", "other"),
        workspace_in(
            "dotfiles",
            false,
            "/worktrees/dotfiles/feat-resume",
            "dotfiles",
        ),
    ];
    assert_eq!(
        worktree_of(&workspaces, "feat/resume", "dotfiles"),
        "/worktrees/dotfiles/feat-resume"
    );
}

#[test]
fn the_page_names_every_answer_it_was_given() {
    let page = ResumePage {
        workspace: "dotfiles modernization".into(),
        waiting: true,
        title: "ship the resume page".into(),
        branch: "feat/resume".into(),
        worktree: "/worktrees/dotfiles/feat-resume".into(),
        command: "cargo".into(),
    };
    let printed = render(Paint::Plain, &page).join("\n");
    for fact in [
        "dotfiles modernization",
        "ship the resume page",
        "feat/resume",
        "/worktrees/dotfiles/feat-resume",
        "cargo",
    ] {
        assert!(printed.contains(fact), "{fact} missing from {printed}");
    }
}

#[test]
fn a_page_with_nothing_waiting_says_so_in_one_line() {
    let printed = render(Paint::Plain, &ResumePage::default()).join("\n");
    assert!(printed.contains("Nothing is waiting on you."), "{printed}");
    assert!(
        printed.contains("herdr named no focused workspace"),
        "{printed}"
    );
    assert!(
        printed.contains("no command has been timed yet"),
        "{printed}"
    );
}

#[test]
fn the_submitted_page_is_an_observation_from_pns_carrying_the_page_itself() {
    let request = request_for("the page").expect("the envelope");
    assert_eq!(request.producer.as_str(), "pns");
    assert_eq!(request.state, pns_protocol::State::Observation);
    assert_eq!(request.detail, "the page");
    assert!(
        request.branch.is_none(),
        "a branch would prefix the message"
    );
}
