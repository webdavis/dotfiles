mod tests {
    use super::super::*;
    use crate::runtime_test_support::*;
    use std::cell::RefCell;

    #[test]
    fn the_tick_says_what_could_not_be_resolved_and_what_was_refused() {
        // THE LOUD HALF of "a dark lamp must never be ambiguous with a typo":
        // the resolution's findings have to leave the tick as complaints, or an
        // unattended machine routes a behaviour to a name nobody can light and
        // no one is ever told.
        let state = scratch("tick-complains");
        let bridge = scripted(true);
        let lights = *pns::config::parse_config(
            "[lights]\nrefresh_secs = 10\n\
             [lights.room.\"3F - Studio\"]\nshows = [\"blocked\"]\n\
             dim_window = \"2200-0700\"\n\
             [lights.lamp.\"3F - Nowhere\"]\nshows = [\"blocked\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &lights,
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[]),
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert_eq!(
            complaints,
            vec![
                "pns lights: `3F - Nowhere` (lamp) is not on the bridge".to_string(),
                "pns lights: `3F - Studio - HCL1` has dim_window \"2200-0700\", which is \
                 not a HH:MM-HH:MM window; that lamp stays dark"
                    .to_string(),
            ],
        );
    }
    #[test]
    fn a_held_lamp_breathes_only_in_the_room_the_reading_names() {
        // THE TICK'S OWN HALF OF THE WIRING. It is the path the SUSTAINED lamp
        // takes, so a narrowing wired into the pulse alone would leave a
        // blocked breath lit in every room while the operator sits in one.
        let lights = *pns::config::parse_config(
            "[lights]\nrefresh_secs = 10\n\
             [lights.room.\"3F - Studio\"]\nshows = [\"blocked\"]\n\
             [lights.room.\"2F - Kitchen\"]\nshows = [\"blocked\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        let bridge = TwoRoomBridge {
            puts: RefCell::new(Vec::new()),
        };
        let state = scratch("tick-narrowed-by-presence");
        run_tick_writes(
            &bridge,
            &state,
            &lights,
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[]),
            0,
            Some(&at_the_desk(pns::presence::PresenceStatus::Nowhere {
                poll_age_secs: 1,
            })),
            no_time_passes(),
            |_| {},
        );
        let armed: Vec<String> = bridge
            .puts
            .borrow()
            .iter()
            .map(|(path, _)| path.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        assert_eq!(
            armed,
            vec!["light/l1".to_string()],
            "only the lamp in the desk's own room is armed"
        );
        assert_eq!(
            last_narrowing(&state).and_then(|entry| entry.room),
            Some("3F - Studio".to_string()),
            "and the decision is where the doctor reads it back"
        );
    }
    #[test]
    fn a_tick_narrows_over_the_lamps_this_state_would_reach_and_not_the_rest() {
        // The tick's half of the same rule, through `shown` rather than
        // `pulse_fires`: a kitchen lamp carrying only `unread` is not a lamp a
        // blocked wait can breathe on.
        let lights = *pns::config::parse_config(
            "[lights]\nrefresh_secs = 10\n\
             [lights.room.\"3F - Studio\"]\nshows = [\"blocked\"]\n\
             [lights.room.\"2F - Kitchen\"]\nshows = [\"unread\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        let bridge = TwoRoomBridge {
            puts: RefCell::new(Vec::new()),
        };
        let state = scratch("tick-narrow-over-eligible");
        run_tick_writes(
            &bridge,
            &state,
            &lights,
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[]),
            0,
            Some(&in_the_kitchen()),
            no_time_passes(),
            |_| {},
        );
        assert_eq!(
            bridge
                .puts
                .borrow()
                .iter()
                .map(|(path, _)| path.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>(),
            vec!["light/l1".to_string()],
            "the kitchen carries nothing for this state, so the whole routing stands"
        );
    }
}
