use super::*;
use herdr_workspace_jump_application::WorkspaceHistory;
use herdr_workspace_jump_domain::Mru;
use std::fs;
fn mru(current: &str, previous: &str) -> Mru {
    Mru {
        current: current.to_string(),
        previous: previous.to_string(),
    }
}

struct TempState(PathBuf);

impl TempState {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("hwj-mru-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn path(&self) -> PathBuf {
        self.0.join("mru")
    }
}

impl Drop for TempState {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn record_writes_the_shift_that_next_mru_decided() {
    let state = TempState::new("record");
    record_at(&state.path(), r#"{"data":{"workspace_id":"wA"}}"#);
    record_at(&state.path(), r#"{"data":{"workspace_id":"wB"}}"#);
    assert_eq!(
        FileWorkspaceHistory::at(&state.path()).read(),
        mru("wB", "wA")
    );
}

#[test]
fn record_leaves_the_state_alone_for_a_refocus_or_a_garbage_event() {
    let state = TempState::new("record-noop");
    record_at(&state.path(), r#"{"data":{"workspace_id":"wA"}}"#);
    record_at(&state.path(), r#"{"data":{"workspace_id":"wA"}}"#);
    record_at(&state.path(), "not json");
    assert_eq!(
        FileWorkspaceHistory::at(&state.path()).read(),
        mru("wA", "")
    );
}
