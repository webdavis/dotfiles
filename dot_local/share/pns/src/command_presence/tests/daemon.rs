mod tests {
    use super::super::*;
    use crate::runtime_test_support::*;

    #[test]
    fn a_poll_the_daemon_launched_stands_down_quietly_and_a_typed_one_does_not() {
        // `Busy` IS NOT ALWAYS TRANSIENT. A poll that was suspended, or
        // orphaned by a daemon that was replaced, holds the lock for as long
        // as it lives, and the daemon relaunches the poll every five seconds.
        // A complaint on that path is a line every five seconds into a log
        // nobody is reading, from a process launchd will neither restart nor
        // alert about. The same stand-down typed by hand is the one case
        // somebody is waiting on an answer for.
        assert_eq!(Polled::Busy.reported(Launch::Daemon), (0, None));
        assert_eq!(Polled::Busy.reported(Launch::Operator).0, 1);
        let complaint = Polled::Busy
            .reported(Launch::Operator)
            .1
            .expect("a typed poll that stood down was told nothing");
        assert!(
            complaint.contains("another poll") && complaint.contains("published nothing"),
            "the complaint does not say what happened: {complaint}"
        );
        // AND NOTHING ELSE CHANGES WITH WHO LAUNCHED IT: publishing and the
        // ordinary refusals are silent and zero either way.
        for launch in [Launch::Daemon, Launch::Operator] {
            assert_eq!(Polled::Published.reported(launch), (0, None));
            assert_eq!(Polled::Nothing.reported(launch), (0, None));
        }
    }

    #[test]
    fn the_poll_argument_parser_tells_the_daemons_launch_from_a_typed_one() {
        // THE FLAG IS THE DAEMON'S OWN SPELLING, and it is what the daemon
        // registers, so the two are read by one parser rather than guessed at
        // from the environment. An unknown word is a refusal, never a silent
        // fallthrough into a poll the operator believes ran differently.
        assert_eq!(presence_launch(&[]), Some(Launch::Operator));
        assert_eq!(
            presence_launch(&[PRESENCE_DAEMON_FLAG.to_string()]),
            Some(Launch::Daemon)
        );
        for arguments in [
            vec!["--dameon".to_string()],
            vec!["--daemon=1".to_string()],
            vec![String::new()],
            vec![
                PRESENCE_DAEMON_FLAG.to_string(),
                PRESENCE_DAEMON_FLAG.to_string(),
            ],
        ] {
            assert_eq!(presence_launch(&arguments), None, "{arguments:?} was read");
        }
    }

    #[test]
    fn an_armed_sensor_registers_the_poll_at_its_own_interval() {
        let state = scratch("presence-register");
        let presence = pns::config::Presence {
            rooms: vec!["3F - Studio".to_string()],
            exclude: Vec::new(),
            desk_room: None,
            desk_stale_after_secs: 120,
            poll_secs: 7,
            stale_after_secs: 21,
        };

        ensure_presence_poll(&state, Some(&presence), 1000);

        let record = std::fs::read_to_string(pns::daemon::spool_dir(&state).join("presence"))
            .expect("the registered job");
        let job = pns::daemon::parse(record.trim()).expect("a job record");
        assert_eq!(job.id, "presence");
        // THE FLAG THE DAEMON ALONE PASSES, so the poll it launches knows
        // nobody is reading its stderr.
        assert_eq!(
            job.args,
            vec![
                "presence".to_string(),
                "poll".to_string(),
                PRESENCE_DAEMON_FLAG.to_string()
            ]
        );
        assert_eq!(job.every, Some(7));
        // DUE NOW, so the reading arrives on the next tick rather than one
        // interval after the switch went on, and leased past it.
        assert_eq!(job.due, 1000);
        assert_eq!(job.until, 1300);
    }

    #[test]
    fn a_sensor_that_is_off_cancels_the_poll_it_had_registered() {
        // OFF, ABSENT AND REFUSED ARRIVE HERE AS ONE `None`, so the operator's
        // switch and a typo in the table both stop the bridge reads within one
        // sweep.
        let state = scratch("presence-cancel");
        let presence = pns::config::Presence {
            rooms: vec!["3F - Studio".to_string()],
            exclude: Vec::new(),
            desk_room: None,
            desk_stale_after_secs: 120,
            poll_secs: 5,
            stale_after_secs: 15,
        };
        ensure_presence_poll(&state, Some(&presence), 1000);
        let record = pns::daemon::spool_dir(&state).join("presence");
        assert!(record.exists(), "the fixture never registered anything");

        ensure_presence_poll(&state, None, 1030);

        assert!(!record.exists(), "the poll outlived its own table");
    }

    #[test]
    fn a_sweep_refreshes_the_lease_without_moving_a_poll_that_is_already_due() {
        // The sweep runs every thirty seconds and the poll every five, so a
        // sweep that re-armed `due` would keep pushing the reading away from
        // itself and the sensor would never report at all.
        let state = scratch("presence-lease");
        let presence = pns::config::Presence {
            rooms: vec!["3F - Studio".to_string()],
            exclude: Vec::new(),
            desk_room: None,
            desk_stale_after_secs: 120,
            poll_secs: 5,
            stale_after_secs: 15,
        };
        ensure_presence_poll(&state, Some(&presence), 1000);
        // As the daemon leaves it after firing once: due again five seconds on.
        let record = pns::daemon::spool_dir(&state).join("presence");
        let fired = pns::daemon::Job {
            due: 1005,
            ..pns::daemon::parse(
                std::fs::read_to_string(&record)
                    .expect("the registered job")
                    .trim(),
            )
            .expect("a job record")
        };
        std::fs::write(&record, format!("{}\n", pns::daemon::render(&fired))).expect("the rearm");

        ensure_presence_poll(&state, Some(&presence), 1002);

        let job = pns::daemon::parse(
            std::fs::read_to_string(&record)
                .expect("the registered job")
                .trim(),
        )
        .expect("a job record");
        assert_eq!(job.due, 1005, "the pending due moved");
        assert_eq!(job.until, 1302, "the lease was not refreshed");
    }
}
