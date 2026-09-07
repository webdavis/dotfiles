mod tests {
    use super::super::*;
    use crate::runtime_test_support::*;
    use std::cell::RefCell;

    #[test]
    fn a_pulse_reaches_only_a_routed_lamp_that_is_neither_muted_nor_held() {
        // THE EVENT PATH'S TWO PER-LAMP GATES, at the seam. The TCP spy the
        // integration tests dial can only count connections, and the resolve's
        // GETs happen either way, so a gate dropped here is invisible to every
        // other test in the crate.
        let lights = *pns::config::parse_config(
            "[lights]\n[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        let free = scripted(true);
        run_pulse_writes(
            &free,
            &scratch("pulse-writes-free"),
            &lights,
            pns::config::Behaviour::Done,
            &noon(&nothing_muted()),
            Some(&[]),
            None,
        );
        let puts = free.puts.borrow();
        assert_eq!(puts.len(), 1, "{puts:?}");
        assert_eq!(
            puts[0].0, LAMP_PATH,
            "the pulse reaches the routed lamp individually"
        );
        assert!(
            puts[0].1.contains("signaling"),
            "and it is the bridge-run signal body: {}",
            puts[0].1
        );
        // THE MUTE IS A RENDER FILTER AT THE PER-LAMP DECISION, on this path
        // exactly as on the tick's.
        let muted = scripted(true);
        run_pulse_writes(
            &muted,
            &scratch("pulse-writes-muted"),
            &lights,
            pns::config::Behaviour::Done,
            &noon(&quieted("3F - Studio")),
            Some(&[]),
            None,
        );
        assert!(
            muted.puts.borrow().is_empty(),
            "a muted lamp is not flashed: {:?}",
            muted.puts.borrow()
        );
        // AND A MUTE READING NOBODY COULD TAKE MUTES EVERY LAMP, which is the
        // fail direction on a lamp path: an unreadable record or clock arrived
        // here as an empty list, which is a house with every lamp loud.
        let dark = scripted(true);
        run_pulse_writes(
            &dark,
            &scratch("pulse-writes-dark"),
            &lights,
            pns::config::Behaviour::Done,
            &noon(&pns::channels::hue::Muting::Everything),
            Some(&[]),
            None,
        );
        assert!(
            dark.puts.borrow().is_empty(),
            "a mute nobody could read let the lamp flash anyway: {:?}",
            dark.puts.borrow()
        );
        // AND THE TICK'S HELD RECORD PREEMPTS THE PULSE on the lamp it holds,
        // which is the dedicated-but-helps-when-free ruling's event-path half.
        let held = scripted(true);
        run_pulse_writes(
            &held,
            &scratch("pulse-writes-held"),
            &lights,
            pns::config::Behaviour::Done,
            &noon(&nothing_muted()),
            Some(&[LAMP_PATH.to_string()]),
            None,
        );
        assert!(
            held.puts.borrow().is_empty(),
            "a held lamp is not flashed over: {:?}",
            held.puts.borrow()
        );
        // AND A PHASED RECORD ON DISK GATES EXACTLY LIKE A BARE ONE: the
        // suffix a resumed breath now writes must never leak into this gate,
        // which reads bare paths off `held_lamps`, the same parser the breath
        // itself reads a phase from.
        let state = scratch("pulse-gate-phased-record");
        std::fs::write(
            state.join(LIGHTS_HELD),
            format!("{LAMP_PATH}@1700000000123:h\n"),
        )
        .expect("a phased record");
        let phased = scripted(true);
        run_pulse_writes(
            &phased,
            &state,
            &lights,
            pns::config::Behaviour::Done,
            &noon(&nothing_muted()),
            held_lamps(&state).as_deref(),
            None,
        );
        assert!(
            phased.puts.borrow().is_empty(),
            "a phased record still gates the pulse over the lamp it names: {:?}",
            phased.puts.borrow()
        );
        // AND A HELD RECORD NOBODY COULD READ HOLDS EVERY LAMP, for the same
        // reason: read as nothing held, a corrupt record let a blink write
        // straight over a lamp breathing about a question.
        let unreadable = scripted(true);
        run_pulse_writes(
            &unreadable,
            &scratch("pulse-writes-unreadable"),
            &lights,
            pns::config::Behaviour::Done,
            &noon(&nothing_muted()),
            None,
            None,
        );
        assert!(
            unreadable.puts.borrow().is_empty(),
            "a held record nobody could read let the pulse fire anyway: {:?}",
            unreadable.puts.borrow()
        );
    }
    #[test]
    fn a_pulse_narrows_over_the_lamps_this_behaviour_would_reach_and_not_the_rest() {
        // NARROWING TO A ROOM THAT CARRIES THE BEHAVIOUR IS NOT THE SAME
        // QUESTION as narrowing to a room that holds a lamp. The kitchen holds
        // one, routed for `blocked` alone; narrowed first and filtered second,
        // a `done` event kept that lamp, then dropped it at the per-lamp gate
        // and wrote nothing at all, which is the silence the fallback exists
        // to prevent.
        let lights = *pns::config::parse_config(
            "[lights]\n[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
             [lights.room.\"2F - Kitchen\"]\nshows = [\"blocked\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        let bridge = TwoRoomBridge {
            puts: RefCell::new(Vec::new()),
        };
        let state = scratch("pulse-narrow-over-eligible");
        run_pulse_writes(
            &bridge,
            &state,
            &lights,
            pns::config::Behaviour::Done,
            &noon(&nothing_muted()),
            Some(&[]),
            Some(&in_the_kitchen()),
        );
        assert_eq!(
            bridge
                .puts
                .borrow()
                .iter()
                .map(|(path, _)| path.clone())
                .collect::<Vec<_>>(),
            vec!["light/l1".to_string()],
            "the kitchen carries nothing for this event, so the whole routing stands"
        );
        assert_eq!(
            last_narrowing(&state).and_then(|entry| entry.room),
            None,
            "and the record says the routing was left whole"
        );
    }
    #[test]
    fn a_pulse_reaches_only_the_room_the_reading_names_and_records_the_decision() {
        // THE WIRING, not the rule. `narrow` is pure and total, so every one of
        // its unit tests stays green with this call site gutted: what is pinned
        // here is that the pulse path narrows AT ALL, and that the decision is
        // written where `pns doctor` reads it back.
        let lights = *pns::config::parse_config(
            "[lights]\n[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
             [lights.room.\"2F - Kitchen\"]\nshows = [\"done\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        let bridge = TwoRoomBridge {
            puts: RefCell::new(Vec::new()),
        };
        let state = scratch("pulse-narrowed-by-presence");
        run_pulse_writes(
            &bridge,
            &state,
            &lights,
            pns::config::Behaviour::Done,
            &noon(&nothing_muted()),
            Some(&[]),
            Some(&in_the_kitchen()),
        );
        assert_eq!(
            bridge
                .puts
                .borrow()
                .iter()
                .map(|(path, _)| path.clone())
                .collect::<Vec<_>>(),
            vec!["light/l2".to_string()],
            "only the lamp in the room the reading named is flashed"
        );
        assert_eq!(
            last_narrowing(&state).and_then(|entry| entry.room),
            Some("2F - Kitchen".to_string()),
            "and the decision is where the doctor reads it back"
        );
    }

    #[test]
    fn the_pulse_path_says_what_it_could_not_resolve_rather_than_dropping_it() {
        // THE PATH A PULSE-ONLY MAP ACTUALLY TAKES. A config that routes only
        // `done` and `failed` holds no state, so its tick never resolves
        // anything and never complains; every resolution such a machine ever
        // does happens right here, and the findings were discarded on the
        // floor. A mistyped lamp name was therefore dark forever with the whole
        // system silent about it.
        let lights = *pns::config::parse_config(
            "[lights]\n[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
             [lights.lamp.\"3F - Nowhere\"]\nshows = [\"done\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        assert_eq!(
            run_pulse_writes(
                &scripted(true),
                &scratch("pulse-writes-complaints"),
                &lights,
                pns::config::Behaviour::Done,
                &noon(&nothing_muted()),
                Some(&[]),
                None,
            ),
            vec!["pns lights: `3F - Nowhere` (lamp) is not on the bridge".to_string()],
        );
    }
}
