mod support;
use serde_json::json;
use support::*;

#[test]
fn missing_config_exits_five() {
    let (r, w) = command(&[], None, vec![]);
    failure(&r, 5, "config");
    assert!(w.is_empty());
}
#[test]
fn malformed_config_exits_five() {
    let (r, w) = command(&[], Some("[broken"), vec![]);
    failure(&r, 5, "config");
    assert!(w.is_empty());
}
#[test]
fn missing_bridge_address_or_key_exits_five() {
    for config in [
        "[controller]\ntype='hue'\nkey='test-secret'",
        "[controller]\ntype='hue'\naddress='192.0.2.1'",
    ] {
        let (r, w) = command(&[], Some(config), vec![]);
        failure(&r, 5, "missing");
        assert!(w.is_empty());
    }
}
#[test]
fn default_command_toggles_configured_room() {
    for args in [vec![], vec!["toggle"]] {
        let (r, w) = accepted(&args);
        assert_eq!(r.exit, 0);
        assert_eq!(r.stdout, "3F - Studio: off\n");
        assert_eq!(w.len(), 2);
        assert_eq!(body(&w[1]), json!({"on":{"on":false}}));
    }
}
#[test]
fn unknown_room_exits_two_on_stderr_without_write() {
    let (r, w) = accepted(&["--room", "Missing", "toggle"]);
    failure(&r, 2, "Missing");
    assert_eq!(w.len(), 1);
}
#[test]
fn unknown_scene_exits_three_on_stderr_without_write() {
    let (r, w) = accepted(&["scene", "Missing"]);
    failure(&r, 3, "Missing");
    assert!(r.stderr.contains("3F - Studio"));
    assert_eq!(w.len(), 1);
}
#[test]
fn unsuccessful_status_exits_four() {
    let (r, w) = command(
        &[],
        Some(config()),
        vec![(403, json!({"errors":[],"data":[]}))],
    );
    failure(&r, 4, "403");
    assert_eq!(w.len(), 1);
}
#[test]
fn status_scene_lookup_failure_exits_four_without_report() {
    let mut data = fixture();
    data["data"][6]["status"] = json!({});
    let (r, w) = command(&["status"], Some(config()), vec![(200, data)]);
    failure(&r, 4, "response");
    assert_eq!(w.len(), 1);
}
#[test]
fn status_uses_one_bulk_read() {
    let (r, w) = accepted(&["status"]);
    assert_eq!(r.exit, 0);
    assert_eq!(
        r.stdout,
        "3F - Studio: ON | brightness: 42.75% | scene: Read\n"
    );
    assert_eq!(w.len(), 1);
}
#[test]
fn absolute_brightness_composes_clamp_and_requested_output() {
    let (r, w) = accepted(&["brightness", "0"]);
    assert_eq!(r.exit, 0);
    assert_eq!(r.stdout, "3F - Studio: brightness requested 1%\n");
    assert_eq!(w.len(), 2);
    assert_eq!(body(&w[1]), json!({"dimming":{"brightness":1}}));
}
#[test]
fn configured_step_reaches_real_adapter() {
    let (r, w) = command(
        &["brightness", "down"],
        Some(&format!("{}[brightness]\nstep=5", config())),
        vec![(200, fixture()), (200, json!({"errors":[],"data":[]}))],
    );
    assert_eq!(r.exit, 0);
    assert_eq!(r.stdout, "3F - Studio: brightness down\n");
    assert_eq!(w.len(), 2);
    assert_eq!(
        body(&w[1]),
        json!({"dimming_delta":{"action":"down","brightness_delta":5}})
    );
}
#[test]
fn named_and_rotated_scenes_use_real_adapter() {
    for (name, id) in [
        ("Read", 6),
        ("next", 8),
        ("previous", 7),
        ("CC Halo Amber", 10),
        ("CC Halo Daylight", 11),
    ] {
        let (r, w) = accepted(&["scene", name]);
        assert_eq!(r.exit, 0);
        assert_eq!(w.len(), 2);
        assert!(String::from_utf8_lossy(&w[1]).starts_with(&format!(
            "PUT /clip/v2/resource/scene/00000000-0000-0000-0000-{id:012} "
        )));
    }
}
#[test]
fn transport_timeout_exits_four_without_success() {
    failure(&timeout_command(), 4, "timed out");
}
