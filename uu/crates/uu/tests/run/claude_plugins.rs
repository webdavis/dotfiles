use super::*;
use std::fs;

#[test]
fn a_weekly_plugin_lane_compares_and_advances_history_under_its_declared_name() {
    let home = Home::new("plugin-weekly");
    let inventory = home.dir.join("inventory.json");
    fs::write(
        &inventory,
        r#"{"plugins":{"alpha":[{"scope":"user","version":"2"}]}}"#,
    )
    .unwrap();
    let lane = home.dir.join(".local/state/uu/lanes/personal");
    fs::create_dir_all(&lane).unwrap();
    fs::write(lane.join("snapshot.tsv"), "alpha\t1\n").unwrap();
    let home = home.with_config(&format!(
        "[lanes.personal]\ntype = \"claude-plugins\"\ninventory = {inventory:?}\n"
    ));
    let output = home.uu(&["run", "personal"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(stdout(&output).contains("`alpha` `1` -> `2`"), "{output:?}");
    assert!(
        stdout(&output).contains("personal: completed"),
        "{output:?}"
    );
    assert_eq!(
        fs::read_to_string(lane.join("snapshot.tsv")).unwrap(),
        "alpha\t2\n"
    );
}
