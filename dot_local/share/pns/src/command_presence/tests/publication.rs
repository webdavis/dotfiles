mod tests {
    use crate::runtime_test_support::*;

    #[test]
    fn a_poll_publishes_the_room_it_read_as_the_line_the_sensor_parses() {
        // THE WHOLE EDGE, END TO END: the bridge, the join, the render and the
        // state file, which is the join every other test in this feature
        // stubs out. Replacing this function's body with a constant leaves it
        // red.
        let state = scratch("presence-poll");
        let bridge = PollBridge(vec![("grouped_motion", MOTION_BODY), ("room", ROOM_BODY)]);

        assert!(poll_published(
            &bridge,
            &state,
            &watching(&["3F - Studio"], &[]),
            1_788_456_100
        ));

        let published = std::fs::read_to_string(state.join(pns::presence_file::STATE_FILE))
            .expect("the state file");
        assert_eq!(published, "1788456100 1788456009 0 3F - Studio\n");
        // AND THE READER TAKES IT BACK, which is the property the whole file
        // format exists for: rendering a line no parse accepts would publish a
        // dead sensor rather than a reading.
        assert_eq!(
            pns::presence_file::parse_presence_line(&published),
            Some(pns::presence_file::RawPresence {
                poll_epoch: 1_788_456_100,
                edge: Some(pns::presence_file::Edge {
                    epoch: 1_788_456_009,
                    motion: false,
                    room: "3F - Studio".to_string(),
                }),
            })
        );
        assert_eq!(
            published_mode(&state.join(pns::presence_file::STATE_FILE)),
            0o600
        );
    }

    #[test]
    fn a_bridge_that_did_not_answer_leaves_the_last_reading_where_it_was() {
        // THE FAIL-CLOSED HALF, and the one that matters most: a poll that
        // wrote its own epoch here would keep a room fresh forever on a bridge
        // that has stopped answering. Left alone, the line ages out to
        // Unknown.
        let state = scratch("presence-silent");
        let reading = state.join(pns::presence_file::STATE_FILE);
        std::fs::write(&reading, "1788456000 1788455900 1 3F - Studio\n").expect("the old line");

        for served in [
            vec![],
            vec![("grouped_motion", MOTION_BODY)],
            vec![("room", ROOM_BODY)],
        ] {
            assert!(!poll_published(
                &PollBridge(served),
                &state,
                &watching(&["3F - Studio"], &[]),
                1_788_456_100
            ));
            assert_eq!(
                std::fs::read_to_string(&reading).expect("the state file"),
                "1788456000 1788455900 1 3F - Studio\n",
                "a silent bridge published a reading anyway"
            );
        }
    }

    #[test]
    fn a_room_this_cannot_spell_leaves_the_last_reading_where_it_was() {
        // THE SECOND FAIL-CLOSED HALF, beside the silent bridge: a room name
        // the reader would refuse used to publish the POLL-ONLY line, which is
        // a different reading rather than a smaller one. The doctor printed
        // `nowhere` while the bridge was reporting motion in a watched room.
        let state = scratch("presence-unspellable");
        let reading = state.join(pns::presence_file::STATE_FILE);
        std::fs::write(&reading, "1788456000 1788455900 1 3F - Studio\n").expect("the old line");
        // The bridge's own text, carrying a tab: real names cross this parse
        // verbatim, so this is the room the operator would have configured.
        let rooms = r#"{"data":[{"id":"studio","metadata":{"name":"3F\tStudio"}}]}"#;

        assert!(!poll_published(
            &PollBridge(vec![("grouped_motion", MOTION_BODY), ("room", rooms)]),
            &state,
            &watching(&["3F\tStudio"], &[]),
            1_788_456_100
        ));
        assert_eq!(
            std::fs::read_to_string(&reading).expect("the state file"),
            "1788456000 1788455900 1 3F - Studio\n",
            "a room the reader would refuse was published as a nowhere"
        );
    }

    /// Two watched rooms report, the pass-through one more recently.
    const TWO_ROOM_MOTION: &str = r#"{"data":[
        {"owner":{"rid":"studio","rtype":"room"},
         "motion":{"motion_report":{"changed":"2026-09-03T17:20:09.413Z","motion":false}}},
        {"owner":{"rid":"hallway","rtype":"room"},
         "motion":{"motion_report":{"changed":"2026-09-03T17:25:00.000Z","motion":true}}}
    ]}"#;

    const TWO_ROOM_ROOMS: &str = r#"{"data":[
        {"id":"studio","metadata":{"name":"3F - Studio"}},
        {"id":"hallway","metadata":{"name":"3F - Hallway"}}
    ]}"#;

    #[test]
    fn an_excluded_room_yields_to_the_newest_room_that_counts() {
        // THE EXCLUSION HAS TO HAPPEN BEFORE THE COMPARISON. `exclude` is
        // documented for "a room you pass through", and a pass-through room
        // holds the newest edge nearly every poll. Publishing it and leaving
        // `classify` to refuse it discards the studio the operator is sitting
        // in and answers Unknown, so the key would blind the sensor exactly
        // when it is doing its job.
        let state = scratch("presence-exclude");
        let settings = watching(&["3F - Studio", "3F - Hallway"], &["3F - Hallway"]);
        let bridge = PollBridge(vec![
            ("grouped_motion", TWO_ROOM_MOTION),
            ("room", TWO_ROOM_ROOMS),
        ]);

        assert!(poll_published(&bridge, &state, &settings, 1_788_456_400));

        let published = std::fs::read_to_string(state.join(pns::presence_file::STATE_FILE))
            .expect("the state file");
        assert_eq!(published, "1788456400 1788456009 0 3F - Studio\n");
        // AND THE READER AGREES, which is the whole point: the same settings
        // that name the exclusion now judge the line the writer chose.
        assert_eq!(
            pns::presence::classify(
                pns::presence_file::parse_presence_line(&published),
                Some(1_788_456_400),
                settings.stale_after_secs,
                &settings.rooms,
                &settings.exclude,
            ),
            pns::presence::PresenceStatus::Room {
                room: "3F - Studio".to_string(),
                age_secs: 391,
            }
        );
    }
}
