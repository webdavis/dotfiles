//! Where the operator is: the readings turned into verdicts, and the policy
//! that arbitrates between them.
//!
//! THE SPLIT INSIDE THIS MODULE MATTERS AS MUCH AS THE ONE BETWEEN THE MODULES.
//! Every decision here is a function of its arguments; the readings themselves
//! sit behind the probe traits, so a test hands this module fixture bytes
//! instead of a second of live network sampling.
//!
//! ONE DEFINITION, EVERY CALLER: the engine deciding which channels fire and a
//! harness gate deciding whether a phone round trip fires at all. A second copy
//! of these bounds is how the two would drift into disagreeing about where the
//! operator is sitting.

use std::collections::BTreeMap;

/// How much a session's inbound byte count must grow between samples before it
/// counts as read rather than pocketed.
pub const DEFAULT_ATTENTION_FLOOR_BYTES: u64 = 100;

/// The unit the idle counter is read in.
const NANOSECONDS_PER_SEC: u64 = 1_000_000_000;

/// The field the session name sits in, and the one its inbound byte count sits
/// in, within a sample row.
const SESSION_NAME_FIELD: usize = 1;
const BYTES_IN_FIELD: usize = 4;

/// What a session row's name starts with. The dot is what separates the process
/// name from its identifier, so a process merely NAMED like one does not match.
const SESSION_NAME_PREFIX: &str = "mosh-server.";

/// Seconds since the last human input, read from a nanosecond counter, or
/// `None` when that cannot be read.
///
/// None is the unknown verdict, and the phone rule reads unknown as away: a
/// garbled probe line must never coerce to 0, which reads as "actively typing"
/// and silently drops the push.
pub fn idle_secs_from_ns(idle_nanoseconds: &str) -> Option<u64> {
    crate::parse_count(idle_nanoseconds).map(|nanoseconds| nanoseconds / NANOSECONDS_PER_SEC)
}

/// True when any remote session's inbound byte count grew by more than
/// `floor_bytes` between the first sample and the last, given a two-sample CSV
/// reading.
///
/// BYTES IN, not bytes out. Traffic the CLIENT sent is what proves the phone's
/// app is foregrounded and being read; output alone is this machine talking
/// into a session nobody has on screen.
///
/// The floor is there because an attached-but-pocketed session still trickles
/// keepalives; a viewed session lands thousands of bytes clear of a pocketed
/// one within a single second, so the separation is not delicate.
///
/// Rows that are not a session line (a repeated header, a truncated sample,
/// anything at all) simply match nothing, so empty and garbage read INACTIVE
/// rather than failing.
pub fn mosh_rate_active(sample_csv: &str, floor_bytes: u64) -> bool {
    let mut readings: BTreeMap<&str, (u64, u64)> = BTreeMap::new();
    for row in sample_csv.lines() {
        let fields: Vec<&str> = row.split(',').collect();
        let Some(session) = fields.get(SESSION_NAME_FIELD) else {
            continue;
        };
        if !session.starts_with(SESSION_NAME_PREFIX) {
            continue;
        }
        let Some(bytes_in) = fields
            .get(BYTES_IN_FIELD)
            .and_then(|field| crate::parse_count(field))
        else {
            continue;
        };
        readings
            .entry(session)
            .and_modify(|(_first, last)| *last = bytes_in)
            .or_insert((bytes_in, bytes_in));
    }
    readings
        .values()
        .any(|(first, last)| last.saturating_sub(*first) > floor_bytes)
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_ATTENTION_FLOOR_BYTES, idle_secs_from_ns, mosh_rate_active};

    // --- idle_secs_from_ns -------------------------------------------------

    #[test]
    fn a_nanosecond_counter_becomes_whole_seconds() {
        assert_eq!(idle_secs_from_ns("5000000000"), Some(5));
    }

    #[test]
    fn a_partial_second_truncates_rather_than_rounding_up() {
        assert_eq!(idle_secs_from_ns("1999999999"), Some(1));
        assert_eq!(idle_secs_from_ns("0"), Some(0));
    }

    #[test]
    fn an_empty_reading_is_unknown_rather_than_zero_seconds_idle() {
        // Zero would read as "actively typing" and silently drop the push.
        assert_eq!(idle_secs_from_ns(""), None);
    }

    #[test]
    fn a_garbled_reading_is_unknown() {
        assert_eq!(idle_secs_from_ns("HIDIdleTime"), None);
        assert_eq!(idle_secs_from_ns("5000000000 "), None);
    }

    // --- mosh_rate_active --------------------------------------------------

    const HEADER: &str = "time,,interface,state,bytes_in,bytes_out";

    fn sample(rows: &[&str]) -> String {
        format!("{}\n", rows.join("\n"))
    }

    #[test]
    fn a_session_whose_inbound_bytes_moved_between_samples_is_active() {
        let csv = sample(&[
            HEADER,
            "01:00:00,mosh-server.111,,,1000,5000",
            "01:00:00,mosh-server.222,,,300,900",
            HEADER,
            "01:00:01,mosh-server.111,,,1600,7800",
            "01:00:01,mosh-server.222,,,300,900",
        ]);
        assert!(mosh_rate_active(&csv, DEFAULT_ATTENTION_FLOOR_BYTES));
    }

    #[test]
    fn sessions_all_flat_between_samples_are_inactive() {
        let csv = sample(&[
            "01:00:00,mosh-server.111,,,1000,5000",
            "01:00:01,mosh-server.111,,,1000,5000",
        ]);
        assert!(!mosh_rate_active(&csv, DEFAULT_ATTENTION_FLOOR_BYTES));
    }

    #[test]
    fn an_inbound_delta_below_the_floor_is_inactive_rather_than_a_phone_in_hand() {
        let csv = sample(&[
            "01:00:00,mosh-server.111,,,1000,5000",
            "01:00:01,mosh-server.111,,,1050,5000",
        ]);
        assert!(!mosh_rate_active(&csv, DEFAULT_ATTENTION_FLOOR_BYTES));
    }

    #[test]
    fn an_inbound_delta_exactly_at_the_floor_is_inactive_because_the_floor_must_be_beaten() {
        let csv = sample(&[
            "01:00:00,mosh-server.111,,,1000,5000",
            "01:00:01,mosh-server.111,,,1100,5000",
        ]);
        assert!(!mosh_rate_active(&csv, DEFAULT_ATTENTION_FLOOR_BYTES));
    }

    #[test]
    fn one_byte_past_the_floor_is_active() {
        let csv = sample(&[
            "01:00:00,mosh-server.111,,,1000,5000",
            "01:00:01,mosh-server.111,,,1101,5000",
        ]);
        assert!(mosh_rate_active(&csv, DEFAULT_ATTENTION_FLOOR_BYTES));
    }

    #[test]
    fn outbound_growth_alone_is_this_machine_talking_to_nobody() {
        let csv = sample(&[
            "01:00:00,mosh-server.111,,,1000,5000",
            "01:00:01,mosh-server.111,,,1000,99000",
        ]);
        assert!(!mosh_rate_active(&csv, DEFAULT_ATTENTION_FLOOR_BYTES));
    }

    #[test]
    fn any_one_active_session_is_enough_even_beside_flat_ones() {
        let csv = sample(&[
            "01:00:00,mosh-server.111,,,1000,5000",
            "01:00:00,mosh-server.222,,,300,900",
            "01:00:01,mosh-server.111,,,1000,5000",
            "01:00:01,mosh-server.222,,,9300,900",
        ]);
        assert!(mosh_rate_active(&csv, DEFAULT_ATTENTION_FLOOR_BYTES));
    }

    #[test]
    fn a_process_that_is_not_a_session_is_ignored_however_much_it_moved() {
        // The last pair CONTAINS the prefix without starting with it, which is
        // where an unanchored match would let any busy process vouch for a
        // phone nobody is holding.
        let csv = sample(&[
            "01:00:00,mosh-serverX,,,1000,5000",
            "01:00:01,mosh-serverX,,,90000,5000",
            "01:00:00,ssh.42,,,1000,5000",
            "01:00:01,ssh.42,,,90000,5000",
            "01:00:00,not-a-mosh-server.42,,,1000,5000",
            "01:00:01,not-a-mosh-server.42,,,90000,5000",
        ]);
        assert!(!mosh_rate_active(&csv, DEFAULT_ATTENTION_FLOOR_BYTES));
    }

    #[test]
    fn a_counter_that_went_backwards_is_inactive_rather_than_wrapping_around() {
        let csv = sample(&[
            "01:00:00,mosh-server.111,,,9000,5000",
            "01:00:01,mosh-server.111,,,1000,5000",
        ]);
        assert!(!mosh_rate_active(&csv, DEFAULT_ATTENTION_FLOOR_BYTES));
    }

    #[test]
    fn the_delta_is_first_to_last_rather_than_the_highest_sample_seen() {
        // A counter that climbed and then reset reads as flat, because the
        // reading is the two ends. Keeping the peak instead would call a
        // session that restarted mid-sample a phone in hand.
        let csv = sample(&[
            "01:00:00,mosh-server.111,,,1000,5000",
            "01:00:01,mosh-server.111,,,9000,5000",
            "01:00:02,mosh-server.111,,,1000,5000",
        ]);
        assert!(!mosh_rate_active(&csv, DEFAULT_ATTENTION_FLOOR_BYTES));
    }

    #[test]
    fn empty_or_garbage_input_is_inactive_and_never_a_failure() {
        assert!(!mosh_rate_active("", DEFAULT_ATTENTION_FLOOR_BYTES));
        assert!(!mosh_rate_active(
            "no such thing\n",
            DEFAULT_ATTENTION_FLOOR_BYTES
        ));
        assert!(!mosh_rate_active(HEADER, DEFAULT_ATTENTION_FLOOR_BYTES));
    }

    #[test]
    fn a_truncated_row_is_skipped_rather_than_read_as_a_zero_reading() {
        // The last row of a sample can arrive cut short, and reading its
        // missing byte count as zero would invent a huge negative delta.
        let csv = sample(&[
            "01:00:00,mosh-server.111,,,1000,5000",
            "01:00:01,mosh-server.111,,,9000,7000",
            "01:00:01,mosh-server.111,,",
        ]);
        assert!(mosh_rate_active(&csv, DEFAULT_ATTENTION_FLOOR_BYTES));
    }

    #[test]
    fn a_non_numeric_byte_count_is_skipped() {
        let csv = sample(&[
            "01:00:00,mosh-server.111,,,bytes_in,bytes_out",
            "01:00:01,mosh-server.111,,,9000,7000",
        ]);
        assert!(!mosh_rate_active(&csv, DEFAULT_ATTENTION_FLOOR_BYTES));
    }

    #[test]
    fn the_floor_is_the_callers_to_set() {
        let csv = sample(&[
            "01:00:00,mosh-server.111,,,1000,5000",
            "01:00:01,mosh-server.111,,,1050,5000",
        ]);
        assert!(mosh_rate_active(&csv, 10));
        assert!(!mosh_rate_active(&csv, 10_000));
    }
}
