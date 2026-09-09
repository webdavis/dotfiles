use super::*;
fn fields() -> toml::Table {
    r#"logs = ["/fixture/one.log", "/fixture/sub/two.log"]
rotate_at_bytes = 16
archives_kept = 3
compressor = "/usr/bin/gzip"
"#
    .parse()
    .unwrap()
}
#[test]
fn a_rotation_config_preserves_its_explicit_paths_and_positive_limits() {
    let parsed = RotateLogsLane::parse_fields("lanes.named", fields()).unwrap();
    assert_eq!(parsed.logs, ["/fixture/one.log", "/fixture/sub/two.log"]);
    assert_eq!(parsed.rotate_at_bytes, 16);
    assert_eq!(parsed.archives_kept, 3);
    assert_eq!(parsed.compressor, "/usr/bin/gzip");
}
#[test]
fn archives_kept_below_one_is_refused_because_it_would_discard_content_outright() {
    for value in [0, -1] {
        let mut table = fields();
        table.insert("archives_kept".into(), value.into());
        let error = RotateLogsLane::parse_fields("lanes.named", table)
            .unwrap_err()
            .detail()
            .to_owned();
        assert!(
            error.contains("archives_kept") && error.contains("at least 1"),
            "{error}"
        );
    }
}
#[test]
fn rotation_config_refuses_missing_mistyped_relative_and_unknown_settings() {
    for key in ["logs", "rotate_at_bytes", "archives_kept", "compressor"] {
        let mut table = fields();
        table.remove(key);
        let error = RotateLogsLane::parse_fields("lanes.named", table)
            .unwrap_err()
            .detail()
            .to_owned();
        assert!(error.contains(key), "{error}");
    }
    for (key, value) in [
        ("logs", toml::Value::Array(vec![])),
        ("logs", "not-a-list".into()),
        ("logs", toml::Value::Array(vec!["relative.log".into()])),
        ("compressor", "gzip".into()),
        ("rotate_at_bytes", 0.into()),
        ("rotate_at_bytes", "16".into()),
        ("archives_kept", true.into()),
        ("typo", 1.into()),
    ] {
        let mut table = fields();
        table.insert(key.into(), value);
        let error = RotateLogsLane::parse_fields("lanes.named", table)
            .unwrap_err()
            .detail()
            .to_owned();
        assert!(
            error.contains("lanes.named") && error.contains(key),
            "{error}"
        );
    }
}
