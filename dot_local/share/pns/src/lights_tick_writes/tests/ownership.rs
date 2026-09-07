mod tests {
    use super::super::*;
    use crate::runtime_test_support::*;

    #[test]
    fn a_tick_whose_record_moved_under_it_stands_down_rather_than_re_arming_the_lamps() {
        // THE RACE THE SOURCE USED TO ADMIT TO. The house is derived BEFORE the
        // bridge work, which is seconds of network, and the operator's return
        // clears every held lamp and empties the record in the middle of it: a
        // tick that then published its own snapshot armed the lamps again and
        // the operator watched a lamp they had just put out come back on, with
        // the record naming it once more.
        //
        // THE OTHER WRITER HAS ALREADY DONE THE CLEARING, so standing down is
        // the whole remedy: nothing is armed, nothing is cleared twice, and the
        // next tick reads a house that agrees with the disk.
        let state = scratch("tick-record-moved");
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            // WHAT THIS TICK READ before the bridge work, against a record the
            // event path has emptied since.
            Some(&[pns::lights::HeldEntry::bare(LAMP_PATH)]),
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert!(
            bridge.puts.borrow().is_empty(),
            "the lamps were re-armed off a snapshot the disk had already moved past: {:?}",
            bridge.puts.borrow()
        );
        assert_eq!(
            recorded(&state),
            None,
            "and the record the other writer left is not overwritten either"
        );
        assert!(complaints.is_empty(), "{complaints:?}");
    }

    #[test]
    fn a_second_tick_stands_down_while_a_first_still_holds_the_lamps() {
        // THE GUARD THE DAEMON'S OWN BOOKKEEPING CANNOT BE. `decide` refuses to
        // fire a second lights child while the first is listed, and that list is
        // ONE process's memory: a tick the operator ran by hand and an orphan a
        // daemon replacement left behind are both invisible to it. Two ticks
        // driving one lamp interleave their fades, and the phase the last of
        // them writes is the one the next tick resumes off.
        let state = scratch("tick-lock-held");
        std::fs::write(state.join(LIGHTS_TICK_LOCK), "").expect("a lock a live tick holds");
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
        assert!(
            bridge.puts.borrow().is_empty(),
            "a second tick drove the lamps while the first still held them: {:?}",
            bridge.puts.borrow()
        );
        assert_eq!(
            recorded(&state),
            None,
            "and it wrote no record over the holder's own"
        );
        assert!(complaints.is_empty(), "{complaints:?}");

        // AND A LOCK NO LIVE TICK COULD STILL BE HOLDING IS TAKEN, so an orphan
        // costs one stale window rather than the lamps forever. The moment is
        // handed in rather than waited out: this test never sleeps.
        let long_past_any_holder_ms = 4_000_000_000_000;
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[]),
            long_past_any_holder_ms,
            None,
            no_time_passes(),
            |_| {},
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        assert!(
            !bridge.puts.borrow().is_empty(),
            "a lock older than any tick may hold it was never taken, so the lamps \
             stayed dark for as long as the orphan sat there"
        );
        assert!(
            !state.join(LIGHTS_TICK_LOCK).exists(),
            "and the tick that took it never gave it back, which stands every later \
             tick down for a whole stale window"
        );
    }

    #[test]
    fn a_tick_whose_bridge_answered_nothing_keeps_the_record_it_was_holding() {
        // A LISTING THAT FAILED IS DIRECT EVIDENCE THE TRANSPORT IS DOWN, and
        // clearing off it forgets the paths after PUTs nobody can prove landed.
        // The lamp is then lit with nothing left in the system that knows about
        // it: the condition ends, so no later tick has anything held to clear,
        // and the event path reads an empty record and returns without a call.
        let state = scratch("bridge-down-keeps-the-record");
        std::fs::write(state.join(LIGHTS_HELD), format!("{LAMP_PATH}\n")).expect("the held record");

        let bridge = scripted(false);
        let complaints = run_tick_writes(
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
            bridge.puts.borrow().is_empty(),
            "a bridge that answered no listing is written to for nothing: {:?}",
            bridge.puts.borrow()
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        assert_eq!(
            recorded(&state).as_deref(),
            Some(LAMP_PATH),
            "and the record survives the outage, so the next reachable tick still \
             has a name to write the clear to"
        );
    }
}
