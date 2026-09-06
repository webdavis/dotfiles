//! The most-recently-used workspace toggle.
//!
//! herdr ships `last_pane` but no `last_workspace`. A key-bound script can only
//! observe the switches routed through itself, so it desyncs the moment focus
//! moves by mouse or picker. This tracks herdr's own `workspace.focused` event,
//! which fires for every focus change, and keeps a two-deep list.
//!
//! The toggle needs nothing from herdr that a jump does not already need: it
//! reads the live list to check its target still exists, then focuses it. So it
//! rides the same `WorkspaceDirectory` boundary with no extra port method.

use std::fs;
use std::path::{Path, PathBuf};

use crate::jump::WorkspaceDirectory;
use crate::protocol::{JumpError, Workspace};

/// The two most-recently-focused workspaces. `previous` is the one a toggle
/// jumps back to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mru {
    pub current: String,
    pub previous: String,
}

/// Where the toggle keeps its state.
///
/// herdr injects `HERDR_PLUGIN_STATE_DIR`; the home-relative path is what a run
/// by hand falls back to. Both values are passed in rather than read here, so
/// every caller is handed the one path this resolves and no code below can
/// resolve a second, different one.
pub fn state_file(state_dir: Option<&str>, home: Option<&str>) -> PathBuf {
    let dir = state_dir
        .filter(|dir| !dir.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "{}/.local/state/herdr/plugins/herdr-workspace-jump",
                home.unwrap_or_default()
            )
        });
    PathBuf::from(dir).join("mru")
}

/// Read the state, treating anything unreadable as a cold start.
pub fn read_at(path: &Path) -> Mru {
    let content = fs::read_to_string(path).unwrap_or_default();
    let mut lines = content.lines();
    Mru {
        current: lines.next().unwrap_or_default().trim().to_string(),
        previous: lines.next().unwrap_or_default().trim().to_string(),
    }
}

fn write_at(path: &Path, mru: &Mru) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, format!("{}\n{}\n", mru.current, mru.previous));
}

/// Shift the list for a newly-focused workspace, or None when nothing moved.
///
/// The workspace being left becomes the new previous. On a cold start there is
/// nothing to leave, so the prior previous is kept.
pub fn next_mru(mru: &Mru, new_id: &str) -> Option<Mru> {
    if new_id.is_empty() || new_id == mru.current {
        return None;
    }
    let previous = if mru.current.is_empty() {
        mru.previous.clone()
    } else {
        mru.current.clone()
    };
    Some(Mru {
        current: new_id.to_string(),
        previous,
    })
}

/// Pull the newly-focused workspace id out of a `workspace.focused` payload.
pub fn parse_focused_id(event_json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(event_json)
        .ok()
        .and_then(|event| event["data"]["workspace_id"].as_str().map(str::to_string))
}

/// What the toggle does with the state it read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bounce {
    Focus(String),
    /// The recorded previous workspace is gone; forget it rather than focus
    /// nowhere. `workspace.focus` exits 0 on a stale id, so the live list is
    /// the only thing that can tell the difference.
    DropStale,
    /// Nothing has been recorded yet.
    Nothing,
}

pub fn decide_bounce(mru: &Mru, workspaces: &[Workspace]) -> Bounce {
    if mru.previous.is_empty() {
        return Bounce::Nothing;
    }
    if workspaces
        .iter()
        .any(|workspace| workspace.workspace_id == mru.previous)
    {
        Bounce::Focus(mru.previous.clone())
    } else {
        Bounce::DropStale
    }
}

/// On `workspace.focused`: shift the list if focus actually moved.
pub fn record(state: &Path, event_json: &str) {
    let Some(new_id) = parse_focused_id(event_json) else {
        return;
    };
    let mru = read_at(state);
    if let Some(next) = next_mru(&mru, &new_id) {
        write_at(state, &next);
    }
}

/// Focus the previously-focused workspace, reporting what it did.
///
/// The resulting `workspace.focused` event re-enters `record`, flipping current
/// and previous, so the next invocation comes back here.
pub fn bounce(directory: &mut dyn WorkspaceDirectory, state: &Path) -> Result<Bounce, JumpError> {
    let mru = read_at(state);
    if mru.previous.is_empty() {
        return Ok(Bounce::Nothing);
    }
    let outcome = decide_bounce(&mru, &directory.list()?);
    match &outcome {
        Bounce::Focus(workspace_id) => directory.focus(workspace_id)?,
        Bounce::DropStale => write_at(
            state,
            &Mru {
                current: mru.current,
                previous: String::new(),
            },
        ),
        Bounce::Nothing => {}
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn next_mru_on_a_cold_start_records_only_the_current() {
        assert_eq!(next_mru(&mru("", ""), "A"), Some(mru("A", "")));
    }

    #[test]
    fn next_mru_keeps_a_previous_it_has_no_current_to_replace() {
        // Reachable through a truncated or hand-edited state file, where
        // `read_at` yields an empty current beside a recorded previous. The
        // recorded one is still the better toggle target than nothing.
        assert_eq!(next_mru(&mru("", "wB"), "A"), Some(mru("A", "wB")));
    }

    #[test]
    fn next_mru_shifts_the_workspace_being_left_into_previous() {
        assert_eq!(next_mru(&mru("A", ""), "B"), Some(mru("B", "A")));
        assert_eq!(next_mru(&mru("B", "A"), "C"), Some(mru("C", "B")));
    }

    #[test]
    fn next_mru_ignores_a_refocus_of_the_current_or_an_empty_id() {
        assert_eq!(next_mru(&mru("B", "A"), "B"), None);
        assert_eq!(next_mru(&mru("B", "A"), ""), None);
    }

    #[test]
    fn parse_focused_id_reads_the_workspace_id_and_survives_garbage() {
        let event = r#"{"event":"workspace_focused","data":{"workspace_id":"w18"}}"#;
        assert_eq!(parse_focused_id(event), Some("w18".to_string()));
        assert_eq!(parse_focused_id("not json"), None);
        assert_eq!(parse_focused_id("{}"), None);
    }

    #[test]
    fn decide_bounce_focuses_a_previous_that_still_exists() {
        let workspaces = [workspace("wA"), workspace("wB")];
        assert_eq!(
            decide_bounce(&mru("wA", "wB"), &workspaces),
            Bounce::Focus("wB".to_string())
        );
    }

    #[test]
    fn decide_bounce_drops_a_previous_that_is_gone() {
        assert_eq!(
            decide_bounce(&mru("wA", "wGone"), &[workspace("wA")]),
            Bounce::DropStale
        );
    }

    #[test]
    fn decide_bounce_does_nothing_before_anything_is_recorded() {
        assert_eq!(
            decide_bounce(&mru("wA", ""), &[workspace("wA")]),
            Bounce::Nothing
        );
        assert_eq!(decide_bounce(&mru("", ""), &[]), Bounce::Nothing);
    }

    #[test]
    fn state_file_prefers_the_injected_directory_over_the_home_default() {
        assert_eq!(
            state_file(Some("/tmp/state-override"), Some("/home/ignored")),
            PathBuf::from("/tmp/state-override/mru")
        );
        assert_eq!(
            state_file(None, Some("/home/me")),
            PathBuf::from("/home/me/.local/state/herdr/plugins/herdr-workspace-jump/mru")
        );
        assert_eq!(
            state_file(Some(""), Some("/home/me")),
            PathBuf::from("/home/me/.local/state/herdr/plugins/herdr-workspace-jump/mru")
        );
    }

    /// A scratch state file removed when the test ends.
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
    fn read_at_treats_a_missing_or_short_file_as_a_cold_start() {
        let state = TempState::new("cold");
        assert_eq!(read_at(&state.path()), Mru::default());
        write_at(&state.path(), &mru("wA", ""));
        assert_eq!(read_at(&state.path()), mru("wA", ""));
    }

    #[test]
    fn record_writes_the_shift_that_next_mru_decided() {
        let state = TempState::new("record");
        record(&state.path(), r#"{"data":{"workspace_id":"wA"}}"#);
        record(&state.path(), r#"{"data":{"workspace_id":"wB"}}"#);
        assert_eq!(read_at(&state.path()), mru("wB", "wA"));
    }

    #[test]
    fn record_leaves_the_state_alone_for_a_refocus_or_a_garbage_event() {
        let state = TempState::new("record-noop");
        record(&state.path(), r#"{"data":{"workspace_id":"wA"}}"#);
        record(&state.path(), r#"{"data":{"workspace_id":"wA"}}"#);
        record(&state.path(), "not json");
        assert_eq!(read_at(&state.path()), mru("wA", ""));
    }

    /// Records what reached the directory, so a test can prove the toggle
    /// focused the previous workspace and nothing else.
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
        let state = TempState::new("bounce");
        write_at(&state.path(), &mru("wA", "wB"));
        let mut directory = FakeWorkspaceDirectory {
            workspaces: vec![workspace("wA"), workspace("wB")],
            ..FakeWorkspaceDirectory::default()
        };

        let outcome = bounce(&mut directory, &state.path());

        assert_eq!(outcome, Ok(Bounce::Focus("wB".to_string())));
        assert_eq!(directory.calls, vec!["list", "focus wB"]);
    }

    #[test]
    fn bounce_forgets_a_previous_workspace_that_no_longer_exists() {
        let state = TempState::new("bounce-stale");
        write_at(&state.path(), &mru("wA", "wGone"));
        let mut directory = FakeWorkspaceDirectory {
            workspaces: vec![workspace("wA")],
            ..FakeWorkspaceDirectory::default()
        };

        assert_eq!(bounce(&mut directory, &state.path()), Ok(Bounce::DropStale));
        assert_eq!(directory.calls, vec!["list"]);
        assert_eq!(read_at(&state.path()), mru("wA", ""));
    }

    #[test]
    fn bounce_without_a_recorded_previous_never_reaches_herdr() {
        let state = TempState::new("bounce-cold");
        let mut directory = FakeWorkspaceDirectory::default();
        assert_eq!(bounce(&mut directory, &state.path()), Ok(Bounce::Nothing));
        assert!(directory.calls.is_empty(), "{:?}", directory.calls);
    }
}
