//! Last Workspace, a most-recently-used (MRU) toggle between the two
//! most-recently-focused herdr workspaces.
//!
//! herdr ships `last_pane` (pane MRU) but no `last_workspace`. A key-bound shell
//! script can only observe workspace switches routed through itself, so it
//! desyncs the moment you switch by mouse or the picker. This plugin hooks
//! herdr's `workspace.focused` event, which fires for every focus change, and
//! keeps a 2-deep MRU.
//!
//! Subcommands:
//!   record, (herdr-plugin.toml event) on each workspace.focused, shift current -> previous
//!   bounce, (herdr-plugin.toml action) focus the recorded previous workspace
//!   seed, (chezmoiscript, NOT a herdr command) one-shot warm-up of the MRU
//!             `current` from the live focused workspace, so the first bounce
//!             after install works before any focus event has fired. Idempotent
//!             (no-op when state already exists) and tolerant of a down herdr
//!             server. Owning the seed here, instead of in the build script,
//!             means it resolves the state file via the SAME `state_file()` the
//!             reader uses, so seeder/reader path divergence is impossible.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
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

fn read_mru_at(path: &Path) -> (String, String) {
    let content = fs::read_to_string(path).unwrap_or_default();
    let mut lines = content.lines();
    let current = lines.next().unwrap_or("").trim().to_string();
    let previous = lines.next().unwrap_or("").trim().to_string();
    (current, previous)
}

fn read_mru() -> (String, String) {
    read_mru_at(&state_file())
}

fn write_mru_at(path: &Path, current: &str, previous: &str) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, format!("{current}\n{previous}\n"));
}

fn write_mru(current: &str, previous: &str) {
    write_mru_at(&state_file(), current, previous);
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

/// Extract the currently-focused workspace id from a `herdr workspace list`
/// payload (the same envelope `workspace_list_json` returns).
fn parse_focused_from_list(list: &serde_json::Value) -> Option<String> {
    list["result"]["workspaces"]
        .as_array()?
        .iter()
        .find(|w| w["focused"].as_bool() == Some(true))
        .and_then(|w| w["workspace_id"].as_str().map(str::to_string))
}

/// Decide the MRU to seed. Idempotent: a non-empty `current` means state already
/// exists, so seeding is a no-op (None). Otherwise seed `current = focused` with
/// an empty `previous`, but only when a focused workspace is actually known.
fn seed_decision(current: &str, focused: Option<&str>) -> Option<(String, String)> {
    if !current.is_empty() {
        return None;
    }
    match focused {
        Some(f) if !f.is_empty() => Some((f.to_string(), String::new())),
        _ => None,
    }
}

/// Seed the MRU state file at `path` from `focused`, honoring idempotence.
/// Returns whether a write happened. Reads the existing state through the same
/// helpers the reader uses, so the seeded bytes match exactly what `bounce`
/// later reads back.
fn seed_at(path: &Path, focused: Option<&str>) -> bool {
    let (current, _previous) = read_mru_at(path);
    match seed_decision(&current, focused) {
        Some((c, p)) => {
            write_mru_at(path, &c, &p);
            true
        }
        None => false,
    }
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
/// re-enters record(), flipping current/previous, so the next invocation returns.
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

/// One-shot warm-up of the MRU `current` with the live focused workspace, so the
/// first `bounce` after install works before any workspace.focused event has
/// fired. Idempotent (returns early when state already exists, matching the
/// build script's old `[[ ! -s $state ]]` guard, without even querying herdr)
/// and tolerant of a down herdr server (a failed query seeds nothing and exits
/// 0, same as the reader treating a missing state as cold start).
fn seed() {
    let path = state_file();
    let (current, _previous) = read_mru_at(&path);
    if !current.is_empty() {
        return;
    }
    let focused = workspace_list_json()
        .as_ref()
        .and_then(parse_focused_from_list);
    if !seed_at(&path, focused.as_deref()) {
        eprintln!(
            "last-workspace: could not seed MRU (herdr server down or no focused workspace); it self-seeds on the first workspace focus"
        );
    }
}

fn main() {
    match env::args().nth(1).as_deref() {
        Some("record") => record(),
        Some("bounce") => bounce(),
        Some("seed") => seed(),
        other => {
            eprintln!(
                "last-workspace: unknown subcommand {:?}; expected `record`, `bounce`, or `seed`",
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

    #[test]
    fn parse_focused_from_list_finds_the_focused_workspace() {
        let list = serde_json::json!({
            "result": { "workspaces": [
                {"focused": false, "workspace_id": "wA"},
                {"focused": true,  "workspace_id": "wB"}
            ]}
        });
        assert_eq!(parse_focused_from_list(&list), Some("wB".into()));
    }

    #[test]
    fn parse_focused_from_list_none_focused_or_garbage() {
        let none_focused = serde_json::json!({
            "result": { "workspaces": [{"focused": false, "workspace_id": "wA"}] }
        });
        assert_eq!(parse_focused_from_list(&none_focused), None);
        assert_eq!(parse_focused_from_list(&serde_json::json!({})), None);
    }

    #[test]
    fn seed_decision_cold_start_seeds_current_only() {
        assert_eq!(
            seed_decision("", Some("wA")),
            Some(("wA".into(), String::new()))
        );
    }

    #[test]
    fn seed_decision_is_idempotent_when_state_exists() {
        // A non-empty current means state already exists -> never re-seed.
        assert_eq!(seed_decision("wB", Some("wA")), None);
    }

    #[test]
    fn seed_decision_no_focused_is_noop() {
        assert_eq!(seed_decision("", None), None);
        assert_eq!(seed_decision("", Some("")), None);
    }

    #[test]
    fn seed_at_writes_to_temp_state_when_absent() {
        let dir = env::temp_dir().join(format!("lw-seed-absent-{}", std::process::id()));
        let path = dir.join("mru");
        let _ = fs::remove_dir_all(&dir);
        assert!(seed_at(&path, Some("wA")));
        assert_eq!(fs::read_to_string(&path).unwrap(), "wA\n\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn seed_at_is_noop_when_state_already_present() {
        let dir = env::temp_dir().join(format!("lw-seed-present-{}", std::process::id()));
        let path = dir.join("mru");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, "wB\nwC\n").unwrap();
        assert!(!seed_at(&path, Some("wA")));
        assert_eq!(fs::read_to_string(&path).unwrap(), "wB\nwC\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn state_file_honors_plugin_state_dir_override() {
        // The seed writes through this SAME state_file() the reader uses, so an
        // override must be honored identically -- seeder/reader divergence is
        // structurally impossible. (Sole test touching this env var, so the
        // set/remove cannot race another case.)
        env::set_var("HERDR_PLUGIN_STATE_DIR", "/tmp/lw-override-xyz");
        assert_eq!(
            state_file(),
            PathBuf::from("/tmp/lw-override-xyz").join("mru")
        );
        env::remove_var("HERDR_PLUGIN_STATE_DIR");
    }
}
