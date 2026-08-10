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

/// How recently this Mac must have been touched for its own input to settle the
/// question outright.
pub const DEFAULT_PHYSICAL_FRESH_SECS: u64 = 20;

/// How much a session's inbound byte count must grow between samples before it
/// counts as read rather than pocketed.
pub const DEFAULT_ATTENTION_FLOOR_BYTES: u64 = 100;

/// How long a deliberate "I am on my phone" signal stands. A tap means "the
/// next few minutes" and is refreshed by tapping again; a longer window would
/// resurrect the mid-reading buzzing the idle threshold exists to stop.
pub const DEFAULT_PHONE_MARKER_TTL_SECS: u64 = 300;

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

/// True when the idle reading sits in the ONE band where a phone signal may
/// overrule the Mac.
///
///   idle < fresh          the operator just touched this Mac, so hands are
///                         here. A phone streaming a session only proves the
///                         app is on a screen somewhere; a keypress proves
///                         where the person is.
///   fresh <= idle < desk  the band this returns true for. The Mac reads "at
///                         the desk" while the operator may be standing in the
///                         hallway watching through their phone, so a phone
///                         signal decides.
///   idle >= desk          away, and the ordinary rule already sends the push,
///                         so there is nothing to overrule.
///
/// This is what confines the probes to the one band where they can change an
/// answer, which keeps their cost off every other notification. An unreadable
/// reading is in no band: unknown presence already fails open into a push, so
/// there is nothing left to overrule.
pub fn attention_band(
    idle_secs: Option<u64>,
    desk_idle_secs: Option<u64>,
    physical_fresh_secs: Option<u64>,
) -> bool {
    let (Some(idle_secs), Some(desk_idle_secs), Some(physical_fresh_secs)) =
        (idle_secs, desk_idle_secs, physical_fresh_secs)
    else {
        return false;
    };
    idle_secs >= physical_fresh_secs && idle_secs < desk_idle_secs
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

/// True when the phone-attention marker was touched within `ttl_secs`.
///
/// THE DELIBERATE SIGNAL, and the one case no probe can see: the session in the
/// background on a phone the operator is holding. A tap runs one forced touch
/// of the marker, which is the operator saying "I am on my phone" in as many
/// words.
///
/// This one fails CLOSED, unlike the idle probe: an absent marker and an
/// unreadable one both mean the operator said nothing, and inventing a signal
/// out of a failed read would push to a phone in a pocket.
pub fn marker_fresh(
    marker_mtime_secs: Option<u64>,
    now_secs: Option<u64>,
    ttl_secs: Option<u64>,
) -> bool {
    let (Some(marker_mtime_secs), Some(now_secs), Some(ttl_secs)) =
        (marker_mtime_secs, now_secs, ttl_secs)
    else {
        return false;
    };
    // Signed, so a marker dated in the future stays fresh instead of wrapping
    // into an age longer than the window.
    i128::from(now_secs) - i128::from(marker_mtime_secs) < i128::from(ttl_secs)
}

/// True when the phone is ACTIVELY VIEWING a session right now.
///
/// Separate from attention because the two answer different questions:
/// attention ("phone in hand", marker OR rate) routes cards TO the phone,
/// viewing ("this session on screen", rate ONLY) is what may suppress the one
/// card about the pane being watched. THE MARKER DELIBERATELY DOES NOT REACH
/// THIS FUNCTION, which is why it has no parameter for one.
pub fn moshi_viewing(forced: Option<bool>, rate_active: bool) -> bool {
    forced.unwrap_or(rate_active)
}

/// True when the operator is demonstrably on their phone.
///
/// Three sources, and every one of them optional: a forced verdict, the tap
/// marker, then whether a session is being viewed. ANY MISSING PREREQUISITE IS
/// "NO SIGNAL", not an error, and leaves the plain idle rule standing.
///
/// The parameters are verdicts rather than probes, so a caller keeps the
/// cheapest-first order: the viewing sample takes a full second of live
/// counters and is worth evaluating only once the marker has said nothing.
pub fn phone_attention(forced: Option<bool>, marker_is_fresh: bool, viewing: bool) -> bool {
    forced.unwrap_or(marker_is_fresh || viewing)
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_ATTENTION_FLOOR_BYTES, DEFAULT_PHONE_MARKER_TTL_SECS, DEFAULT_PHYSICAL_FRESH_SECS,
        attention_band, idle_secs_from_ns, marker_fresh, mosh_rate_active, moshi_viewing,
        phone_attention,
    };

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

    // --- attention_band ----------------------------------------------------

    #[test]
    fn an_idle_reading_between_fresh_and_desk_is_the_band_a_phone_may_overrule() {
        assert!(attention_band(Some(60), Some(120), Some(20)));
    }

    #[test]
    fn an_idle_reading_exactly_at_fresh_is_already_in_the_band() {
        assert!(attention_band(Some(20), Some(120), Some(20)));
    }

    #[test]
    fn a_just_touched_mac_is_below_the_band_so_no_phone_signal_can_overrule_it() {
        assert!(!attention_band(Some(19), Some(120), Some(20)));
        assert!(!attention_band(Some(0), Some(120), Some(20)));
    }

    #[test]
    fn an_idle_reading_exactly_at_the_desk_threshold_is_past_the_band() {
        // At the threshold the ordinary rule already sends the push, so there
        // is nothing left to overrule.
        assert!(!attention_band(Some(120), Some(120), Some(20)));
        assert!(!attention_band(Some(900), Some(120), Some(20)));
    }

    #[test]
    fn an_unreadable_reading_is_in_no_band() {
        assert!(!attention_band(None, Some(120), Some(20)));
        assert!(!attention_band(Some(60), None, Some(20)));
        assert!(!attention_band(Some(60), Some(120), None));
    }

    #[test]
    fn the_default_freshness_window_is_twenty_seconds() {
        assert_eq!(DEFAULT_PHYSICAL_FRESH_SECS, 20);
        assert!(!attention_band(
            Some(19),
            Some(120),
            Some(DEFAULT_PHYSICAL_FRESH_SECS)
        ));
        assert!(attention_band(
            Some(20),
            Some(120),
            Some(DEFAULT_PHYSICAL_FRESH_SECS)
        ));
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

    // --- marker_fresh ------------------------------------------------------

    #[test]
    fn a_marker_younger_than_the_ttl_means_the_phone_is_in_hand() {
        assert!(marker_fresh(Some(1_000), Some(1_100), Some(300)));
    }

    #[test]
    fn a_marker_older_than_the_ttl_has_expired() {
        assert!(!marker_fresh(Some(1_000), Some(1_400), Some(300)));
    }

    #[test]
    fn a_marker_exactly_at_the_ttl_has_expired() {
        assert!(!marker_fresh(Some(1_000), Some(1_300), Some(300)));
    }

    #[test]
    fn no_marker_at_all_is_simply_not_a_signal() {
        assert!(!marker_fresh(None, Some(1_100), Some(300)));
    }

    #[test]
    fn an_unreadable_clock_or_ttl_fails_closed_rather_than_inventing_a_signal() {
        assert!(!marker_fresh(Some(1_000), None, Some(300)));
        assert!(!marker_fresh(Some(1_000), Some(1_100), None));
    }

    #[test]
    fn a_marker_dated_in_the_future_is_still_fresh_rather_than_underflowing() {
        assert!(marker_fresh(Some(9_000), Some(1_000), Some(300)));
    }

    #[test]
    fn the_default_ttl_is_five_minutes() {
        assert_eq!(DEFAULT_PHONE_MARKER_TTL_SECS, 300);
        assert!(marker_fresh(
            Some(1_000),
            Some(1_299),
            Some(DEFAULT_PHONE_MARKER_TTL_SECS)
        ));
        assert!(!marker_fresh(
            Some(1_000),
            Some(1_300),
            Some(DEFAULT_PHONE_MARKER_TTL_SECS)
        ));
    }

    // --- the composition policy --------------------------------------------

    #[test]
    fn a_forced_viewing_verdict_wins_over_the_sampled_rate_both_ways() {
        assert!(moshi_viewing(Some(true), false));
        assert!(!moshi_viewing(Some(false), true));
    }

    #[test]
    fn viewing_falls_through_to_the_sampled_rate_when_nothing_forced_it() {
        assert!(moshi_viewing(None, true));
        assert!(!moshi_viewing(None, false));
    }

    #[test]
    fn a_forced_attention_verdict_wins_over_every_signal_both_ways() {
        assert!(phone_attention(Some(true), false, false));
        assert!(!phone_attention(Some(false), true, true));
    }

    #[test]
    fn a_fresh_marker_alone_is_attention() {
        assert!(phone_attention(None, true, false));
    }

    #[test]
    fn a_viewed_session_alone_is_attention() {
        assert!(phone_attention(None, false, true));
    }

    #[test]
    fn no_signal_at_all_leaves_the_plain_idle_rule_standing() {
        assert!(!phone_attention(None, false, false));
    }

    #[test]
    fn a_fresh_marker_never_counts_as_viewing() {
        // "Phone in hand" is not "this pane on screen", so the marker must not
        // reach the verdict that suppresses the card about the watched pane.
        let marker_is_fresh = true;
        assert!(phone_attention(
            None,
            marker_is_fresh,
            moshi_viewing(None, false)
        ));
        assert!(!moshi_viewing(None, false));
    }
}
