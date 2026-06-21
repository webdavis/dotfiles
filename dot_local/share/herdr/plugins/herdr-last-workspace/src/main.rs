//! Last Workspace — a most-recently-used (MRU) toggle between the two
//! most-recently-focused herdr workspaces.
//!
//! herdr ships `last_pane` (pane MRU) but no `last_workspace`. A key-bound shell
//! script can only observe workspace switches routed through itself, so it
//! desyncs the moment you switch by mouse or the picker. This plugin hooks
//! herdr's `workspace.focused` event, which fires for every focus change, and
//! keeps a 2-deep MRU.
//!
//! Subcommands (declared in herdr-plugin.toml):
//!   record  — on each workspace.focused event, shift current -> previous
//!   bounce  — focus the recorded previous workspace (backs the last_workspace action)

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Extract the newly-focused workspace id from a workspace.focused event payload.
fn parse_focused_id(event_json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(event_json)
        .ok()
        .and_then(|v| v["data"]["workspace_id"].as_str().map(str::to_string))
}

/// Given the stored MRU (current, previous) and the newly-focused id, compute the
/// next (current, previous), or None if nothing should change. The workspace being
/// left becomes the new previous; on cold start (empty current) the prior previous
/// is kept.
fn next_mru(current: &str, previous: &str, new_id: &str) -> Option<(String, String)> {
    if new_id.is_empty() || new_id == current {
        return None;
    }
    let new_previous = if current.is_empty() {
        previous
    } else {
        current
    };
    Some((new_id.to_string(), new_previous.to_string()))
}

fn state_file() -> PathBuf {
    let dir = env::var("HERDR_PLUGIN_STATE_DIR").unwrap_or_else(|_| {
        let home = env::var("HOME").unwrap_or_default();
        format!("{home}/.local/state/herdr/plugins/herdr-last-workspace")
    });
    PathBuf::from(dir).join("mru")
}

fn read_mru() -> (String, String) {
    let content = fs::read_to_string(state_file()).unwrap_or_default();
    let mut lines = content.lines();
    let current = lines.next().unwrap_or("").trim().to_string();
    let previous = lines.next().unwrap_or("").trim().to_string();
    (current, previous)
}

fn write_mru(current: &str, previous: &str) {
    let path = state_file();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, format!("{current}\n{previous}\n"));
}

fn herdr_bin() -> String {
    env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".to_string())
}

/// `herdr workspace list` emits JSON on stdout (no --json flag).
fn workspace_list_json() -> Option<serde_json::Value> {
    let out = Command::new(herdr_bin())
        .args(["workspace", "list"])
        .output()
        .ok()?;
    serde_json::from_slice(&out.stdout).ok()
}

fn workspace_exists(list: &serde_json::Value, id: &str) -> bool {
    list["result"]["workspaces"]
        .as_array()
        .map(|ws| ws.iter().any(|w| w["workspace_id"].as_str() == Some(id)))
        .unwrap_or(false)
}

/// On workspace.focused: shift the MRU if the focus actually moved.
fn record() {
    let event = env::var("HERDR_PLUGIN_EVENT_JSON").unwrap_or_default();
    let Some(new_id) = parse_focused_id(&event) else {
        return;
    };
    let (current, previous) = read_mru();
    if let Some((c, p)) = next_mru(&current, &previous, &new_id) {
        write_mru(&c, &p);
    }
}

/// Focus the recorded previous workspace. The resulting workspace.focused event
/// re-enters record(), flipping current/previous — so the next invocation returns.
fn bounce() {
    let (current, previous) = read_mru();
    if previous.is_empty() {
        return;
    }
    // `herdr workspace focus` exits 0 even on a stale id, so guard on the live
    // list: if the target is gone, drop it instead of focusing nowhere.
    if let Some(list) = workspace_list_json() {
        if !workspace_exists(&list, &previous) {
            write_mru(&current, "");
            return;
        }
    }
    let _ = Command::new(herdr_bin())
        .args(["workspace", "focus", &previous])
        .status();
}

fn main() {
    match env::args().nth(1).as_deref() {
        Some("record") => record(),
        Some("bounce") => bounce(),
        other => {
            eprintln!(
                "last-workspace: unknown subcommand {:?}; expected `record` or `bounce`",
                other.unwrap_or("")
            );
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_mru_cold_start() {
        assert_eq!(next_mru("", "", "A"), Some(("A".into(), "".into())));
    }

    #[test]
    fn next_mru_shifts_current_to_previous() {
        assert_eq!(next_mru("A", "", "B"), Some(("B".into(), "A".into())));
        assert_eq!(next_mru("B", "A", "C"), Some(("C".into(), "B".into())));
    }

    #[test]
    fn next_mru_refocus_same_is_noop() {
        assert_eq!(next_mru("B", "A", "B"), None);
    }

    #[test]
    fn next_mru_empty_new_is_noop() {
        assert_eq!(next_mru("B", "A", ""), None);
    }

    #[test]
    fn parse_focused_id_extracts_workspace_id() {
        let ev = r#"{"event":"workspace_focused","data":{"type":"workspace_focused","workspace_id":"w18"}}"#;
        assert_eq!(parse_focused_id(ev), Some("w18".into()));
    }

    #[test]
    fn parse_focused_id_handles_garbage() {
        assert_eq!(parse_focused_id("not json"), None);
        assert_eq!(parse_focused_id("{}"), None);
    }
}
