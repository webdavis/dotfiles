use super::*;
use std::{fs, path::PathBuf};
struct Fixture {
    root: PathBuf,
    lane: ClaudePluginsLane,
}
impl Fixture {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!("uu-plugins-{tag}-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let lane = ClaudePluginsLane {
            inventory: root.join("inventory.json").to_str().unwrap().into(),
        };
        let fixture = Self { root, lane };
        fixture.inventory("2");
        fixture
    }
    fn inventory(&self, version: &str) {
        fs::write(
            &self.lane.inventory,
            serde_json::json!({"plugins":{"alpha":[{"scope":"user","version":version}]}})
                .to_string(),
        )
        .unwrap();
    }
    fn snapshot(&self) -> PathBuf {
        self.root
            .join(".local/state/uu/lanes/claude-plugins/snapshot.tsv")
    }
    fn legacy(&self) -> PathBuf {
        self.root
            .join(".local/state/report-plugin-updates/installed-plugins.snapshot")
    }
    fn set(&self, path: PathBuf, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn run(&self, bootstrap: bool) -> LaneReport {
        execute(
            &self.lane,
            "claude-plugins",
            self.root.to_str().unwrap(),
            bootstrap,
        )
    }
    fn current(&self) -> String {
        fs::read_to_string(self.snapshot()).unwrap()
    }
}
fn completed(report: &LaneReport) {
    assert_eq!(
        report.verdict(),
        uu_domain::LaneVerdict::Completed,
        "{report:?}"
    );
}
#[test]
fn the_first_reading_is_a_baseline_that_compares_nothing() {
    let f = Fixture::new("first");
    let report = f.run(false);
    completed(&report);
    assert!(report.lines.join("\n").contains("baseline"), "{report:?}");
    assert!(!report.lines.join("\n").contains("changed"));
    assert_eq!(f.current(), "alpha\t2\n");
}
#[test]
fn a_second_reading_is_compared_and_the_snapshot_moves() {
    let f = Fixture::new("second");
    f.set(f.snapshot(), "alpha\t1\n");
    let report = f.run(false);
    completed(&report);
    let line = report.lines.join("\n");
    assert!(line.contains("`alpha` `1` -> `2`"), "{line}");
    assert!(line.contains("USER-scope"), "{line}");
    assert_eq!(f.current(), "alpha\t2\n");
}
#[test]
fn an_unreadable_inventory_fails_the_lane_and_leaves_the_snapshot_alone() {
    let mut f = Fixture::new("unreadable");
    f.set(f.snapshot(), "alpha\t1\n");
    f.lane.inventory = f.root.join("missing").to_str().unwrap().into();
    let report = f.run(false);
    assert_eq!(report.failures(), 1, "{report:?}");
    assert!(report.lines.join("\n").contains("NOT COMPARED"));
    assert_eq!(f.current(), "alpha\t1\n");
}
#[test]
fn bootstrap_seeds_a_baseline_once_and_leaves_an_existing_one_alone() {
    let f = Fixture::new("seed");
    completed(&f.run(true));
    assert_eq!(f.current(), "alpha\t2\n");
    f.inventory("3");
    completed(&f.run(true));
    assert_eq!(f.current(), "alpha\t2\n");
}
#[test]
fn an_imported_legacy_snapshot_reports_changes_since_the_last_delivered_bash_record() {
    let f = Fixture::new("legacy");
    f.set(f.legacy(), "alpha\t1\n");
    let report = f.run(false);
    completed(&report);
    assert!(
        report.lines.join("\n").contains("`alpha` `1` -> `2`"),
        "{report:?}"
    );
    assert_eq!(f.current(), "alpha\t2\n");
    assert_eq!(fs::read_to_string(f.legacy()).unwrap(), "alpha\t1\n");
}
#[test]
fn an_existing_uu_snapshot_wins_over_a_legacy_snapshot() {
    let f = Fixture::new("precedence");
    f.set(f.snapshot(), "alpha\t1.5\n");
    f.set(f.legacy(), "malformed");
    let report = f.run(false);
    completed(&report);
    assert!(
        report.lines.join("\n").contains("`1.5` -> `2`"),
        "{report:?}"
    );
    assert_eq!(fs::read_to_string(f.legacy()).unwrap(), "malformed");
}
#[test]
fn an_empty_legacy_snapshot_is_imported_and_new_user_plugins_are_reported_added() {
    let f = Fixture::new("empty");
    f.set(f.legacy(), "");
    completed(&f.run(true));
    assert_eq!(f.current(), "");
    let report = f.run(false);
    completed(&report);
    assert!(
        report.lines.join("\n").contains("`alpha` (added)"),
        "{report:?}"
    );
}
#[test]
fn a_failed_legacy_import_preserves_both_states_and_never_seeds_fresh() {
    for (i, text) in ["not-tab-separated", "z\t1\na\t2\n", "a\t1\textra\n"]
        .iter()
        .enumerate()
    {
        let f = Fixture::new(&format!("invalid-{i}"));
        f.set(f.legacy(), text);
        let report = f.run(true);
        assert_eq!(report.failures(), 1, "{report:?}");
        assert!(!f.snapshot().exists());
        assert_eq!(fs::read_to_string(f.legacy()).unwrap(), *text);
    }
    let f = Fixture::new("import-write");
    f.set(f.legacy(), "alpha\t1\n");
    f.set(f.root.join(".local/state/uu/lanes"), "blocked");
    assert_eq!(f.run(true).failures(), 1);
    assert!(!f.snapshot().exists());
    assert_eq!(fs::read_to_string(f.legacy()).unwrap(), "alpha\t1\n");
}
#[test]
fn repeated_bootstrap_never_consumes_the_imported_comparison() {
    let f = Fixture::new("repeat");
    f.set(f.legacy(), "alpha\t1\n");
    completed(&f.run(true));
    assert_eq!(f.current(), "alpha\t1\n");
    f.inventory("3");
    let report = f.run(true);
    completed(&report);
    assert!(!report.lines.join("\n").contains("changed"));
    assert_eq!(f.current(), "alpha\t1\n");
    assert!(f.run(false).lines.join("\n").contains("`1` -> `3`"));
}

mod extra;
