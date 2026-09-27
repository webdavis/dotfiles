mod support;

use support::*;

const APPENDS_ITS_INPUT: &str = "cat >>\"$HOME/alerts\"\n";

fn two_lanes_and(home: Home, report_body: &str, alert_body: &str) -> Home {
    let good = home.write_stub("good", "cat >/dev/null\nprintf 'all current\\n'\n");
    let bad = home.write_stub("bad", "cat >/dev/null\nprintf 'disk full\\n' >&2\nexit 1\n");
    let report = home.write_stub("report", report_body);
    let alert = home.write_stub("alert", alert_body);
    home.with_config(&format!(
        "[lane.good]\ncommand = [\"{}\"]\n\n[lane.bad]\ncommand = [\"{}\"]\n\n\
         [report]\ncommand = [\"{}\"]\n\n[alert]\ncommand = [\"{}\"]\n",
        good.display(),
        bad.display(),
        report.display(),
        alert.display(),
    ))
}

fn alerts(home: &Home) -> Vec<serde_json::Value> {
    std::fs::read_to_string(home.dir.join("alerts"))
        .expect("the alerts")
        .lines()
        .map(|line| serde_json::from_str(line).expect("one JSON document per alert"))
        .collect()
}

#[test]
fn a_run_hands_one_report_holding_every_lane_to_the_report_command() {
    let home = two_lanes_and(
        Home::new("report-command"),
        "cat >\"$HOME/report.json\"\n",
        APPENDS_ITS_INPUT,
    );
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

    let alerts = alerts(&home);
    assert_eq!(alerts.len(), 1, "{alerts:?}");
    assert_eq!(alerts[0]["version"], 1, "{alerts:?}");
    assert_eq!(alerts[0]["kind"], "failed", "{alerts:?}");
    assert_eq!(alerts[0]["lane"], "bad", "{alerts:?}");
    assert_eq!(alerts[0]["host"], report["host"], "{alerts:?}");
    assert!(
        alerts[0]["message"].as_str().unwrap().contains("disk full"),
        "{alerts:?}"
    );
}

#[test]
fn a_report_command_that_exits_non_zero_is_tried_once_and_raises_report_undelivered() {
    let home = two_lanes_and(
        Home::new("report-undelivered"),
        "cat >/dev/null\nprintf 'attempt\\n' >>\"$HOME/attempts\"\nexit 1\n",
        APPENDS_ITS_INPUT,
    );
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let attempts = std::fs::read_to_string(home.dir.join("attempts")).expect("the attempts");
    assert_eq!(attempts, "attempt\n");
    let alerts = alerts(&home);
    let undelivered: Vec<_> = alerts
        .iter()
        .filter(|alert| alert["kind"] == "report_undelivered")
        .collect();
    assert_eq!(undelivered.len(), 1, "{alerts:?}");
    assert!(undelivered[0]["lane"].is_null(), "{alerts:?}");
    assert!(
        undelivered[0]["message"]
            .as_str()
            .unwrap()
            .contains("the run report was NOT delivered: exit 1"),
        "{alerts:?}"
    );
}

#[test]
fn an_alert_command_that_fails_is_logged_in_the_run_log_and_nothing_else_happens() {
    let home = two_lanes_and(
        Home::new("alert-command-fails"),
        "cat >/dev/null\n",
        "cat >/dev/null\nprintf 'no route\\n' >&2\nexit 1\n",
    );
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(!stdout(&output).contains("did NOT reach"), "{output:?}");
    let log = std::fs::read_to_string(home.dir.join(".local/log/uu/uu.log")).expect("the log");
    assert!(
        log.contains("uu: the [alert] command for `bad` failed: exit 1: no route"),
        "{log}"
    );
}
