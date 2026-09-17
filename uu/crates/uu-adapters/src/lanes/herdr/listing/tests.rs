use super::*;

/// The envelope herdr answers with, trimmed to the fields this reads.
fn envelope(plugins: &str) -> String {
    format!(
        "{{\"id\":\"cli:plugin\",\"result\":{{\"plugins\":[{plugins}],\"type\":\"plugin_list\"}}}}"
    )
}

#[test]
fn a_plugin_installed_at_a_requested_ref_is_read_with_that_ref_and_its_commit() {
    let listed = parse_plugin_list(&envelope(
        "{\"plugin_id\":\"clauth\",\"source\":{\"kind\":\"github\",\"requested_ref\":\"v0.15.2\",\
         \"resolved_commit\":\"46f04adcc516e2911020f8c192e28525f73b2dbf\"}}",
    ))
    .expect("the envelope must read");
    assert_eq!(
        listed.get("clauth"),
        Some(&Installed {
            requested_ref: Some("v0.15.2".to_string()),
            commit: Some("46f04adcc516e2911020f8c192e28525f73b2dbf".to_string()),
        })
    );
}

#[test]
fn a_plugin_installed_from_tip_records_a_commit_and_no_requested_ref() {
    let listed = parse_plugin_list(&envelope(
        "{\"plugin_id\":\"worktrunk\",\"source\":{\"kind\":\"github\",\"resolved_commit\":\"4be9bba\"}}",
    ))
    .expect("the envelope must read");
    assert_eq!(
        listed.get("worktrunk"),
        Some(&Installed {
            requested_ref: None,
            commit: Some("4be9bba".to_string()),
        })
    );
}

#[test]
fn an_answer_that_carries_no_plugin_list_is_an_error_not_an_empty_machine() {
    // herdr answers its own errors as an envelope and still exits 0. Reading
    // one as "nothing is installed" would report every pin as missing.
    for stdout in [
        "{\"id\":\"cli:plugin\",\"error\":{\"message\":\"server unreachable\"}}",
        "{}",
        "",
        "not json at all",
    ] {
        assert!(parse_plugin_list(stdout).is_err(), "{stdout}");
    }
}

#[test]
fn a_listed_entry_with_no_id_is_skipped_rather_than_keyed_on_nothing() {
    let listed = parse_plugin_list(&envelope(
        "{\"source\":{\"kind\":\"local\"}},{\"plugin_id\":\"a\"}",
    ))
    .expect("the envelope must read");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed.get("a"), Some(&Installed::default()));
}

#[test]
fn a_pin_is_held_by_its_requested_ref_by_its_commit_and_by_a_long_enough_prefix() {
    let at = Installed {
        requested_ref: Some("v1.2.0".to_string()),
        commit: Some("deadbeef1234".to_string()),
    };
    assert!(at.holds("v1.2.0"));
    assert!(at.holds("deadbeef1234"));
    assert!(at.holds("deadbee"));
    // SIX CHARACTERS IS NOT A REVISION. A prefix that short matches commits
    // this plugin was never at, so it is not accepted as the pin.
    assert!(!at.holds("deadbe"));
    assert!(!at.holds("v1.3.0"));
    assert!(!Installed::default().holds("v1.2.0"));
}

#[test]
fn the_pending_sentence_names_where_it_sits_and_the_command_that_moves_it() {
    let at = Installed {
        requested_ref: Some("v1.0.0".to_string()),
        commit: None,
    };
    let moved = pin_sentence("/opt/herdr", "a", "o/a", "v1.2.0", Some(&at));
    assert_eq!(
        moved,
        "plugin a is pinned at v1.2.0 and is installed at v1.0.0. Run the following command to \
         install that revision: /opt/herdr plugin install o/a --ref v1.2.0 --yes"
    );
    let absent = pin_sentence("/opt/herdr", "a", "o/a", "v1.2.0", None);
    assert!(absent.contains("is not installed"), "{absent}");
    assert!(
        absent.contains("/opt/herdr plugin install o/a --ref v1.2.0 --yes"),
        "{absent}"
    );
}

#[test]
fn a_copy_that_records_neither_field_still_reads_as_somewhere() {
    assert_eq!(
        Installed::default().revision(),
        "a revision it does not record"
    );
}
