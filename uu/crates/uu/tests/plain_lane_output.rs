//! Every child a lane starts runs with `NO_COLOR=1`, so what it prints reaches
//! the record as plain text.

mod support;

use support::*;

#[test]
fn a_command_lane_runs_its_command_with_no_color_set() {
    let home = Home::new("plain-command-lane");
    let stub = home.write_stub("updater", "printf 'NO_COLOR=%s\\n' \"${NO_COLOR-unset}\"\n");
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n",
        stub.display()
    ));
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        stdout(&output).contains("NO_COLOR=1\n"),
        "{}",
        stdout(&output)
    );
}
