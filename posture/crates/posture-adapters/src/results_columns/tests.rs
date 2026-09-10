//! The category names are checked against the osquery config that STAMPS them
//! (`osquery-converge/desired/osquery.conf.tmpl`, the `file_paths` block), not
//! against the alerter that read them.

use super::*;
use crate::rows;

fn row(columns: serde_json::Value) -> ResultsRow {
    let line = serde_json::json!({"name": "file_events_recent", "columns": columns}).to_string();
    rows(&line).pop().expect("one row")
}

#[test]
fn a_column_the_row_has_nothing_to_say_for_is_absent_rather_than_blank() {
    // osquery writes a column it has no value for as an empty string, and the
    // page renders `Some("")` as a labelled row with nothing after the label.
    let row = row(serde_json::json!({"label": "", "program": "/bin/x"}));
    let columns = row.page_columns();
    assert_eq!(columns.label, None);
    assert_eq!(columns.program, Some("/bin/x"));
}

#[test]
fn every_column_the_page_renders_is_carried_across() {
    // A column that silently failed to project would render as a page block
    // missing the one fact that identified the finding.
    let row = row(serde_json::json!({
        "label": "com.example.agent", "program": "/bin/p", "name": "n", "command": "c",
        "path": "/tmp/p", "username": "u", "uid": "501", "address": "1.2.3.4", "port": "22",
        "service": "s", "identifier": "id", "team": "t", "target_path": "/tmp/t",
        "category": "ssh", "action": "added", "filename": "f", "dest_filename": "d"
    }));
    let c = row.page_columns();
    for (got, expected) in [
        (c.label, "com.example.agent"),
        (c.program, "/bin/p"),
        (c.name, "n"),
        (c.command, "c"),
        (c.path, "/tmp/p"),
        (c.username, "u"),
        (c.uid, "501"),
        (c.address, "1.2.3.4"),
        (c.port, "22"),
        (c.service, "s"),
        (c.identifier, "id"),
        (c.team, "t"),
        (c.target_path, "/tmp/t"),
        (c.category, "ssh"),
        (c.action, "added"),
        (c.filename, "f"),
        (c.dest_filename, "d"),
    ] {
        assert_eq!(got, Some(expected));
    }
}

#[test]
fn every_watched_tree_the_osquery_config_names_maps_to_its_own_category() {
    // THE EIGHT NAMES ARE THE CONFIG'S, verified against the `file_paths` block
    // of `osquery.conf.tmpl` on 2026-09-09. A name that stopped matching would
    // silently drop a whole watched tree to the `Other` tier, which is the tier
    // that does not page.
    for (name, expected) in [
        ("ssh", FileCategory::Ssh),
        ("sshd_config", FileCategory::SshdConfig),
        ("pipeline_integrity", FileCategory::PipelineIntegrity),
        ("managed_bin", FileCategory::ManagedBin),
        ("launch_agents", FileCategory::LaunchAgents),
        ("launch_daemons", FileCategory::LaunchDaemons),
        ("allowlist_file", FileCategory::AllowlistFile),
        ("sudoers", FileCategory::Sudoers),
    ] {
        let row = row(serde_json::json!({"category": name}));
        assert_eq!(row.gate_columns().file_category, expected, "{name}");
    }
}

#[test]
fn a_category_this_does_not_know_is_judged_rather_than_refused() {
    // Adding a watched tree to the osquery config must not make its events
    // unjudgeable.
    for name in ["", "a_tree_added_later", "SSH"] {
        let row = row(serde_json::json!({"category": name}));
        assert_eq!(
            row.gate_columns().file_category,
            FileCategory::Other,
            "{name:?}"
        );
    }
}

#[test]
fn the_launchd_tuple_carries_all_three_fields_the_allowlist_matches_on() {
    let row = row(serde_json::json!({
        "label": "com.example.agent", "path": "/tmp/a.plist", "program": "/usr/bin/true"
    }));
    assert_eq!(
        row.gate_columns().launchd,
        LaunchdIdentity {
            label: "com.example.agent",
            path: "/tmp/a.plist",
            program: "/usr/bin/true"
        }
    );
}

#[test]
fn a_row_missing_part_of_the_tuple_leaves_those_fields_empty_rather_than_guessing() {
    // AN ALLOWLIST ENTRY MATCHES ON ALL THREE. A row carrying only a label must
    // not match an entry that named all three, so the empty fields stay empty
    // and the comparison fails, which is the direction that pages.
    let row = row(serde_json::json!({"label": "com.example.agent"}));
    let tuple = row.gate_columns().launchd;
    assert_eq!(tuple.label, "com.example.agent");
    assert_eq!(tuple.path, "");
    assert_eq!(tuple.program, "");
}

#[test]
fn the_gates_target_path_comes_from_whichever_column_the_detector_uses() {
    // A file event names `target_path`; a launchd or suid row names `path`. The
    // gate asks one question of all of them.
    let event = row(serde_json::json!({"target_path": "/tmp/t", "path": "/tmp/p"}));
    assert_eq!(event.gate_columns().target_path, "/tmp/t");

    let launchd = row(serde_json::json!({"path": "/tmp/p"}));
    assert_eq!(launchd.gate_columns().target_path, "/tmp/p");

    let blank = row(serde_json::json!({"target_path": "", "path": "/tmp/p"}));
    assert_eq!(
        blank.gate_columns().target_path,
        "/tmp/p",
        "an empty target_path is no target_path"
    );

    let neither = row(serde_json::json!({}));
    assert_eq!(neither.gate_columns().target_path, "");
}
