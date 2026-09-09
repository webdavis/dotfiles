use super::*;
use crate::config::NvimMasonLane;

fn own_lane() -> NvimMasonLane {
    NvimMasonLane {
        host: lane(false).host,
    }
}

#[test]
fn the_mason_lane_runs_the_mason_module_headless_under_the_lanes_own_name() {
    let child = Child::new(Verdict::Clean);
    let report = own_lane().run("chosen-editor", &stub_facts(), &child);
    assert_eq!(report.name, "chosen-editor");
    assert_eq!(report.verdict(), LaneVerdict::Completed);
    assert_eq!(
        *child.calls.borrow(),
        vec![vec![
            "/fixture/nvim",
            "--headless",
            "-u",
            "/fixture/config with spaces/init.lua",
            "-l",
            "/fixture/config with spaces/lua/uu/mason.lua"
        ]]
    );
}

#[test]
fn a_mason_child_exiting_non_zero_is_a_counted_failure() {
    let child = Child::new(Verdict::Failed("exit 1: owned compiler tail".into()));
    let report = own_lane().run("chosen-editor", &stub_facts(), &child);
    assert_eq!(report.verdict(), LaneVerdict::Failed);
    assert_eq!(report.failures(), 1);
    assert!(report.lines[1].ends_with("owned compiler tail"));
}
