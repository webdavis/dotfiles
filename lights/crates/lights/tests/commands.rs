mod support;
use lights::Response;
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
        format!("3F - Studio: ON | brightness: 42.75% | scene: Read\n{PIN_STATE}")
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
const REMEMBERING: &str = "[scenes]\nremember_position = true\n";

fn press(home: &support::Home, config: &str, fixture: serde_json::Value) -> Response {
    command_in(
        home,
        &["scene", "next"],
        Some(config),
        vec![(200, fixture), (200, json!({"errors":[],"data":[]}))],
    )
    .0
}
#[test]
fn a_remembered_place_continues_the_rotation_after_the_bridge_forgets() {
    let home = home();
    let config = format!("{}{REMEMBERING}", config());
    assert_eq!(
        press(&home, &config, fixture()).stdout,
        "Room: 3F - Studio | Scene: Energize\n"
    );
    assert_eq!(
        press(&home, &config, fixture_with_no_active_scene()).stdout,
        "Room: 3F - Studio | Scene: Concentrate\n"
    );
}
#[test]
fn without_the_setting_nothing_is_written_and_the_pause_falls_back() {
    let home = home();
    assert_eq!(
        press(&home, config(), fixture()).stdout,
        "Room: 3F - Studio | Scene: Energize\n"
    );
    assert!(!home.path().join("state/position.toml").exists());
    assert_eq!(
        press(&home, config(), fixture_with_no_active_scene()).stdout,
        "Room: 3F - Studio | Scene: Read\n"
    );
}
#[test]
fn transport_timeout_exits_four_without_success() {
    failure(&timeout_command(), 4, "timed out");
}

const PRESETS: &str = "[presets]\n\
     evening = [\n\
       { room = 'studio', scene = 'Energize' },\n\
       { room = 'bedroom', scene = 'Read' },\n\
       { room = 'kitchen', off = true },\n\
     ]\n\
     partial = [\n\
       { room = 'studio', scene = 'Energize' },\n\
       { room = 'kitchen', scene = 'Missing' },\n\
       { room = 'bedroom', scene = 'Read' },\n\
     ]\n\
     doomed = [\n\
       { room = 'No Such Room', scene = 'Read' },\n\
       { room = 'studio', scene = 'Missing' },\n\
     ]\n";

fn preset(args: &[&str], writes: usize) -> (Response, Vec<Vec<u8>>) {
    let mut responses = vec![(200, fixture())];
    responses.extend((0..writes).map(|_| (200, json!({"errors":[],"data":[]}))));
    command(args, Some(&format!("{}{PRESETS}", config())), responses)
}

#[test]
fn preset_applies_every_room_in_order_and_reports_each() {
    let (r, w) = preset(&["preset", "evening"], 3);
    assert_eq!(r.exit, 0);
    assert_eq!(r.stderr, "");
    assert_eq!(
        r.stdout,
        "Room: 3F - Studio | Scene: Energize\n\
         Room: 3F - MBedroom | Scene: Read\n\
         2F - Kitchen: off\n"
    );
    assert_eq!(w.len(), 4);
    for (request, expected) in w[1..].iter().zip([
        "PUT /clip/v2/resource/scene/00000000-0000-0000-0000-000000000008 ",
        "PUT /clip/v2/resource/scene/00000000-0000-0000-0000-000000000005 ",
        "PUT /clip/v2/resource/grouped_light/00000000-0000-0000-0000-000000000021 ",
    ]) {
        assert!(
            String::from_utf8_lossy(request).starts_with(expected),
            "{expected}"
        );
    }
}

#[test]
fn a_failed_room_leaves_the_rest_applied_and_sets_the_exit_code() {
    let (r, w) = preset(&["preset", "partial"], 2);
    assert_eq!(r.exit, 3);
    assert_eq!(
        r.stdout,
        "Room: 3F - Studio | Scene: Energize\nRoom: 3F - MBedroom | Scene: Read\n"
    );
    assert_eq!(r.stderr.lines().count(), 1);
    assert!(r.stderr.contains("Missing") && r.stderr.contains("2F - Kitchen"));
    assert_eq!(w.len(), 3);
}

#[test]
fn the_first_failure_names_the_exit_code_and_a_room_the_bridge_lacks_is_one() {
    let (r, w) = preset(&["preset", "doomed"], 0);
    assert_eq!(r.exit, 2);
    assert_eq!(r.stdout, "");
    assert_eq!(r.stderr.lines().count(), 2);
    assert!(r.stderr.contains("No Such Room"));
    assert!(r.stderr.contains("Missing"));
    assert_eq!(w.len(), 1);
}

#[test]
fn unknown_preset_name_is_a_usage_error_without_a_read() {
    let (r, w) = preset(&["preset", "midnight"], 0);
    failure(&r, 1, "midnight");
    assert!(w.is_empty());
}

#[test]
fn bare_preset_lists_the_configured_presets_without_a_read() {
    let (r, w) = preset(&["preset"], 0);
    assert_eq!(r.exit, 0);
    assert_eq!(r.stdout, "doomed\nevening\npartial\n");
    assert!(w.is_empty());
    let (r, w) = command(&["preset"], Some(config()), vec![]);
    assert_eq!((r.exit, r.stdout.as_str()), (0, ""));
    assert_eq!(r.stderr, "lights: no presets configured\n");
    assert!(w.is_empty());
}

#[path = "commands/notifications.rs"]
mod notifications;

const ALL_DAY_WINDOWS: &str = "[[preset_windows]]\n\
     start = '00:00'\n\
     end = '12:00'\n\
     preset = 'evening'\n\
     [[preset_windows]]\n\
     start = '12:00'\n\
     end = '00:00'\n\
     preset = 'evening'\n";

/// Two windows that between them cover every minute, so the assertion holds at
/// whatever time the suite runs.
#[test]
fn preset_now_applies_the_preset_the_current_window_names() {
    let mut responses = vec![(200, fixture())];
    responses.extend((0..3).map(|_| (200, json!({"errors":[],"data":[]}))));
    let (r, w) = command(
        &["preset", "now"],
        Some(&format!("{}{PRESETS}{ALL_DAY_WINDOWS}", config())),
        responses,
    );
    assert_eq!(r.exit, 0);
    assert_eq!(r.stderr, "");
    assert_eq!(
        r.stdout,
        "Room: 3F - Studio | Scene: Energize\n\
         Room: 3F - MBedroom | Scene: Read\n\
         2F - Kitchen: off\n"
    );
    assert_eq!(w.len(), 4);
}

#[test]
fn preset_now_without_windows_names_what_to_add_without_a_read() {
    let (r, w) = preset(&["preset", "now"], 0);
    failure(&r, 1, "preset_windows");
    assert!(w.is_empty());
}

#[path = "commands/all_rooms.rs"]
mod all_rooms;

#[path = "commands/fade.rs"]
mod fade;
