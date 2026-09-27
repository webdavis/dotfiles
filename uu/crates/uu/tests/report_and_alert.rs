mod support;

use support::*;

fn two_lanes_and(home: Home, report_body: &str) -> Home {
    let good = home.write_stub("good", "cat >/dev/null\nprintf 'all current\\n'\n");
    let bad = home.write_stub("bad", "cat >/dev/null\nprintf 'disk full\\n' >&2\nexit 1\n");
    let report = home.write_stub("report", report_body);
    home.with_config(&format!(
        "[lane.good]\ncommand = [\"{}\"]\n\n[lane.bad]\ncommand = [\"{}\"]\n\n\
         [report]\ncommand = [\"{}\"]\n",
        good.display(),
        bad.display(),
        report.display(),
    ))
}

#[test]
fn a_run_hands_one_report_holding_every_lane_to_the_report_command() {
    let home = two_lanes_and(Home::new("report-command"), "cat >\"$HOME/report.json\"\n");
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let text = std::fs::read_to_string(home.dir.join("report.json")).expect("the report");
    let report: serde_json::Value = serde_json::from_str(&text).expect("one JSON document");
    assert_eq!(report["version"], 1, "{report}");
    assert_eq!(report["dry_run"], false, "{report}");
    let lanes = report["lanes"].as_array().expect("the lanes");
    assert_eq!(lanes.len(), 2, "{report}");
    let lane = |name: &str| {
        lanes
            .iter()
            .find(|lane| lane["name"] == name)
            .unwrap_or_else(|| panic!("no lane {name}: {report}"))
    };
    assert_eq!(lane("good")["outcome"], "ok", "{report}");
    assert_eq!(lane("good")["exit_code"], 0, "{report}");
    assert_eq!(lane("good")["output"], "all current", "{report}");
    assert_eq!(lane("good")["last_ok"], report["started"], "{report}");
    assert_eq!(lane("bad")["outcome"], "failed", "{report}");
    assert_eq!(lane("bad")["exit_code"], 1, "{report}");
    assert!(lane("bad")["last_ok"].is_null(), "{report}");
    assert!(lane("bad")["duration_secs"].is_u64(), "{report}");
}
