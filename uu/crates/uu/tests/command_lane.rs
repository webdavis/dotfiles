//! The command lane, end to end: the run event on stdin and the exit code as the verdict.

mod support;

use support::*;

// --- the command lane -------------------------------------------------------

#[test]
fn a_command_lane_runs_end_to_end_and_the_record_carries_what_it_printed() {
    let home = Home::new("command-lane");
    let stub = home.write_stub("updater", "cat >\"$HOME/event\"; printf '3 upgraded\\n'\n");
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n",
        stub.display()
    ));
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let event = std::fs::read_to_string(home.dir.join("event")).expect("the event file");
    assert!(event.contains("\"lane\":\"mine\""), "{event}");
    assert!(
        stdout(&output).contains("3 upgraded"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn a_failed_command_lane_alerts_through_the_configured_engine() {
    let home = Home::new("command-lane-failed");
    // Both stdout and stderr, and a non-1 exit: a fixture that says nothing
    // on either stream passes even when the exit code or the stderr tail
    // never reach the record or the alert.
    let stub = home.write_stub(
        "updater",
        "cat >/dev/null\nprintf 'did some upgrading\\n'\nprintf 'boom: disk full\\n' >&2\nexit 2\n",
    );
    let pns_stub = home.write_stub("pns-stub", "printf '%s\\n' \"$*\" >\"$HOME/alert-args\"\n");
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n\n[alerts]\nbinary = \"{}\"\n",
        stub.display(),
        pns_stub.display(),
    ));
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let record = stdout(&output);
    assert!(record.contains("did some upgrading"), "{record}");
    assert!(record.contains("exit 2"), "{record}");
    assert!(record.contains("boom: disk full"), "{record}");
    let args = std::fs::read_to_string(home.dir.join("alert-args")).expect("the alert args");
    assert!(args.contains("--state failed"), "{args}");
    // The lane's own NAME heads the detail, not its type and not a path that
    // happens to hold the word, and the exit code and stderr tail ride along
    // with it rather than a bare failure count.
    assert!(args.contains("--detail mine: 1 failure(s);"), "{args}");
    assert!(args.contains("exit 2"), "{args}");
    assert!(args.contains("boom: disk full"), "{args}");
}
