mod tests {
    use super::super::*;
    use crate::runtime_test_support::*;

    /// A bridge whose motion read PARKS until it is let go, so a second poller
    /// can be driven while the first is still mid-poll.
    struct ParkedBridge {
        entered: std::sync::mpsc::Sender<()>,
        release: std::sync::mpsc::Receiver<()>,
        served: Vec<(&'static str, &'static str)>,
    }

    impl pns::channels::hue::Bridge for ParkedBridge {
        fn get(&self, path: &str) -> Option<String> {
            if path == "grouped_motion" {
                let _ = self.entered.send(());
                // BOUNDED, AND NEVER WAITED ON BY A GREEN RUN: the release
                // arrives the moment the second poller has answered, so this
                // deadline is only ever reached by a run that already failed.
                let _ = self.release.recv_timeout(Duration::from_secs(5));
            }
            self.served
                .iter()
                .find(|(served, _)| *served == path)
                .map(|(_, body)| (*body).to_string())
        }

        fn put(&self, _path: &str, _body: &str) {
            unreachable!("the poll never writes to the bridge");
        }
    }

    #[test]
    fn a_poll_already_running_stands_a_second_one_down_rather_than_racing_it() {
        // THE LOCK IS HELD ACROSS BOTH READS AND THE PUBLICATION, which is
        // what stops two processes publishing out of order: a stalled poller
        // that finishes late would otherwise rename its older reading over a
        // newer one and make it current. The second poller here is NEWER and
        // must still publish nothing while the first holds the poll.
        let state = scratch("presence-race");
        let (entered, entry) = std::sync::mpsc::channel();
        let (release, held) = std::sync::mpsc::channel();
        let (outcome, finished) = std::sync::mpsc::channel();

        // DETACHED, NEVER JOINED: a regression that parks the first poller
        // fails this test on a deadline instead of hanging the suite on a
        // join.
        let running = state.clone();
        std::thread::spawn(move || {
            let bridge = ParkedBridge {
                entered,
                release: held,
                served: vec![("grouped_motion", MOTION_BODY), ("room", ROOM_BODY)],
            };
            let published = poll_published(
                &bridge,
                &running,
                &watching(&["3F - Studio"], &[]),
                1_788_456_100,
            );
            let _ = outcome.send(published);
        });
        entry
            .recv_timeout(Duration::from_secs(5))
            .expect("the first poller reaches the bridge");

        // NAMED AS CONTENTION rather than as "published nothing": a poll that
        // stood down for a live holder is the one refusal a hand-typed poll is
        // told about, so a regression that turned it into an ordinary silent
        // refusal would pass a weaker assertion.
        assert_eq!(
            write_presence_reading(
                &PollBridge(vec![("grouped_motion", MOTION_BODY), ("room", ROOM_BODY)]),
                &state,
                &watching(&["3F - Studio"], &[]),
                1_788_456_200
            ),
            Polled::Busy,
            "a second poller was let into a poll the first was holding"
        );

        let _ = release.send(());
        assert_eq!(
            finished.recv_timeout(Duration::from_secs(5)),
            Ok(true),
            "the poller holding the lock published nothing"
        );
        assert_eq!(
            std::fs::read_to_string(state.join(pns::presence_file::STATE_FILE))
                .expect("the state file"),
            "1788456100 1788456009 0 3F - Studio\n",
            "the published line is not the one the lock holder wrote"
        );
    }

    #[test]
    fn the_lock_a_killed_poller_left_behind_does_not_stand_the_next_poll_down() {
        // A POLLER KILLED MID-POLL RUNS NO `Drop`, and the daemon kills every
        // child that outlives its bound, so the file it leaves behind is the
        // ORDINARY end of a wedged poll rather than a rarity. Nothing is
        // inside the poll once that process is gone, so the next interval has
        // to publish: a lock believed until its own mtime ages out instead
        // blinds the sensor for a whole stale window, and the reading it never
        // refreshed goes Unknown.
        let state = scratch("presence-killed-holder");
        std::fs::write(state.join(pns::presence_lock::LOCK_FILE), b"").expect("the leavings");

        assert!(
            poll_published(
                &PollBridge(vec![("grouped_motion", MOTION_BODY), ("room", ROOM_BODY)]),
                &state,
                &watching(&["3F - Studio"], &[]),
                1_788_456_100
            ),
            "the lock a killed poller left behind stood the next poll down"
        );
    }

    #[test]
    fn a_poll_gives_its_lock_back_so_the_next_interval_can_take_it() {
        // THE GUARD IS WHAT MAKES THE LOCK A LOCK RATHER THAN A LATCH: a hold
        // left behind would stand every later poll down for a whole stale
        // window, which is a sensor that answers once and then goes quiet.
        let state = scratch("presence-relock");
        let settings = watching(&["3F - Studio"], &[]);
        for now in [1_788_456_100, 1_788_456_105] {
            // THE OUTCOME ITSELF, not just whether it published: `Busy` says
            // the lock was never given back and `Nothing` says the publish
            // failed for a reason that has nothing to do with the lock, and a
            // bare boolean cannot tell the two apart from a failure line.
            //
            // AND RETRIED FOR A MOMENT, which is a fact about this BINARY
            // rather than about the lock: other tests here spawn subprocesses,
            // `fork` duplicates every open descriptor, and an inherited copy of
            // this lock's descriptor holds the lock until the child's `exec`
            // closes it. Measured at one 5ms tick, at about one run in ten. A
            // poll opens no such window, spawning nothing at all while it
            // holds the lock, and the retry is what an interval would be
            // anyway. A lock that is never given back still fails this: the
            // deadline runs out and the last outcome is the one asserted.
            let poll = || {
                write_presence_reading(
                    &PollBridge(vec![("grouped_motion", MOTION_BODY), ("room", ROOM_BODY)]),
                    &state,
                    &settings,
                    now,
                )
            };
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            let mut outcome = poll();
            while outcome == Polled::Busy && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
                outcome = poll();
            }
            assert_eq!(
                outcome,
                Polled::Published,
                "the poll at {now} published nothing"
            );
        }
        assert_eq!(
            std::fs::read_to_string(state.join(pns::presence_file::STATE_FILE))
                .expect("the state file"),
            "1788456105 1788456009 0 3F - Studio\n"
        );
    }
}
