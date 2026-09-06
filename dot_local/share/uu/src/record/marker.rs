//! The last-successful-run marker, and the one gap sentence every entry opens
//! with.
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
