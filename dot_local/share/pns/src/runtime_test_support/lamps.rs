mod fixtures {
    use crate::*;
    use std::cell::RefCell;

    /// The bridge the tick's writes are driven against: three listings,
    /// answered or not, and every PUT recorded IN ORDER.
    ///
    /// A SEQUENCE RATHER THAN A SET, because the order is the whole question
    /// here: an arm followed by an off is a lamp the tick put out after telling
    /// it to breathe, and a set cannot tell that from a lamp that was only ever
    /// armed.
    pub(crate) struct ScriptedBridge {
        listings: Option<()>,
        pub(crate) puts: RefCell<Vec<(String, String)>>,
    }

    impl pns::channels::hue::Bridge for ScriptedBridge {
        fn get(&self, path: &str) -> Option<String> {
            self.listings?;
            Some(
                match path {
                    "light" => ONE_LAMP,
                    "zone" => r#"{"data":[]}"#,
                    _ => ONE_ROOM,
                }
                .to_string(),
            )
        }
        fn put(&self, path: &str, body: &str) {
            self.puts
                .borrow_mut()
                .push((path.to_string(), body.to_string()));
        }
    }

    /// The clock a tick test hands the driver when it asserts on WHAT was
    /// written and to which lamp rather than on when: nothing takes any time,
    /// so every fade the schedule holds is issued and none is dropped at the
    /// budget. A test that asserts a PHASE hands over a `FakeClock` instead,
    /// because a phase is a moment and this clock has none.
    pub(crate) fn no_time_passes() -> impl FnMut() -> u64 {
        || 0
    }

    /// A monotonic clock and its sleeper over one cell. THE SLEEPER IS THE ONLY
    /// THING THAT ADVANCES IT, so a whole tick plays out at the milliseconds its
    /// own schedule names, with no wall clock in the test at all.
    #[derive(Default)]
    pub(crate) struct FakeClock(std::cell::Cell<u64>);

    impl FakeClock {
        pub(crate) fn elapsed_ms(&self) -> u64 {
            self.0.get()
        }

        pub(crate) fn slept(&self, waited: Duration) {
            self.0
                .set(self.0.get() + u64::try_from(waited.as_millis()).unwrap_or(0));
        }
    }

    /// A bridge whose calls cost the tick real time on the tick's own clock,
    /// which is what a slow LAN does to a synchronous schedule. The two costs
    /// are separate because they buy different failures: a slow resolve eats
    /// the budget before a single fade is issued, and a slow write pushes every
    /// later fade past the moment it was due.
    pub(crate) struct SlowBridge<'a> {
        pub(crate) clock: &'a FakeClock,
        pub(crate) get_cost_ms: u64,
        pub(crate) put_cost_ms: u64,
        pub(crate) answers: bool,
        pub(crate) puts: RefCell<Vec<(String, String)>>,
    }

    impl pns::channels::hue::Bridge for SlowBridge<'_> {
        fn get(&self, path: &str) -> Option<String> {
            self.clock.slept(Duration::from_millis(self.get_cost_ms));
            self.answers.then(|| {
                match path {
                    "light" => ONE_LAMP,
                    "zone" => r#"{"data":[]}"#,
                    _ => ONE_ROOM,
                }
                .to_string()
            })
        }
        fn put(&self, path: &str, body: &str) {
            self.puts
                .borrow_mut()
                .push((path.to_string(), body.to_string()));
            self.clock.slept(Duration::from_millis(self.put_cost_ms));
        }
    }

    pub(crate) fn scripted(answers: bool) -> ScriptedBridge {
        ScriptedBridge {
            listings: answers.then_some(()),
            puts: RefCell::new(Vec::new()),
        }
    }

    const ONE_ROOM: &str = r#"{"data":[
      {"id":"r1","type":"room","metadata":{"name":"3F - Studio"},
       "children":[{"rid":"dev-1","rtype":"device"}],
       "services":[{"rid":"g1","rtype":"grouped_light"}]}
    ]}"#;

    const ONE_LAMP: &str = r#"{"data":[
      {"id":"l1","type":"light","owner":{"rid":"dev-1","rtype":"device"},
       "metadata":{"name":"3F - Studio - HCL1"}}
    ]}"#;

    pub(crate) const LAMP_PATH: &str = "light/l1";
    pub(crate) const CLEAR_BODY: &str = r#"{"on":{"on":false}}"#;

    /// A room routed for every held state, which is the map these tick tests
    /// resolve through.
    pub(crate) fn held_lights() -> pns::config::Lights {
        *pns::config::parse_config(
            "[lights]\nrefresh_secs = 10\n\
             [lights.room.\"3F - Studio\"]\nshows = [\"blocked\", \"unread\", \"loop\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table")
    }

    /// The clock and the mutes a tick that is testing something else is judged
    /// against: noon, and nothing muted.
    pub(crate) fn noon(muted: &pns::channels::hue::Muting) -> pns::channels::hue::Reading<'_> {
        pns::channels::hue::Reading {
            minutes_now: Some(12 * 60),
            muted,
        }
    }

    /// The ordinary mute: a machine that has never typed the command.
    pub(crate) fn nothing_muted() -> pns::channels::hue::Muting {
        pns::channels::hue::Muting::Places(Vec::new())
    }

    /// One place the operator quieted by hand.
    pub(crate) fn quieted(place: &str) -> pns::channels::hue::Muting {
        pns::channels::hue::Muting::Places(vec![place.to_string()])
    }

    /// What the held record says right now.
    pub(crate) fn recorded(state: &std::path::Path) -> Option<String> {
        std::fs::read_to_string(state.join(LIGHTS_HELD))
            .ok()
            .map(|line| line.trim().to_string())
    }
    /// A bridge holding two rooms with one lamp each, which is the smallest
    /// listing a narrowing can be observed against: with one room, keeping the
    /// room and keeping everything are the same answer.
    pub(crate) struct TwoRoomBridge {
        pub(crate) puts: RefCell<Vec<(String, String)>>,
    }

    impl pns::channels::hue::Bridge for TwoRoomBridge {
        fn get(&self, path: &str) -> Option<String> {
            Some(
                match path {
                    "light" => {
                        r#"{"data":[
                      {"id":"l1","type":"light","owner":{"rid":"dev-1","rtype":"device"},
                       "metadata":{"name":"3F - Studio - HCL1"}},
                      {"id":"l2","type":"light","owner":{"rid":"dev-2","rtype":"device"},
                       "metadata":{"name":"2F - Kitchen - HCD6"}}
                    ]}"#
                    }
                    "zone" => r#"{"data":[]}"#,
                    _ => {
                        r#"{"data":[
                      {"id":"r1","type":"room","metadata":{"name":"3F - Studio"},
                       "children":[{"rid":"dev-1","rtype":"device"}],
                       "services":[{"rid":"g1","rtype":"grouped_light"}]},
                      {"id":"r2","type":"room","metadata":{"name":"2F - Kitchen"},
                       "children":[{"rid":"dev-2","rtype":"device"}],
                       "services":[{"rid":"g2","rtype":"grouped_light"}]}
                    ]}"#
                    }
                }
                .to_string(),
            )
        }
        fn put(&self, path: &str, body: &str) {
            self.puts
                .borrow_mut()
                .push((path.to_string(), body.to_string()));
        }
    }
}

pub(crate) use fixtures::*;
