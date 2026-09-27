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

#[test]
fn a_rotate_logs_compressor_runs_with_no_color_set() {
    let home = Home::new("plain-compressor");
    let log = home.dir.join("lane.log");
    std::fs::write(&log, "one line\n").expect("the log");
    let compressor = home.write_stub(
        "compressor",
        "printf 'NO_COLOR=%s\\n' \"${NO_COLOR-unset}\"\n",
    );
    let home = home.with_config(&format!(
        "[lanes.logs]\ntype = \"rotate-logs\"\nlogs = [\"{}\"]\nrotate_at_bytes = 1\n\
         archives_kept = 1\ncompressor = \"{}\"\n",
        log.display(),
        compressor.display()
    ));
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let archive = std::fs::read_to_string(home.dir.join("lane.log.1.gz")).expect("the archive");
    assert_eq!(archive, "NO_COLOR=1\n", "{}", stdout(&output));
}
