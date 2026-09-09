//! The decision, pinned: readings.

use super::fixtures::{CountingProbes, decide, decide_with, three_selection};
use pns_domain::Overrides;
use std::collections::BTreeMap;

#[test]
fn a_reading_nobody_could_take_is_reported_as_absent_and_never_as_a_number() {
    // AN ABSENCE IS NOT A ZERO. Every field here is an `Option` precisely
    // so an unread probe stays unread in the record: a `0` would read as
    // "touched this instant" and a `false` lock would read as "the screen
    // was awake", each of which explains a decision by an observation
    // nobody made.
    let all_readable = CountingProbes {
        idle: Some(30),
        marker_mtime: Some(999_400),
        phone_atime: Some(999_912),
        screen_locked: Some(true),
        ..CountingProbes::default()
    };

    // A GARBLED THRESHOLD: there is no window, so nothing below it was
    // measured either.
    let garbled = Overrides::from_env(&BTreeMap::from([(
        "PNS_DESK_IDLE_SECS".to_string(),
        "0600".to_string(),
    )]));
    let inputs = decide_with(&all_readable, &garbled, "").inputs;
    assert_eq!(inputs.desk_fresh_secs, None, "no window to measure against");
    assert_eq!(inputs.desk_input_age, None);
    assert_eq!(inputs.phone_input_age, None);
    assert_eq!(inputs.marker_age, None);
    assert_eq!(inputs.screen_locked, None);

    // AN UNREADABLE CLOCK ages nothing, so neither phone signal has an
    // age, while the desk clock, which is an age already, still does.
    let inputs = decide(
        &all_readable,
        &three_selection(),
        &Overrides::default(),
        pns_domain::DeliveryScope::Automatic,
        "",
        None,
        false,
        false,
    )
    .inputs;
    assert_eq!(inputs.phone_input_age, None, "aged against no clock");
    assert_eq!(inputs.marker_age, None, "aged against no clock");
    assert_eq!(inputs.desk_input_age, Some(30));

    // AN UNREAD LOCK is neither locked nor unlocked. The probe is skipped
    // wherever the idle clock answered nothing, which is exactly where a
    // `false` would claim a display somebody was sitting at.
    let no_idle_reading = CountingProbes {
        idle: None,
        screen_locked: Some(true),
        ..CountingProbes::default()
    };
    let inputs = decide_with(&no_idle_reading, &Overrides::default(), "").inputs;
    assert_eq!(inputs.screen_locked, None);
    assert_eq!(no_idle_reading.lock_reads.get(), 0, "and never read at all");
}
