use super::*;
use crate::{LaneAdapter, LaneRegistration};
#[test]
fn a_plugin_lane_keeps_the_absolute_inventory_it_was_given_under_any_name() {
    let fields: toml::Table = r#"inventory = "/owned/plugins.json""#.parse().unwrap();
    assert_eq!(
        ClaudePluginsLane::parse("lanes.personal", fields)
            .unwrap()
            .inventory,
        "/owned/plugins.json"
    );
    assert!(
        crate::config::parse_config(
            r#"[lanes.personal]
type = "claude-plugins"
inventory = "/owned/plugins.json""#,
            &[LaneRegistration::new::<ClaudePluginsLane>("claude-plugins")]
        )
        .is_ok()
    );
}
#[test]
fn a_plugin_lane_refuses_a_missing_relative_or_mistyped_inventory() {
    for text in [
        "",
        r#"inventory = "relative.json""#,
        "inventory = 3",
        r#"inventory = "/owned/plugins.json"
inventroy = "/other""#,
    ] {
        let fields: toml::Table = text.parse().unwrap();
        assert!(
            ClaudePluginsLane::parse("lanes.personal", fields).is_err(),
            "{text}"
        );
    }
}
