//! The records path: one entry per run, posted straight to the hermes
//! gateway.
//!
//! WHY EVERY ENTRY STATES ITS OWN GAP, rather than the channel being a
//! heartbeat you count. `man launchd.plist`, under StartCalendarInterval,
//! verbatim:
//!
//!   "Unlike cron which skips job invocations when the computer is asleep,
//!    launchd will start the job the next time the computer wakes up. If
//!    multiple intervals transpire before the computer is woken, those events
//!    will be coalesced into one event upon wake from sleep."
//!
//! So a live, healthy job can legitimately produce ONE entry covering three
//! weeks, and an absent entry cannot distinguish a dead LaunchAgent from a
//! laptop that was closed for two Sundays. Counting entries measures nothing.
//! The newest entry carries its own gap instead, which reads the same under
//! coalescing, sleep and shutdown.
//!
//! WHY THE MARKER STORES EPOCH PLUS ISO on one line. The epoch is what the gap
//! arithmetic uses, so nothing ever has to parse a timestamp back; the ISO
//! field is for the human reading the entry.
//!
//! NOTHING HERE IS EVER SILENT. A missing marker, an unreadable marker and a
//! clock that moved backwards each produce their own stated sentence, because
//! a quiet fallback reads downstream as a healthy week.

/// What the last-successful-run marker says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Marker {
    /// No marker at all: this machine has never recorded a successful run.
    NeverRecorded,
    /// A marker that is there and says nothing usable.
    Unreadable,
    Recorded {
        epoch: i64,
        iso: String,
    },
}

/// The marker file's one line: `<epoch-seconds> <iso-8601-utc>`.
///
/// DIGITS ONLY, read in base ten. The shell this ports read the same field
/// inside `(( ))`, where a leading zero is octal and a truncated marker such as
/// `0837000000` raises "value too great for base" from a line that runs at
/// start-up. A field that is not a plain count is UNREADABLE rather than zero,
/// because zero renders as a gap of decades.
pub fn parse_marker(text: &str) -> Marker {
    let mut fields = text.split_whitespace();
    let Some(epoch) = fields.next() else {
        return Marker::Unreadable;
    };
    if epoch.is_empty() || !epoch.bytes().all(|byte| byte.is_ascii_digit()) {
        return Marker::Unreadable;
    }
    let Ok(epoch) = epoch.parse::<i64>() else {
        return Marker::Unreadable;
    };
    Marker::Recorded {
        epoch,
        iso: fields.next().unwrap_or_default().to_string(),
    }
}

/// The marker's contents for a run finishing at `epoch` / `iso`.
pub fn marker_contents(epoch: i64, iso: &str) -> String {
    format!("{epoch} {iso}\n")
}

/// A gap a human reads at a glance. Units shift with magnitude.
///
/// A NEGATIVE gap means the recorded timestamp is in the future, i.e. the
/// clock moved backwards (a restored backup, an NTP correction). Rendering
/// that as a small positive number would be a confident lie, so it is named.
pub fn elapsed(seconds: i64) -> String {
    if seconds < 0 {
        return "unknown (the recorded timestamp is in the FUTURE; this clock moved backwards)"
            .to_string();
    }
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m", seconds / 60),
        3600..86_400 => format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60),
        _ => format!("{}d {}h", seconds / 86_400, (seconds % 86_400) / 3600),
    }
}

/// The one line that makes an entry legible on its own. Three marker states,
/// three distinct sentences, and no state that renders as a plausible small
/// gap.
pub fn gap_line(marker: &Marker, marker_path: &str, now_epoch: i64) -> String {
    match marker {
        Marker::NeverRecorded => "last successful run: NEVER RECORDED on this machine".to_string(),
        Marker::Unreadable => {
            format!("last successful run: UNKNOWN (the record at {marker_path} is unreadable)")
        }
        Marker::Recorded { epoch, iso } => {
            // A marker written without its ISO field still has a usable gap,
            // so the epoch stands in rather than leaving the sentence blank.
            let when = if iso.is_empty() {
                epoch.to_string()
            } else {
                iso.clone()
            };
            format!(
                "last successful run: {when} ({} ago)",
                elapsed(now_epoch.saturating_sub(*epoch))
            )
        }
    }
}

/// The record's `state` field: what the whole run amounted to.
///
/// ONE FAILURE ANYWHERE MAKES THE RUN FAILED. The record is read at a glance,
/// and a partial success reported as a success is exactly the reading the
/// record exists to prevent.
pub fn record_state(failures: usize) -> &'static str {
    if failures == 0 { "completed" } else { "failed" }
}

/// The record's `detail`: the header, the gap, every lane's own lines, and the
/// count that closes it.
///
/// A RUN THAT RAN NO LANE SAYS SO. An entry listing nothing reads identically
/// to an entry whose lanes all passed quietly, and those are very different
/// weeks: one of them is a config that turned everything off.
pub fn record_detail(
    host: &str,
    now_iso: &str,
    gap: &str,
    lanes: &[crate::lanes::LaneReport],
) -> String {
    let mut out = format!("run at {now_iso} on {host}\n{gap}\n");
    if lanes.is_empty() {
        out.push_str("no lane is enabled in this config, so nothing was updated\n");
    }
    let mut failures = 0;
    for lane in lanes {
        failures += lane.failures;
        out.push_str(&format!("\n{}: {} failure(s)\n", lane.name, lane.failures));
        for line in &lane.lines {
            out.push_str(&format!("  {line}\n"));
        }
    }
    out.push_str(&format!("\n=== done, {failures} failure(s) ===\n"));
    out
}

/// uu's own gateway body. The four field NAMES are the hermes webhook's
/// contract; what goes in them is uu's.
///
/// BUILT BY THE JSON WRITER, never by interpolation. Every value in here is
/// text a third party wrote (a plugin name, a herdr error), and a quote in one
/// of them would otherwise end the field early.
pub fn record_body(state: &str, host: &str, detail: &str) -> String {
    serde_json::json!({
        "agent": AGENT,
        "state": state,
        "project": host,
        "detail": detail,
    })
    .to_string()
}

/// The agent name every uu record and alert carries.
pub const AGENT: &str = "uu";

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

/// The JSON event handed to a command lane's child on its stdin, newline
/// terminated. A CONTRACT another program parses: never the prose sentence
/// `gap_line` composes for a human reading the doctor output or the record.
/// The field NAMES are the contract and their order is not (serde_json
/// writes them alphabetically), so a child parses the object, never scans it.
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
    use crate::lanes::LaneReport;

    // --- the marker -----------------------------------------------------------

    #[test]
    fn a_marker_is_an_epoch_and_an_iso_on_one_line() {
        assert_eq!(
            parse_marker("1754870400 2026-08-11T00:00:00Z\n"),
            Marker::Recorded {
                epoch: 1_754_870_400,
                iso: "2026-08-11T00:00:00Z".to_string(),
            }
        );
    }

    #[test]
    fn a_marker_that_is_not_an_epoch_is_unreadable_rather_than_zero() {
        // Zero would render as "56 years ago", which is a confident lie about a
        // machine whose bookkeeping was truncated mid-write.
        for text in [
            "",
            "\n",
            "garbage 2026-08-11T00:00:00Z\n",
            "-5 x\n",
            "1e9 x\n",
        ] {
            assert_eq!(parse_marker(text), Marker::Unreadable, "case: {text:?}");
        }
    }

    #[test]
    fn a_marker_with_no_iso_field_still_carries_its_epoch() {
        // Half a marker is still a usable gap: the arithmetic only ever needs
        // the number, and the sentence falls back to printing it.
        assert_eq!(
            parse_marker("1754870400\n"),
            Marker::Recorded {
                epoch: 1_754_870_400,
                iso: String::new(),
            }
        );
    }

    #[test]
    fn a_leading_zero_epoch_is_read_in_base_ten_and_never_as_octal() {
        // The shell this ports read markers inside `(( ))`, where a leading zero
        // is octal and `0837000000` raises "value too great for base" from a
        // line that runs at start-up.
        assert_eq!(
            parse_marker("0837000000 x\n"),
            Marker::Recorded {
                epoch: 837_000_000,
                iso: "x".to_string(),
            }
        );
    }

    #[test]
    fn the_marker_written_is_the_marker_read_back() {
        assert_eq!(
            parse_marker(&marker_contents(1_754_870_400, "2026-08-11T00:00:00Z")),
            Marker::Recorded {
                epoch: 1_754_870_400,
                iso: "2026-08-11T00:00:00Z".to_string(),
            }
        );
    }

    // --- the gap --------------------------------------------------------------

    #[test]
    fn elapsed_shifts_units_with_magnitude() {
        assert_eq!(elapsed(0), "0s");
        assert_eq!(elapsed(59), "59s");
        assert_eq!(elapsed(60), "1m");
        assert_eq!(elapsed(3599), "59m");
        assert_eq!(elapsed(3600), "1h 0m");
        assert_eq!(elapsed(86_399), "23h 59m");
        assert_eq!(elapsed(86_400), "1d 0h");
        assert_eq!(elapsed(694_800), "8d 1h");
    }

    #[test]
    fn a_clock_that_moved_backwards_is_named_and_never_rendered_as_a_small_gap() {
        assert_eq!(
            elapsed(-1),
            "unknown (the recorded timestamp is in the FUTURE; this clock moved backwards)"
        );
    }

    #[test]
    fn a_machine_that_never_finished_a_run_says_so_rather_than_reporting_a_gap() {
        assert_eq!(
            gap_line(&Marker::NeverRecorded, "/state/last-success", 100),
            "last successful run: NEVER RECORDED on this machine"
        );
    }

    #[test]
    fn an_unreadable_marker_names_the_file_the_operator_has_to_look_at() {
        assert_eq!(
            gap_line(&Marker::Unreadable, "/state/last-success", 100),
            "last successful run: UNKNOWN (the record at /state/last-success is unreadable)"
        );
    }

    #[test]
    fn a_recorded_marker_states_when_and_how_long_ago() {
        let marker = Marker::Recorded {
            epoch: 1_000_000,
            iso: "2026-08-11T00:00:00Z".to_string(),
        };
        assert_eq!(
            gap_line(&marker, "/state/last-success", 1_086_400),
            "last successful run: 2026-08-11T00:00:00Z (1d 0h ago)"
        );
    }

    #[test]
    fn a_marker_with_no_iso_prints_its_epoch_rather_than_an_empty_when() {
        let marker = Marker::Recorded {
            epoch: 1_000_000,
            iso: String::new(),
        };
        assert_eq!(
            gap_line(&marker, "/state/last-success", 1_000_060),
            "last successful run: 1000000 (1m ago)"
        );
    }

    // --- the record -----------------------------------------------------------

    #[test]
    fn a_clean_run_is_completed_and_any_failure_makes_the_whole_run_failed() {
        assert_eq!(record_state(0), "completed");
        assert_eq!(record_state(1), "failed");
        assert_eq!(record_state(9), "failed");
    }

    fn report(name: &str, failures: usize, lines: &[&str]) -> LaneReport {
        LaneReport {
            name: name.to_string(),
            failures,
            lines: lines.iter().map(|line| line.to_string()).collect(),
            last_failure: None,
        }
    }

    #[test]
    fn the_detail_opens_with_when_this_run_started_and_the_gap_before_it() {
        let detail = record_detail(
            "dresden",
            "2026-08-11T12:00:00Z",
            "last successful run: NEVER RECORDED on this machine",
            &[],
        );
        let mut lines = detail.lines();
        assert_eq!(lines.next(), Some("run at 2026-08-11T12:00:00Z on dresden"));
        assert_eq!(
            lines.next(),
            Some("last successful run: NEVER RECORDED on this machine")
        );
    }

    #[test]
    fn a_run_with_no_lane_enabled_says_so_instead_of_reading_as_a_quiet_week() {
        let detail = record_detail("dresden", "iso", "gap", &[]);
        assert!(detail.contains("no lane is enabled"), "{detail}");
    }

    #[test]
    fn every_lane_contributes_its_name_its_verdict_and_its_own_lines() {
        let detail = record_detail(
            "dresden",
            "iso",
            "gap",
            &[report(
                "herdr",
                1,
                &["herdr self-update: ok", "plugin a: refreshed"],
            )],
        );
        assert!(detail.contains("herdr: 1 failure(s)"), "{detail}");
        assert!(detail.contains("herdr self-update: ok"), "{detail}");
        assert!(detail.contains("plugin a: refreshed"), "{detail}");
    }

    #[test]
    fn the_detail_closes_with_the_total_across_every_lane() {
        let detail = record_detail(
            "dresden",
            "iso",
            "gap",
            &[report("herdr", 2, &[]), report("other", 3, &[])],
        );
        assert!(
            detail.trim_end().ends_with("=== done, 5 failure(s) ==="),
            "{detail}"
        );
    }

    // --- the body -------------------------------------------------------------

    #[test]
    fn the_body_carries_uus_own_four_fields_and_nothing_else() {
        let body = record_body("completed", "dresden", "the whole record");
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["agent"], "uu");
        assert_eq!(parsed["state"], "completed");
        assert_eq!(parsed["project"], "dresden");
        assert_eq!(parsed["detail"], "the whole record");
        assert_eq!(parsed.as_object().unwrap().len(), 4);
    }

    #[test]
    fn a_detail_holding_json_syntax_is_encoded_rather_than_glued_into_the_body() {
        let body = record_body("failed", "dresden", "plugin \"a\": {broken}\nnext");
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["detail"], "plugin \"a\": {broken}\nnext");
    }

    // --- the command lane's run event ------------------------------------------

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
