use super::*;

#[test]
fn info_envelope_pins_version_and_unknown_values_without_inventing_a_timestamp() {
    let mut result = TapResult::new(TapOperation::Info);
    result.marker = Some(TapMarker {
        path: "/attention".into(),
        source: "config".into(),
        config_file: "/config.toml".into(),
        exists: None,
        mtime_epoch_secs: None,
        age_secs: None,
        fresh: None,
    });
    result.fail("marker_unreadable", "timestamp unavailable");
    let actual: serde_json::Value = serde_json::from_str(&result.encode().unwrap()).unwrap();
    assert_eq!(
        actual,
        serde_json::json!({
            "schema": "pns.tap/1", "operation": "info", "ok": false,
            "write_status": "not_requested", "marker": {
                "path": "/attention", "source": "config", "config_file": "/config.toml",
                "exists": null, "mtime_epoch_secs": null, "age_secs": null, "fresh": null
            }, "surface": null, "message": "timestamp unavailable", "install": null,
            "error": {"code": "marker_unreadable", "message": "timestamp unavailable"}
        })
    );
}

#[test]
fn reporting_failure_retains_a_successful_write_and_applies_existing_bounds() {
    let mut result = TapResult::new(TapOperation::Tap);
    result.write_status = TapWriteStatus::Recorded;
    result.fail("marker_unreadable", "timestamp unavailable");
    let encoded = result.encode().unwrap();
    assert!(encoded.contains("\"write_status\":\"recorded\""));
    result.message = "x".repeat(crate::MAX_TEXT_CHARS + 1);
    assert_eq!(result.encode().unwrap_err().reason.code(), "text_over_cap");
}
