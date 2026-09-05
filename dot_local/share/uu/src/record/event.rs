//! The JSON run event a command lane's child is handed on its stdin.
//!
//! A CONTRACT ANOTHER PROGRAM PARSES, never the prose sentence `gap_line`
//! composes for a human reading the doctor output or the record. The field
//! NAMES are the contract and their order is not (serde_json writes them
//! alphabetically), so a child parses the object rather than scanning it.

use super::AGENT;
use super::marker::Marker;

/// What a command lane's own run event needs: `main` computes `started` and
/// drops the `Marker` inside `gap_line` before the lane loop runs, so this
/// struct carries those facts into `run_lane` rather than each lane
/// recomputing them.
pub struct RunFacts<'a> {
    pub host: &'a str,
    pub started_epoch: i64,
    pub started_iso: &'a str,
    pub marker: &'a Marker,
}

/// The `last_successful_run` field: the same three states `gap_line` renders
/// as a sentence, but as DATA. NEVER a zero epoch standing in for "none
/// recorded yet": the two "no usable timestamp" states carry no `epoch` key
/// at all, so a reader cannot mistake either for a real, very old run.
fn last_successful_run(marker: &Marker) -> serde_json::Value {
    match marker {
        Marker::NeverRecorded => serde_json::json!({ "state": "never-recorded" }),
        Marker::Unreadable => serde_json::json!({ "state": "unreadable" }),
        Marker::Recorded { epoch, iso } => {
            serde_json::json!({ "state": "recorded", "epoch": epoch, "iso": iso })
        }
    }
}

/// The event itself, newline terminated.
pub fn lane_event(lane: &str, facts: &RunFacts) -> String {
    let event = serde_json::json!({
        "agent": AGENT,
        "lane": lane,
        "host": facts.host,
        "started": { "epoch": facts.started_epoch, "iso": facts.started_iso },
        "last_successful_run": last_successful_run(facts.marker),
    });
    format!("{event}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lane_event_names_the_lane_the_host_the_start_and_the_last_successful_run() {
        let marker = Marker::Recorded {
            epoch: 1_787_659_200,
            iso: "2026-08-24T12:00:00Z".to_string(),
        };
        let facts = RunFacts {
            host: "dresden",
            started_epoch: 1_788_264_000,
            started_iso: "2026-08-31T12:00:00Z",
            marker: &marker,
        };
        let event = lane_event("example", &facts);
        assert!(event.ends_with('\n'), "{event:?}");
        let parsed: serde_json::Value = serde_json::from_str(event.trim_end()).unwrap();
        assert_eq!(parsed["agent"], "uu");
        assert_eq!(parsed["lane"], "example");
        assert_eq!(parsed["host"], "dresden");
        assert_eq!(parsed["started"]["epoch"], 1_788_264_000);
        assert_eq!(parsed["started"]["iso"], "2026-08-31T12:00:00Z");
        assert_eq!(parsed["last_successful_run"]["state"], "recorded");
        assert_eq!(parsed["last_successful_run"]["epoch"], 1_787_659_200);
        assert_eq!(parsed["last_successful_run"]["iso"], "2026-08-24T12:00:00Z");
    }

    #[test]
    fn a_never_recorded_marker_is_a_state_in_the_event_and_never_a_zero_epoch() {
        for (marker, state) in [
            (Marker::NeverRecorded, "never-recorded"),
            (Marker::Unreadable, "unreadable"),
        ] {
            let facts = RunFacts {
                host: "dresden",
                started_epoch: 1_788_264_000,
                started_iso: "2026-08-31T12:00:00Z",
                marker: &marker,
            };
            let event = lane_event("example", &facts);
            let parsed: serde_json::Value = serde_json::from_str(event.trim_end()).unwrap();
            assert_eq!(parsed["last_successful_run"]["state"], state, "{event}");
            assert!(
                parsed["last_successful_run"].get("epoch").is_none(),
                "a {state} marker must never carry an epoch key: {event}"
            );
        }
    }
}
