use super::*;
use herdr_workspace_jump_domain::Workspace;

#[derive(Default)]
struct MemoryWorkspaceHistory(Mru);

impl WorkspaceHistory for MemoryWorkspaceHistory {
    fn read(&self) -> Mru {
        self.0.clone()
    }
    fn write(&mut self, mru: &Mru) {
        self.0 = mru.clone();
    }
}

fn mru(current: &str, previous: &str) -> Mru {
    Mru {
        current: current.to_string(),
        previous: previous.to_string(),
    }
}

fn workspace(workspace_id: &str) -> Workspace {
    Workspace {
        workspace_id: workspace_id.to_string(),
        label: String::new(),
    }
}

#[derive(Default)]
struct FakeWorkspaceDirectory {
    workspaces: Vec<Workspace>,
    calls: Vec<String>,
}

impl WorkspaceDirectory for FakeWorkspaceDirectory {
    fn list(&mut self) -> Result<Vec<Workspace>, JumpError> {
        self.calls.push("list".to_string());
        Ok(self.workspaces.clone())
    }

    fn focus(&mut self, workspace_id: &str) -> Result<(), JumpError> {
        self.calls.push(format!("focus {workspace_id}"));
        Ok(())
    }

    fn create(&mut self, label: &str, cwd: &str) -> Result<(), JumpError> {
        self.calls.push(format!("create {label} {cwd}"));
        Ok(())
    }
}

#[test]
fn bounce_focuses_the_previous_workspace_and_never_creates() {
    let mut state = MemoryWorkspaceHistory(mru("wA", "wB"));
    let mut directory = FakeWorkspaceDirectory {
        workspaces: vec![workspace("wA"), workspace("wB")],
        ..FakeWorkspaceDirectory::default()
    };

    let target = state.read();
    let outcome = bounce(&mut directory, &mut state, &target);

    assert_eq!(outcome, Ok(Bounce::Focus("wB".to_string())));
    assert_eq!(directory.calls, vec!["list", "focus wB"]);
}

#[test]
fn bounce_forgets_a_previous_workspace_that_no_longer_exists() {
    let mut state = MemoryWorkspaceHistory(mru("wA", "wGone"));
    let mut directory = FakeWorkspaceDirectory {
        workspaces: vec![workspace("wA")],
        ..FakeWorkspaceDirectory::default()
    };

    let target = state.read();
    assert_eq!(
        bounce(&mut directory, &mut state, &target),
        Ok(Bounce::DropStale)
    );
    assert_eq!(directory.calls, vec!["list"]);
    assert_eq!(state.read(), mru("wA", ""));
}

#[test]
fn bounce_without_a_recorded_previous_never_reaches_herdr() {
    let mut state = MemoryWorkspaceHistory::default();
    let mut directory = FakeWorkspaceDirectory::default();
    let target = state.read();
    assert_eq!(
        bounce(&mut directory, &mut state, &target),
        Ok(Bounce::Nothing)
    );
    assert!(directory.calls.is_empty(), "{:?}", directory.calls);
}
