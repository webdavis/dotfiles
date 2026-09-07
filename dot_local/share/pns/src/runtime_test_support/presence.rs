mod fixtures {
    use crate::*;

    // --- the room sensor's poll ---------------------------------------------

    /// The live `grouped_motion` and `room` shapes, cut to what the join
    /// needs: one watched room with an edge, and the house roll-up that is not
    /// a room.
    pub(crate) const MOTION_BODY: &str = r#"{"data":[
        {"owner":{"rid":"studio","rtype":"room"},"enabled":true,
         "motion":{"motion_report":{"changed":"2026-09-03T17:20:09.413Z","motion":false}}},
        {"owner":{"rid":"house","rtype":"bridge_home"},"enabled":true,
         "motion":{"motion_report":{"changed":"2026-09-03T17:59:59.000Z","motion":true}}}
    ]}"#;

    pub(crate) const ROOM_BODY: &str =
        r#"{"data":[{"id":"studio","metadata":{"name":"3F - Studio"}}]}"#;

    /// A bridge that serves what it was given and nothing else, so a test can
    /// take either listing away.
    pub(crate) struct PollBridge(pub(crate) Vec<(&'static str, &'static str)>);

    impl pns::channels::hue::Bridge for PollBridge {
        fn get(&self, path: &str) -> Option<String> {
            self.0
                .iter()
                .find(|(served, _)| *served == path)
                .map(|(_, body)| (*body).to_string())
        }

        fn put(&self, _path: &str, _body: &str) {
            unreachable!("the poll never writes to the bridge");
        }
    }

    /// The poll's outcome as the one bit most of these tests assert on. The
    /// contention answer is named directly where it is the subject.
    pub(crate) fn poll_published<B: pns::channels::hue::Bridge>(
        bridge: &B,
        state: &std::path::Path,
        presence: &pns::config::Presence,
        now: u64,
    ) -> bool {
        write_presence_reading(bridge, state, presence, now) == Polled::Published
    }

    /// The sensor's settings, watching one room and excluding none.
    pub(crate) fn watching(rooms: &[&str], exclude: &[&str]) -> pns::config::Presence {
        pns::config::Presence {
            rooms: rooms.iter().map(|room| (*room).to_string()).collect(),
            exclude: exclude.iter().map(|room| (*room).to_string()).collect(),
            desk_room: None,
            desk_stale_after_secs: 120,
            poll_secs: 5,
            stale_after_secs: 15,
        }
    }
    /// A snapshot with the desk warm and no room reading of its own, so the
    /// desk's own room is what the narrowing picks.
    pub(crate) fn at_the_desk(
        status: pns::presence::PresenceStatus,
    ) -> pns::presence_policy::Snapshot {
        pns::presence_policy::Snapshot {
            status,
            desk_idle_secs: Some(0),
            screen_locked: Some(false),
            home: pns::home::HomePresence::Unknown,
            desk_room: Some("3F - Studio".to_string()),
            desk_stale_after_secs: 120,
            now: Some(1_700_000_000),
        }
    }

    /// A snapshot with the desk cold and fresh motion in the kitchen.
    pub(crate) fn in_the_kitchen() -> pns::presence_policy::Snapshot {
        pns::presence_policy::Snapshot {
            desk_idle_secs: None,
            ..at_the_desk(pns::presence::PresenceStatus::Room {
                room: "2F - Kitchen".to_string(),
                age_secs: 0,
            })
        }
    }
}

pub(crate) use fixtures::*;
