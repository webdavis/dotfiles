use super::*;
use crate::config::NvimParsersLane;

fn own_lane() -> NvimParsersLane {
    NvimParsersLane {
        host: lane(false).host,
    }
}

#[test]
fn the_parsers_lane_runs_the_parsers_module_headless_under_the_lanes_own_name() {
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
            "/fixture/config with spaces/lua/uu/parsers.lua"
        ]]
    );
}

#[test]
fn a_parsers_child_exiting_non_zero_is_a_counted_failure_carrying_the_compiler_tail() {
    let child = Child::new(Verdict::Failed("exit 1: owned compiler tail".into()));
    let report = own_lane().run("chosen-editor", &stub_facts(), &child);
    assert_eq!(report.verdict(), LaneVerdict::Failed);
    assert_eq!(report.failures(), 1);
    assert!(report.lines[1].ends_with("owned compiler tail"));
}
