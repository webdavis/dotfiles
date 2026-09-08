use super::*;
use serde_json::json;
#[test]
fn an_inventory_holding_two_documents_is_refused_not_read_twice() {
    assert!(read_inventory(r#"{"plugins":{"ok":[]}} {"plugins":{"ok":[]}}"#).is_err());
}
#[test]
fn an_inventory_with_no_plugins_object_or_an_empty_one_is_refused_rather_than_a_quiet_week() {
    for text in [
        "null",
        "{}",
        r#"{"plugins":[]}"#,
        r#"{"plugins":{}}"#,
        r#"{"plugins":{"bad":{}}}"#,
        r#"{"plugins":{"bad":[3]}}"#,
    ] {
        assert!(read_inventory(text).is_err(), "{text}");
    }
}
#[test]
fn a_record_without_a_scope_string_refuses_the_whole_reading_rather_than_reading_as_removed() {
    for scope in [json!(null), json!(3), json!({})] {
        let text =
            json!({"plugins":{"ok":[{"scope":"user","version":"1"}], "bad":[{"scope":scope}]}})
                .to_string();
        assert!(read_inventory(&text).is_err(), "{text}");
    }
    assert!(read_inventory(r#"{"plugins":{"bad":[{}]}}"#).is_err());
}
#[test]
fn only_user_scope_records_are_read() {
    let text = r#"{"plugins":{"z":[{"scope":"project","version":"private"},{"scope":"user","version":"2"}],"a":[{"scope":"user","version":"1","installPath":"private","lastUpdated":"private"}],"empty":[]}}"#;
    assert_eq!(
        read_inventory(text).unwrap(),
        vec![("a".into(), "1".into()), ("z".into(), "2".into())]
    );
    assert_eq!(
        read_inventory(r#"{"plugins":{"a":[{"scope":"project"}]}}"#).unwrap(),
        vec![]
    );
}
#[test]
fn the_fingerprint_is_version_then_commit_then_unknown() {
    let text = r#"{"plugins":{"a":[{"scope":"user","version":"1","gitCommitSha":"c"}],"b":[{"scope":"user","version":"unknown","gitCommitSha":"b"}],"c":[{"scope":"user","version":"","gitCommitSha":"c"}],"d":[{"scope":"user","version":3,"gitCommitSha":""}],"e":[{"scope":"user","gitCommitSha":4}]}}"#;
    assert_eq!(
        read_inventory(text).unwrap(),
        vec![
            ("a".into(), "1".into()),
            ("b".into(), "b".into()),
            ("c".into(), "c".into()),
            ("d".into(), "unknown".into()),
            ("e".into(), "unknown".into())
        ]
    );
}
