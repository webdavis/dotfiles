//! Every expectation was read off `results-alerter/normalize.sh`, the jq
//! program this replaces.

use super::*;

fn line(body: serde_json::Value) -> String {
    body.to_string()
}

#[test]
fn a_scheduled_detector_arrives_with_its_action_and_its_columns() {
    let batch = line(serde_json::json!({
        "name": "persistence_launchd",
        "action": "added",
        "counter": 4,
        "columns": {"label": "com.example.agent", "path": "/tmp/a.plist"}
    }));
    let rows = rows(&batch);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].detector, Detector::PersistenceLaunchd);
    assert_eq!(rows[0].action, Action::Added);
    assert_eq!(rows[0].column("label"), "com.example.agent");
    assert_eq!(rows[0].enrichment_path, "/tmp/a.plist");
}

#[test]
fn a_packed_query_and_a_top_level_one_arrive_as_the_same_detector() {
    // The pack prefix is the bridge's, not the detector's, and routing that
    // matched on the full name would miss every packed row.
    for name in ["persistence_launchd", "pack_security_persistence_launchd"] {
        let batch = line(serde_json::json!({"name": name, "columns": {}}));
        assert_eq!(
            rows(&batch)[0].detector,
            Detector::PersistenceLaunchd,
            "{name}"
        );
    }
}

#[test]
fn a_query_nothing_schedules_is_dropped_rather_than_alerted_on() {
    // A STRICT ALLOWLIST. An unknown, renamed or rogue query name must never
    // reach a page, and a made-up pack is dropped exactly like a made-up
    // top-level name.
    for name in [
        "heartbeat_canary",
        "pack_security_made_up_query",
        "made_up_query",
        "",
    ] {
        let batch = line(serde_json::json!({"name": name, "columns": {}}));
        assert!(rows(&batch).is_empty(), "{name}");
    }
}

#[test]
fn a_baseline_row_is_seeded_away_and_an_absolute_state_row_is_kept() {
    // osquery emits a differential query's first full result set with counter
    // 0. Paging on those would page the whole machine on the first tick. The
    // three absolute-state queries are the exception, because their very
    // presence is the unsafe state.
    for (name, kept) in [
        ("persistence_launchd", false),
        ("filevault_off", true),
        ("remote_access_sharing_state", true),
        ("agent_exposure_changed", true),
    ] {
        let batch = line(serde_json::json!({"name": name, "counter": 0, "columns": {}}));
        assert_eq!(!rows(&batch).is_empty(), kept, "{name}");
    }
}

#[test]
fn a_row_with_no_counter_is_not_read_as_a_baseline() {
    // Reading the absence as zero would seed away a first observation the
    // domain means to page about.
    let batch = line(serde_json::json!({"name": "persistence_launchd", "columns": {}}));
    assert_eq!(rows(&batch).len(), 1);
}

#[test]
fn a_counter_written_as_a_quoted_number_is_the_same_counter() {
    // osquery's own JSON logger has written it both ways, and a baseline that
    // slipped through as a string would page the whole machine once.
    let batch = line(serde_json::json!({
        "name": "persistence_launchd", "counter": "0", "columns": {}
    }));
    assert!(rows(&batch).is_empty());
}

#[test]
fn write_churn_from_an_atomic_rename_is_not_a_change() {
    // chezmoi and renameio publish a file through a scratch directory, so a
    // file event inside one is the write, not the result.
    let batch = line(serde_json::json!({
        "name": "file_events_recent",
        "columns": {"target_path": "/x/.renameio-TempDir-123/y"}
    }));
    assert!(rows(&batch).is_empty());
}

#[test]
fn a_row_that_is_not_json_costs_that_row_and_no_other() {
    // osquery can be killed mid-write and something else can append to the
    // log, so a batch carrying rubbish is normal. Refusing the batch would
    // drop the findings beside it, which are what this exists to page about.
    let batch = format!(
        "{}\nnot json at all\n{{\"unclosed\": \n[]\n\"a bare string\"\n{}\n",
        line(serde_json::json!({"name": "new_admin_user", "columns": {}})),
        line(serde_json::json!({"name": "suid_bin_unexpected", "columns": {}})),
    );
    let rows = rows(&batch);
    assert_eq!(
        rows.iter().map(|row| row.detector).collect::<Vec<_>>(),
        [Detector::NewAdminUser, Detector::SuidBinUnexpected]
    );
}

#[test]
fn a_row_with_no_action_is_a_change_rather_than_an_addition() {
    // Severity reads the action, and defaulting to `added` would promote every
    // actionless protection row to critical.
    for action in [serde_json::json!(null), serde_json::json!("changed")] {
        let batch = line(serde_json::json!({
            "name": "firewall_state", "action": action, "columns": {"state": "off"}
        }));
        assert_eq!(rows(&batch)[0].action, Action::Other, "{action}");
    }
}

#[test]
fn a_protection_row_says_off_only_when_the_column_does() {
    for (state, expected) in [
        ("off", ProtectionState::Off),
        ("on", ProtectionState::Other),
        ("", ProtectionState::Other),
    ] {
        let batch = line(serde_json::json!({
            "name": "firewall_state", "columns": {"state": state}
        }));
        assert_eq!(rows(&batch)[0].protection, expected, "{state:?}");
    }
}

#[test]
fn a_system_extension_falls_back_to_its_path_when_the_bundle_column_is_blank() {
    // jq read an empty string as absent through `//`, and a fallback that only
    // fired on a MISSING key would enrich the wrong file.
    for bundle in [None, Some("")] {
        let mut columns = serde_json::json!({"path": "/tmp/ext"});
        if let Some(bundle) = bundle {
            columns["bundle_path"] = serde_json::json!(bundle);
        }
        let batch = line(serde_json::json!({
            "name": "system_extensions_new", "columns": columns
        }));
        assert_eq!(rows(&batch)[0].enrichment_path, "/tmp/ext", "{bundle:?}");
    }
    let batch = line(serde_json::json!({
        "name": "system_extensions_new",
        "columns": {"path": "/tmp/ext", "bundle_path": "/tmp/Bundle.app"}
    }));
    assert_eq!(rows(&batch)[0].enrichment_path, "/tmp/Bundle.app");
}

#[test]
fn a_tab_or_newline_in_a_path_reaches_the_enricher_as_a_space() {
    // FSEvents produces these, and the path is rendered into a page.
    let batch = line(serde_json::json!({
        "name": "file_events_recent", "columns": {"target_path": "/a\tb\nc"}
    }));
    assert_eq!(rows(&batch)[0].enrichment_path, "/a b c");
}

#[test]
fn a_row_carrying_no_columns_at_all_still_judges() {
    // A detector whose row lost its columns is still a finding; every column
    // read just answers empty.
    let batch = line(serde_json::json!({"name": "new_admin_user"}));
    let rows = rows(&batch);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].column("username"), "");
    assert_eq!(rows[0].enrichment_path, "");
}

#[test]
fn a_column_that_is_not_a_string_reads_as_empty_rather_than_as_its_digits() {
    // Nothing downstream distinguishes absent from unreadable, and the shell
    // read every column through `// ""`.
    let batch = line(serde_json::json!({
        "name": "listening_ports_non_loopback", "columns": {"port": 8080, "address": null}
    }));
    assert_eq!(rows(&batch)[0].column("port"), "");
    assert_eq!(rows(&batch)[0].column("address"), "");
}

#[test]
fn the_surviving_rows_keep_the_order_osquery_wrote_them_in() {
    // A page reads as a timeline.
    let batch = format!(
        "{}\n{}\n{}\n",
        line(serde_json::json!({"name": "new_admin_user", "columns": {}})),
        line(serde_json::json!({"name": "made_up", "columns": {}})),
        line(serde_json::json!({"name": "sip_state", "columns": {}})),
    );
    assert_eq!(
        rows(&batch).iter().map(|r| r.detector).collect::<Vec<_>>(),
        [Detector::NewAdminUser, Detector::SipState]
    );
}
