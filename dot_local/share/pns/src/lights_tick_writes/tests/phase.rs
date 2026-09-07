mod tests {
    use super::super::*;
    use crate::runtime_test_support::*;
    use std::cell::RefCell;

    #[test]
    fn the_recorded_end_counts_the_resolve_the_driver_started_after() {
        // THE DRIVER'S TIMELINE STARTS AFTER THE RESOLVE, so a landing it
        // reports is an offset from a moment three bridge calls later than the
        // tick's own. Written into the record without that term, every end
        // would be a whole resolve early and the next tick would take the
        // breath over before this one had finished it: exactly the pause this
        // slice exists to remove, reintroduced through the record.
        let lights = *pns::config::parse_config(
            "[lights]\nrefresh_secs = 12\n\
             [lights.room.\"3F - Studio\"]\nshows = [\"blocked\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        let state = scratch("resolve-counted-in-the-record");
        let clock = FakeClock::default();
        let bridge = SlowBridge {
            clock: &clock,
            get_cost_ms: 250,
            put_cost_ms: 0,
            answers: true,
            puts: RefCell::new(Vec::new()),
        };
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &lights,
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[]),
            0,
            None,
            || clock.elapsed_ms(),
            |waited| clock.slept(waited),
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        assert_eq!(
            read_held(&state).expect("a record this tick wrote"),
            vec![pns::lights::HeldEntry {
                path: LAMP_PATH.to_string(),
                resume: Some(pns::lights::Phase {
                    end_unix_ms: 12_500,
                    landed_on: 100,
                    held: pns::lights::Held::Blocked,
                }),
            }],
            "three listings at 250ms leave an 11,250ms budget, whose sixth and last \
             fade is issued 9,750ms into the DRIVER and ends 2,000ms later: 12,500ms \
             from the moment the tick itself began"
        );
    }

    #[test]
    fn a_resumed_breath_composes_across_two_ticks_on_a_fake_clock() {
        // THE HANDOFF, END TO END, on numbers a real clock never has to
        // supply: both ticks are handed their own `now_ms`, so nothing here
        // sleeps or waits for real time. Tick one's breath lands on an end
        // and records it; tick two reads that record and picks the breath
        // back up from exactly where it left off.
        let lights = *pns::config::parse_config(
            "[lights]\nrefresh_secs = 12\n\
             [lights.room.\"3F - Studio\"]\nshows = [\"blocked\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        let state = scratch("resumed-breath-two-ticks");

        // TICK ONE, at N=0, with nothing yet held: the locked blocked shape's
        // seven fades (the seamless schedule at a twelve-second budget) land
        // on low, 13,700ms after this tick's own start.
        let clock = FakeClock::default();
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &lights,
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[]),
            0,
            None,
            || clock.elapsed_ms(),
            |waited| clock.slept(waited),
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        let held_after_tick_one = read_held(&state).expect("a record this tick wrote");
        assert_eq!(
            held_after_tick_one,
            vec![pns::lights::HeldEntry {
                path: LAMP_PATH.to_string(),
                resume: Some(pns::lights::Phase {
                    end_unix_ms: 13_700,
                    landed_on: 30,
                    held: pns::lights::Held::Blocked,
                }),
            }],
            "seven fades of the locked blocked shape land on low at 13,700ms"
        );

        // TICK TWO, at N=12,400: the previous tick's last fade does not
        // finish landing on the bridge until 13,700, less the seamless
        // lead, less now, which is 1,250ms still to wait.
        let sleeps: RefCell<Vec<Duration>> = RefCell::new(Vec::new());
        let clock = FakeClock::default();
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &lights,
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&held_after_tick_one),
            12_400,
            None,
            || clock.elapsed_ms(),
            |waited| {
                sleeps.borrow_mut().push(waited);
                clock.slept(waited);
            },
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        // EXACTLY 1,250ms, not a tolerance: the clock this tick was handed
        // moves only when the sleeper moves it, so nothing here reads or waits
        // on wall-clock time and the number is the schedule's own.
        assert_eq!(
            sleeps.borrow()[0],
            Duration::from_millis(1_250),
            "tick two's first fade is due 1,250ms in, and it sleeps that out \
             before issuing anything"
        );
        let puts = bridge.puts.borrow();
        assert!(
            puts[0].1.contains(r#""brightness":100.0"#) && puts[0].1.contains("color"),
            "tick one landed on low, so tick two resumes toward high, armed with \
             the colour and `on` again: {}",
            puts[0].1
        );
    }

    #[test]
    fn a_lamp_that_changed_state_starts_its_new_colour_at_once_rather_than_resuming() {
        // THE LOCKED PRECEDENCE IS "RED WINS, BLOCKED OUTRANKS LOOP", and a
        // resume taken on the fixture path alone delays it. The slow loop shape
        // lands its last fade almost four seconds past the interval that issued
        // it; the next tick, now holding BLOCKED, would wait that fade out
        // before its first blocked body reached the lamp, because the first fade of
        // every tick is the one that carries the colour. The same delay hits an
        // unread lamp that has to turn red.
        let lights = *pns::config::parse_config(
            "[lights]\nrefresh_secs = 12\n\
             [lights.room.\"3F - Studio\"]\nshows = [\"blocked\", \"loop\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        let state = scratch("state-change-starts-at-once");

        // TICK ONE holds the LOOP state, whose four-second shape issues its
        // last fade at 11,850ms and lands it 15,850ms after this tick began.
        let clock = FakeClock::default();
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &lights,
            &[pns::lights::Held::Looping],
            &noon(&nothing_muted()),
            Some(&[]),
            0,
            None,
            || clock.elapsed_ms(),
            |waited| clock.slept(waited),
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        let held_after_the_loop = read_held(&state).expect("a record this tick wrote");

        // TICK TWO holds BLOCKED instead. Resumed off the loop's phase it would
        // sleep 3,400ms before its first blocked body; it starts down at once
        // instead, and only then keeps the blocked cadence.
        let sleeps: RefCell<Vec<Duration>> = RefCell::new(Vec::new());
        let clock = FakeClock::default();
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &lights,
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&held_after_the_loop),
            12_400,
            None,
            || clock.elapsed_ms(),
            |waited| {
                sleeps.borrow_mut().push(waited);
                clock.slept(waited);
            },
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        assert_eq!(
            sleeps.borrow().first().copied(),
            Some(Duration::from_millis(1_950)),
            "the first blocked fade is issued before anything is slept for, so the \
             first sleep is the blocked shape's own step"
        );
    }

    #[test]
    fn the_phase_reaches_disk_only_after_the_breath_that_earned_it_has_run() {
        // THE PRE-ARM WRITE IS BARE, AND THE PHASE IS A SECOND WRITE. A record
        // written with its phase BEFORE the fades are issued is a promise about
        // a breath that has not happened: a child killed mid-interval would
        // leave the next tick resuming from an end no lamp ever reached, and
        // the whole point of the bare token is that a killed child leaves
        // something this run cannot promise anything about.
        let state = scratch("phase-lands-after-the-breath");
        let clock = FakeClock::default();
        let bridge = scripted(true);
        let seen_mid_breath: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[]),
            0,
            None,
            || clock.elapsed_ms(),
            |waited| {
                seen_mid_breath
                    .borrow_mut()
                    .push(recorded(&state).unwrap_or_default());
                clock.slept(waited);
            },
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        assert_eq!(
            seen_mid_breath.borrow().first().map(String::as_str),
            Some(LAMP_PATH),
            "the record carried a phase while the breath was still being issued"
        );
        assert!(
            recorded(&state).is_some_and(|line| line.starts_with(&format!("{LAMP_PATH}@"))),
            "and the phase never landed once the breath had actually run: {:?}",
            recorded(&state)
        );
    }

    #[test]
    fn a_record_cleared_during_the_breath_is_left_cleared_rather_than_resurrected() {
        // THE OPERATOR'S RETURN, ARRIVING MID-BREATH. It clears every held lamp
        // and empties this record from a process that holds no lock, and the
        // phase write comes seconds later: written unguarded it would put the
        // lamp back into the record with a phase attached, so the pulse gate
        // would go on treating a lamp the operator just put out as held.
        let state = scratch("record-cleared-mid-breath");
        let clock = FakeClock::default();
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
            || clock.elapsed_ms(),
            |waited| {
                let _ = std::fs::remove_file(state.join(LIGHTS_HELD));
                clock.slept(waited);
            },
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        assert_eq!(
            recorded(&state),
            None,
            "the phase write resurrected a hold the return had already ended"
        );
    }
}
