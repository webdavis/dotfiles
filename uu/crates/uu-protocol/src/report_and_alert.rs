#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaneOutcome {
    Ok,
    Deferred,
    Pending,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlertKind {
    Failed,
    Stale,
    Pending,
    ReportUndelivered,
}

pub struct LaneEntry<'a> {
    pub name: &'a str,
    pub outcome: LaneOutcome,
    pub exit_code: Option<i32>,
    pub duration_secs: u64,
    pub output: String,
    pub last_ok: Option<&'a str>,
}

impl LaneOutcome {
    fn word(self) -> &'static str {
        match self {
            LaneOutcome::Ok => "ok",
            LaneOutcome::Deferred => "deferred",
            LaneOutcome::Pending => "pending",
            LaneOutcome::Failed => "failed",
        }
    }
}

impl AlertKind {
    fn word(self) -> &'static str {
        match self {
            AlertKind::Failed => "failed",
            AlertKind::Stale => "stale",
            AlertKind::Pending => "pending",
            AlertKind::ReportUndelivered => "report_undelivered",
        }
    }
}

pub fn report_document(host: &str, started: &str, ended: &str, lanes: &[LaneEntry<'_>]) -> String {
    let lanes: Vec<serde_json::Value> = lanes
        .iter()
        .map(|lane| {
            serde_json::json!({
                "name": lane.name,
                "outcome": lane.outcome.word(),
                "exit_code": lane.exit_code,
                "duration_secs": lane.duration_secs,
                "output": lane.output,
                "last_ok": lane.last_ok,
            })
        })
        .collect();
    let document = serde_json::json!({
        "version": 1,
        "host": host,
        "started": started,
        "ended": ended,
        "dry_run": false,
        "lanes": lanes,
    });
    format!("{document}\n")
}

pub fn alert_document(kind: AlertKind, lane: Option<&str>, host: &str, message: &str) -> String {
    let document = serde_json::json!({
        "version": 1,
        "kind": kind.word(),
        "lane": lane,
        "host": host,
        "message": message,
    });
    format!("{document}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_document_carries_the_run_and_every_lane_it_ran() {
        let lanes = [
            LaneEntry {
                name: "first",
                outcome: LaneOutcome::Ok,
                exit_code: Some(0),
                duration_secs: 12,
                output: "one\ntwo".into(),
                last_ok: Some("2026-09-26T12:00:00Z"),
            },
            LaneEntry {
                name: "second",
                outcome: LaneOutcome::Failed,
                exit_code: None,
                duration_secs: 0,
                output: "".into(),
                last_ok: None,
            },
            LaneEntry {
                name: "third",
                outcome: LaneOutcome::Deferred,
                exit_code: Some(75),
                duration_secs: 1,
                output: "".into(),
                last_ok: None,
            },
            LaneEntry {
                name: "fourth",
                outcome: LaneOutcome::Pending,
                exit_code: Some(100),
                duration_secs: 1,
                output: "".into(),
                last_ok: None,
            },
        ];
        let document = report_document(
            "fixture-host",
            "2026-09-26T12:00:00Z",
            "2026-09-26T12:05:00Z",
            &lanes,
        );
        let parsed: serde_json::Value = serde_json::from_str(&document).unwrap();
        assert_eq!(
            parsed,
            serde_json::json!({
                "version": 1,
                "host": "fixture-host",
                "started": "2026-09-26T12:00:00Z",
                "ended": "2026-09-26T12:05:00Z",
                "dry_run": false,
                "lanes": [
                    {
                        "name": "first",
                        "outcome": "ok",
                        "exit_code": 0,
                        "duration_secs": 12,
                        "output": "one\ntwo",
                        "last_ok": "2026-09-26T12:00:00Z",
                    },
                    {
                        "name": "second",
                        "outcome": "failed",
                        "exit_code": null,
                        "duration_secs": 0,
                        "output": "",
                        "last_ok": null,
                    },
                    {
                        "name": "third",
                        "outcome": "deferred",
                        "exit_code": 75,
                        "duration_secs": 1,
                        "output": "",
                        "last_ok": null,
                    },
                    {
                        "name": "fourth",
                        "outcome": "pending",
                        "exit_code": 100,
                        "duration_secs": 1,
                        "output": "",
                        "last_ok": null,
                    },
                ],
            })
        );
    }

    #[test]
    fn the_alert_document_names_its_kind_its_lane_or_none_the_host_and_the_message() {
        for (kind, lane, word) in [
            (AlertKind::Failed, Some("first"), "failed"),
            (AlertKind::Stale, Some("first"), "stale"),
            (AlertKind::Pending, Some("first"), "pending"),
            (AlertKind::ReportUndelivered, None, "report_undelivered"),
        ] {
            let document = alert_document(kind, lane, "fixture-host", "what \"went\" wrong");
            let parsed: serde_json::Value = serde_json::from_str(&document).unwrap();
            assert_eq!(
                parsed,
                serde_json::json!({
                    "version": 1,
                    "kind": word,
                    "lane": lane,
                    "host": "fixture-host",
                    "message": "what \"went\" wrong",
                })
            );
        }
    }
}
