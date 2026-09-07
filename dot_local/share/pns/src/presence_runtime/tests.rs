mod tests {
    use super::super::*;
    use crate::runtime_test_support::*;

    #[test]
    fn the_snapshot_is_taken_off_the_callers_own_probe_set_and_never_a_fresh_one() {
        // ONE MOMENT, STRUCTURALLY. `SystemProbes` memoizes its clock and its
        // presence line, so a snapshot built off the caller's set cannot have
        // the reading aged against one clock and the decision made against
        // another. Built off a set of its own it can, and did: a reading fresh
        // at fourteen seconds when the surface was chosen was stale at sixteen
        // when presence was read, turning one room into the whole house.
        let state = scratch("snapshot-one-probe-set");
        let line = state.join("presence");
        let now = now_secs().expect("a clock");
        std::fs::write(&line, format!("{now} {now} 1 3F - Studio\n")).expect("a reading");
        let probes = system_probes().with_presence_path(line.to_string_lossy().into_owned());
        let settings = pns::config::Presence {
            rooms: vec!["3F - Studio".to_string()],
            exclude: Vec::new(),
            desk_room: None,
            desk_stale_after_secs: 120,
            poll_secs: 5,
            stale_after_secs: 15,
        };
        let snapshot = presence_snapshot(
            Some(&settings),
            &probes,
            Some(9),
            Some(false),
            pns::home::HomePresence::Unknown,
        )
        .expect("an armed table takes a snapshot");
        assert_eq!(
            snapshot.status,
            pns::presence::PresenceStatus::Room {
                room: "3F - Studio".to_string(),
                age_secs: 0,
            },
            "the line THIS probe set was pointed at is the one that was read"
        );
        assert_eq!(snapshot.now, probes.now_secs(), "and so is the clock");
        assert_eq!(snapshot.desk_idle_secs, Some(9));
    }

    #[test]
    fn the_lamps_narrow_by_the_reading_the_decision_saw_and_never_a_later_poll() {
        // THE PULSE IS THE LAST THING THE EVENT PATH DOES, so a snapshot built
        // down there is built after every channel, the record and the replay.
        // The clock is memoized at the decision and the presence line was not,
        // so a poll the daemon published DURING delivery was read against that
        // older clock and classified `Future`, which is no reading at all and
        // hands the whole house back. MEASURED against the shipped ordering:
        // `Unknown(Future)` where the decision had seen the study.
        let state = scratch("presence-fixed-at-the-decision");
        let line = state.join("presence");
        let now = now_secs().expect("a clock");
        std::fs::write(&line, format!("{now} {now} 1 3F - Studio\n")).expect("a reading");
        let probes = system_probes().with_presence_path(line.to_string_lossy().into_owned());
        let settings = pns::config::Presence {
            rooms: vec!["3F - Studio".to_string(), "2F - Kitchen".to_string()],
            exclude: Vec::new(),
            desk_room: None,
            desk_stale_after_secs: 120,
            poll_secs: 5,
            stale_after_secs: 15,
        };
        // The decision's own clock read, which every age below is judged
        // against, and then the readings the lamps narrow by beside it.
        let at_the_decision = probes.now_secs();
        let taken = presence_snapshot(
            Some(&settings),
            &probes,
            Some(0),
            Some(false),
            pns::home::HomePresence::Unknown,
        )
        .expect("an armed table takes a snapshot");
        // The daemon publishes a poll while the channels are dispatching.
        std::fs::write(&line, format!("{} {} 1 2F - Kitchen\n", now + 5, now + 5))
            .expect("a later poll");
        let studio = pns::presence::PresenceStatus::Room {
            room: "3F - Studio".to_string(),
            age_secs: 0,
        };
        assert_eq!(
            (taken.status, taken.now),
            (studio.clone(), at_the_decision),
            "the snapshot is the decision's own moment, reading and clock together"
        );
        // AND THE PROBE SET CANNOT BE TALKED OUT OF IT AFTERWARDS, which is
        // what makes the ordering above sufficient rather than merely earlier:
        // the line was read once, at the decision, so nothing further down the
        // path can pick the later poll up.
        assert_eq!(
            presence_snapshot(
                Some(&settings),
                &probes,
                Some(0),
                Some(false),
                pns::home::HomePresence::Unknown,
            )
            .map(|later| later.status),
            Some(studio),
            "a read after the channels ran still answers the decision's line"
        );
    }
}
