mod tests {
    use super::super::*;
    use crate::runtime_test_support::*;

    #[test]
    fn a_tick_arms_a_held_lamp_records_it_and_a_dark_house_puts_it_out_by_name() {
        // THE ARM, THE RECORD AND THE CLEAR ARE ONE ORDERED TRIO, and this is
        // that trio. Every held body is a plain state write that does NOT
        // expire, so a record written before the clear, or a clear computed
        // before the arm, is a lamp left lit with nothing that knows its name.
        let state = scratch("tick-arms-and-clears");
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[]),
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        let puts = bridge.puts.borrow();
        assert_eq!(
            puts.first().map(|(path, _)| path.as_str()),
            Some(LAMP_PATH),
            "the lamp is addressed individually, never through its room's group: \
             arbitration, the dim window and the mute are each per lamp, and a \
             group write would reach one that answered any of the three differently"
        );
        assert!(
            puts[0].1.contains(r#""x":0.3395"#) && puts[0].1.contains(r#""brightness":30.0"#),
            "the arm states the blocked magenta and the first fade in one write: {}",
            puts[0].1
        );
        assert!(
            puts.len() > 1 && !puts[1].1.contains("color"),
            "and every fade after it states brightness and duration alone"
        );
        assert_eq!(
            held_lamps(&state).as_deref(),
            Some([LAMP_PATH.to_string()].as_slice()),
            "the record carries the lamp, or nothing will ever put it out"
        );
        assert!(
            recorded(&state)
                .expect("a record is on disk")
                .starts_with(&format!("{LAMP_PATH}@")),
            "and the second write, after the breath returns, carries the phase \
             the lamp landed on"
        );

        // THE OTHER DIRECTION, which is what the clear exists for: a house with
        // nothing to show writes to no lamp at all, so the held path really is
        // stale and goes out by name.
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[],
            &noon(&nothing_muted()),
            Some(&[pns::lights::HeldEntry::bare(LAMP_PATH)]),
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert_eq!(
            bridge.puts.borrow().as_slice(),
            &[(LAMP_PATH.to_string(), CLEAR_BODY.to_string())],
            "the lamp is put out by name, off the recorded path, with no listing \
             resolved at all"
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        assert_eq!(
            recorded(&state),
            None,
            "and the tick stops claiming to hold it"
        );
    }

    #[test]
    fn a_phased_record_clears_by_its_bare_path_never_by_the_suffix() {
        // THE SUFFIX A RESUMED BREATH WRITES MUST NEVER LEAK INTO A PUT PATH.
        // A lamp the previous tick recorded with a phase is cleared exactly
        // like a bare one: by the fixture path alone.
        let state = scratch("tick-phased-record-clears-bare");
        std::fs::write(
            state.join(LIGHTS_HELD),
            format!("{LAMP_PATH}@1700000000123:h\n"),
        )
        .expect("a phased record");
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[],
            &noon(&nothing_muted()),
            Some(&[pns::lights::HeldEntry {
                path: LAMP_PATH.to_string(),
                resume: Some(pns::lights::Phase {
                    end_unix_ms: 1_700_000_000_123,
                    landed_on: 100,
                    held: pns::lights::Held::Blocked,
                }),
            }]),
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert_eq!(
            bridge.puts.borrow().as_slice(),
            &[(LAMP_PATH.to_string(), CLEAR_BODY.to_string())],
            "the clear addresses the bare path, never `{LAMP_PATH}@1700000000123:h`"
        );
        assert!(complaints.is_empty(), "{complaints:?}");
    }

    #[test]
    fn a_lamp_this_arm_wrote_to_stays_held_rather_than_being_put_out_behind_the_arm() {
        // THE CLEAR SUBTRACTS EVERY PATH THIS ARM WROTE TO, and it has to: a
        // held body is a plain state write, so a clear computed as "everything
        // that was held" would PUT the arm and then the off to the same lamp on
        // every single re-arm, in that order, and the lamp would be dark for the
        // whole of every interval after the first.
        let state = scratch("tick-rearm-keeps-the-lamp");
        // THE RECORD ON DISK IS WHAT THE TICK READ, and it has to agree with
        // the reading handed in: the pass stands down when the record moved
        // under it, which is how a return that cleared every lamp mid-tick
        // stops this run re-arming them.
        std::fs::write(state.join(LIGHTS_HELD), format!("{LAMP_PATH}\n")).expect("the record");
        let bridge = scripted(true);
        run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[pns::lights::HeldEntry::bare(LAMP_PATH)]),
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert!(
            !bridge
                .puts
                .borrow()
                .iter()
                .any(|(_, body)| body == CLEAR_BODY),
            "no off reaches a lamp this arm wrote to: {:?}",
            bridge.puts.borrow()
        );
        assert_eq!(
            held_lamps(&state).as_deref(),
            Some([LAMP_PATH.to_string()].as_slice()),
            "and it is still recorded as held, or nothing will ever put it out"
        );
    }

    #[test]
    fn a_lamp_the_operator_muted_is_not_armed_and_is_put_out_if_it_was_held() {
        // THE MUTE IS A RENDER FILTER AT THE PER-LAMP DECISION, decided once:
        // the lamp simply drops out of the arm, which makes its held path stale
        // and puts it out through the ordinary clear rather than a second path.
        let state = scratch("tick-mute-clears");
        // THE RECORD ON DISK IS WHAT THE TICK READ, and it has to agree with
        // the reading handed in: the pass stands down when the record moved
        // under it, which is how a return that cleared every lamp mid-tick
        // stops this run re-arming them.
        std::fs::write(state.join(LIGHTS_HELD), format!("{LAMP_PATH}\n")).expect("the record");
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[pns::lights::Held::Blocked],
            &noon(&quieted("3F - Studio")),
            Some(&[pns::lights::HeldEntry::bare(LAMP_PATH)]),
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert_eq!(
            bridge.puts.borrow().as_slice(),
            &[(LAMP_PATH.to_string(), CLEAR_BODY.to_string())],
            "a muted lamp is armed with nothing and put out if it was lit"
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        assert_eq!(recorded(&state), None);
    }

    #[test]
    fn a_mute_reading_nobody_could_take_leaves_every_lamp_quiet_rather_than_loud() {
        // THE FAIL DIRECTION ON A LAMP PATH IS DARK. An unreadable mute record
        // and a clock that would not answer each arrived at the walk as an
        // EMPTY list of quiet places, which is a house with every lamp loud:
        // the one outcome the operator armed the mute to prevent, on the one
        // night the machine could not say why.
        let state = scratch("tick-mute-unreadable");
        // THE RECORD ON DISK IS WHAT THE TICK READ, and it has to agree with
        // the reading handed in: the pass stands down when the record moved
        // under it, which is how a return that cleared every lamp mid-tick
        // stops this run re-arming them.
        std::fs::write(state.join(LIGHTS_HELD), format!("{LAMP_PATH}\n")).expect("the record");
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[pns::lights::Held::Blocked],
            &noon(&pns::channels::hue::Muting::Everything),
            Some(&[pns::lights::HeldEntry::bare(LAMP_PATH)]),
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert_eq!(
            bridge.puts.borrow().as_slice(),
            &[(LAMP_PATH.to_string(), CLEAR_BODY.to_string())],
            "every lamp is quiet, so the lamp is armed with nothing and put out"
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        assert_eq!(recorded(&state), None);
    }

    #[test]
    fn a_held_record_that_will_not_publish_stops_the_arm_rather_than_lighting_a_lamp() {
        // A LAMP THE RECORD DOES NOT NAME IS A LAMP NOTHING CAN PUT OUT. Every
        // held body is a plain state write that does not expire, and the next
        // tick, the return from an absence and the operator's own mute all
        // clear BY NAME off this file, so arming after a failed publish is a
        // bulb held by nothing until somebody finds the wall switch.
        let state = scratch("tick-record-unwritable");
        std::fs::create_dir(state.join(LIGHTS_HELD)).expect("a directory where the record goes");
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            None,
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert!(
            bridge.puts.borrow().is_empty(),
            "no lamp is armed once the record refused to land: {:?}",
            bridge.puts.borrow()
        );
        assert!(
            complaints
                .iter()
                .any(|said| said.contains("the held record could not be written")),
            "and the tick says so rather than carrying on quietly: {complaints:?}"
        );
    }
}
