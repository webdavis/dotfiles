use super::*;
use crate::profiles::Admits;

#[test]
fn every_way_a_profile_can_be_chosen_reads_back() {
    assert_eq!(because(&Chose::Fallback, &[], None), "no rule matched");
    assert_eq!(
        because(&Chose::Rule { index: 2 }, &["days", "hours"], None),
        "rule 2: days, hours"
    );
    assert_eq!(
        because(&Chose::Rule { index: 1 }, &[], None),
        "rule 1: every time"
    );
    assert_eq!(
        because(&Chose::Manual { until: None }, &[], None),
        "manual, until cleared"
    );
    assert_eq!(
        because(&Chose::Manual { until: Some(1) }, &[], Some("17:30")),
        "manual, until 17:30"
    );
    assert_eq!(
        because(&Chose::Manual { until: Some(1) }, &[], None),
        "manual, until an hour this machine cannot read",
        "an unreadable clock never prints a time it made up"
    );
}

#[test]
fn the_surfaces_read_in_one_line() {
    assert_eq!(
        surfaces_line(&Profile::default()),
        "quiet off; banner all, Discord all, phone all, lights all"
    );
    assert_eq!(
        surfaces_line(&Profile {
            quiet: true,
            banner: Admits::None,
            discord: Admits::None,
            phone: Admits::Priority,
            lights: Admits::None,
        }),
        "quiet on; banner none, Discord none, phone priority, lights none"
    );
}
