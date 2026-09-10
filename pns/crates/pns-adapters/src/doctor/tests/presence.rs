use super::*;

#[test]
fn the_room_sensor_line_names_what_the_last_decision_narrowed_the_lamps_to() {
    // The reading alone does not say what the lamps DID with it: the desk
    // overrules a room, an empty room falls back, and an operator staring
    // at a lamp in the wrong room needs to see which of those happened.
    assert_eq!(
        presence_line_with(
            PresenceStatus::Room {
                room: "2F - Kitchen".to_string(),
                age_secs: 4,
            },
            Some(pns_domain::PresenceDecision {
                room: Some("3F - Studio".to_string()),
                ..Default::default()
            }),
        ),
        "presence: 2F - Kitchen (4s ago); last narrowed to 3F - Studio"
    );
    // A ROUTING LEFT WHOLE NAMES ITS REASON, and is told from a room
    // STRUCTURALLY rather than by the shape of a phrase.
    assert_eq!(
        presence_line_with(
            PresenceStatus::Nowhere { poll_age_secs: 3 },
            Some(pns_domain::PresenceDecision {
                room: None,
                reason: "motion in no watched room".to_string(),
                ..Default::default()
            }),
        ),
        "presence: nowhere (last poll 3s ago); last narrowed nothing \
             (motion in no watched room)"
    );
    // AND A RING WITH NOTHING IN IT SAYS NOTHING, rather than claiming a
    // narrowing never decided: presence off, or on and never yet consulted.
    assert_eq!(
        presence_line_for(PresenceStatus::Nowhere { poll_age_secs: 3 }),
        "presence: nowhere (last poll 3s ago)"
    );
}

#[test]
fn a_room_name_the_bridge_chose_is_filtered_on_its_way_out_of_the_ring_too() {
    // It reached the ring as JSON, so a newline survives the round trip
    // intact; the terminal is where it would forge a second `pns doctor:`
    // line, and that is this filter's job.
    let said = presence_line_with(
        PresenceStatus::Nowhere { poll_age_secs: 3 },
        Some(pns_domain::PresenceDecision {
            room: Some("3F - Studio\npns doctor: forged".to_string()),
            ..Default::default()
        }),
    );
    assert!(!said.contains('\n'), "{said}");
}

#[test]
fn the_selected_room_sensor_is_a_reading_rather_than_the_sensor_skip() {
    // The router is the sensor with nothing to report and keeps the skip;
    // this one has a reading, and a bare "a sensor" line would leave a
    // machine whose bridge died looking like one that is fine.
    let config = "[plugins.presence]\nenabled = true\ntype = \"hue\"\n\
                      [plugins.hue]\nenabled = true\n";
    assert_eq!(kind_for(config, "presence"), CheckKind::Presence);
    assert_eq!(kind_for(config, "router"), CheckKind::Skipped(NOT_ENABLED));
}

#[test]
fn a_room_sensor_the_config_never_switched_on_is_still_a_skip() {
    // NOT SELECTED IS ASKED FIRST, or a plugin nobody enabled would print
    // a reading and read as switched on.
    assert_eq!(
        kind_for("[plugins.hermes]\nenabled = true\n", "presence"),
        CheckKind::Skipped(NOT_ENABLED)
    );
}

#[test]
fn a_known_room_is_named_with_the_age_of_its_motion_edge() {
    assert_eq!(
        presence_line_for(PresenceStatus::Room {
            room: "3F - Studio".to_string(),
            age_secs: 4,
        }),
        "presence: 3F - Studio (4s ago)"
    );
}

#[test]
fn a_fresh_poll_that_found_nobody_says_nowhere_rather_than_unknown() {
    assert_eq!(
        presence_line_for(PresenceStatus::Nowhere { poll_age_secs: 3 }),
        "presence: nowhere (last poll 3s ago)"
    );
}

#[test]
fn every_way_of_not_knowing_says_which_way_it_is() {
    // FIVE DIFFERENT EDITS: nothing published yet, a daemon or bridge that
    // stopped, a clock, a wrong epoch, and a room nobody watches. One
    // wording for all of them sends four operators in five to the wrong
    // file.
    assert_eq!(
        presence_line_for(PresenceStatus::Unknown(Unreadable::NoReading)),
        "presence: unknown (no reading)"
    );
    assert_eq!(
        presence_line_for(PresenceStatus::Unknown(Unreadable::NoClock)),
        "presence: unknown (the clock could not be read)"
    );
    assert_eq!(
        presence_line_for(PresenceStatus::Unknown(Unreadable::Stale {
            poll_age_secs: 42
        })),
        "presence: unknown (stale, poll 42s old)"
    );
    assert_eq!(
        presence_line_for(PresenceStatus::Unknown(Unreadable::Future)),
        "presence: unknown (future epoch)"
    );
    assert_eq!(
        presence_line_for(PresenceStatus::Unknown(Unreadable::NotWatched)),
        "presence: unknown (the reported room is not one this config watches)"
    );
}

#[test]
fn a_room_name_the_bridge_chose_is_filtered_before_it_reaches_the_terminal() {
    // An unfiltered newline forges a second `pns doctor:` line the operator
    // reads as pns's own verdict, and an escape rewrites the ones above it.
    let said = presence_line_for(PresenceStatus::Room {
        room: "3F\n\u{1b}[2Kpns doctor: all clear".to_string(),
        age_secs: 1,
    });
    assert_eq!(said.lines().count(), 1, "{said}");
    assert!(!said.contains('\u{1b}'), "{said}");
    // AND A NAME THAT FILTERS AWAY TO NOTHING IS NAMED, never printed as a
    // blank that reads as a room with no name.
    assert_eq!(
        presence_line_for(PresenceStatus::Room {
            room: "\u{30ad}\u{30c3}\u{30c1}\u{30f3}".to_string(),
            age_secs: 1,
        }),
        "presence: a room whose name will not print (1s ago)"
    );
}

#[test]
fn a_reading_is_never_counted_as_a_send_however_good_it_is() {
    // Nothing was delivered through a sensor and nothing failed to be, so
    // a green reading must not be what makes `pns doctor` exit 0.
    let outcomes = vec![Outcome::Presence(
        PresenceStatus::Room {
            room: "3F - Studio".to_string(),
            age_secs: 1,
        },
        None,
    )];
    assert_eq!(
        summary(&outcomes),
        "pns doctor: 0 sent, 0 failed, 1 skipped"
    );
    assert_eq!(exit_code(&outcomes, &pairing_report(None, None)), 1);
}
