mod tests {
    use super::super::*;
    use crate::runtime_test_support::*;
    use std::cell::RefCell;

    #[test]
    fn two_breathing_lamps_share_one_schedule_rather_than_running_back_to_back() {
        // ONE SLEEP SCHEDULE FOR EVERY LAMP, in due order ACROSS lamps. Issued
        // per lamp instead, every fade of the second lamp would be past due by
        // the time the first lamp's breath ended: all issued at once, late, a
        // jump rather than a breath.
        let bridge = scripted(true);
        // TWO SHAPES THIS TEST OWNS, written out rather than read from
        // `Lights::default()`. They equal the locked blocked and loop shapes as
        // it happens, but the interleave asserted below is the exact due-order
        // these two durations produce, so reading them from the defaults would
        // rewrite the expected order every time a cadence is retuned and this
        // test would start failing for a reason it is not about. Leave these
        // alone when a cadence change sends you grepping for a duration.
        let quick = pns::config::Breath {
            duration_ms: 2000,
            high: 100,
            low: 30,
        };
        let slow = pns::config::Breath {
            duration_ms: 4000,
            high: 60,
            low: 10,
        };
        drive_breaths(
            &bridge,
            12_000,
            &[
                Breathing {
                    path: "light/a".to_string(),
                    held: pns::lights::Held::Blocked,
                    cycle: pns::lights::breath_cycle(&quick),
                    color: pns::pulse::BLOCKED_COLOR,
                    resume: pns::lights::Resume::default(),
                },
                Breathing {
                    path: "light/b".to_string(),
                    held: pns::lights::Held::Looping,
                    cycle: pns::lights::breath_cycle(&slow),
                    color: pns::pulse::LOOP_COLOR,
                    resume: pns::lights::Resume::default(),
                },
            ],
            no_time_passes(),
            |_| {},
        );
        let order: Vec<String> = bridge
            .puts
            .borrow()
            .iter()
            .map(|(path, _)| path.clone())
            .collect();
        assert_eq!(
            order,
            [
                "light/a", "light/b", "light/a", "light/a", "light/b", "light/a", "light/a",
                "light/b", "light/a", "light/a", "light/b",
            ],
            "the fades interleave by their due milliseconds, not by lamp: the quick \
             shape's seven fades and the slow shape's four, seamless past the old \
             stop-at-the-peak count"
        );
    }

    #[test]
    fn a_slow_write_stops_the_schedule_at_the_budget_and_lands_where_it_really_did() {
        // THE SCHEDULE IS NOMINAL AND THE WRITES ARE NOT. Writes are
        // synchronous and sequential, so a lamp answering slowly pushes every
        // later fade past the moment it was due, and the locked blocked shape's
        // seventh fade would be issued three seconds AFTER the budget it
        // belongs to. Two things follow, and both are asserted here: nothing is
        // issued at or past the budget, and the phase left for the next tick is
        // the end of a write that ACTUALLY HAPPENED, timed from when it
        // actually started.
        let clock = FakeClock::default();
        let bridge = SlowBridge {
            clock: &clock,
            get_cost_ms: 0,
            put_cost_ms: 3_000,
            answers: true,
            puts: RefCell::new(Vec::new()),
        };
        let landings = drive_breaths(
            &bridge,
            12_000,
            &[Breathing {
                path: "light/a".to_string(),
                held: pns::lights::Held::Blocked,
                cycle: pns::lights::breath_cycle(&pns::config::Breath {
                    duration_ms: 2_000,
                    high: 100,
                    low: 30,
                }),
                color: pns::pulse::BLOCKED_COLOR,
                resume: pns::lights::Resume::default(),
            }],
            || clock.elapsed_ms(),
            |waited| clock.slept(waited),
        );
        assert_eq!(
            bridge.puts.borrow().len(),
            4,
            "four writes at three seconds apiece fill a twelve-second budget, and \
             the fifth would be issued AT the budget, so it is not issued at all"
        );
        assert_eq!(
            landings,
            vec![("light/a".to_string(), 100, 11_000)],
            "the last write really happened at 9,000ms and its fade runs 2,000ms \
             from there, so the next tick resumes off 11,000ms rather than off the \
             13,700ms the nominal schedule would have claimed"
        );
    }

    #[test]
    fn a_landing_is_timed_by_the_fade_that_ran_and_not_by_the_cycles_first_leg() {
        // THE ONE CYCLE WHOSE LEGS DO NOT SHARE A DURATION. Every other shape
        // fades at one cadence throughout, so a landing timed from the shape
        // and one timed from the fade agree and neither can be told from the
        // other. An accent is what separates them: timed from the shape, the
        // flash would claim to finish a whole fade out instead of at its own
        // brief duration, and `resume_from` reads a landing that far past the
        // accent's own step as a clock that moved. The next tick would throw
        // the phase away and restart the breath rather than falling out of the
        // flash.
        //
        // THE MOTION IS WRITTEN OUT rather than read from `Lights::default()`,
        // for the reason the interleave test above states: the milliseconds
        // asserted here are the exact arithmetic these durations produce, and
        // reading them from the defaults would rewrite them on every retune.
        let clock = FakeClock::default();
        let bridge = SlowBridge {
            clock: &clock,
            get_cost_ms: 0,
            put_cost_ms: 0,
            answers: true,
            puts: RefCell::new(Vec::new()),
        };
        // A BUDGET THAT ENDS ON THE ACCENT: the fall after it is due at
        // 4,350ms, so 4,300ms leaves the flash as the last fade issued.
        let landings = drive_breaths(
            &bridge,
            4_300,
            &[Breathing {
                path: "light/a".to_string(),
                held: pns::lights::Held::Looping,
                cycle: pns::lights::breathe_then_flare_cycle(&pns::config::BreatheThenFlare {
                    breath: pns::config::Breath {
                        duration_ms: 2_000,
                        high: 80,
                        low: 30,
                    },
                    flare: 100,
                    flare_ms: 500,
                }),
                color: pns::pulse::LOOP_COLOR,
                resume: pns::lights::Resume::default(),
            }],
            || clock.elapsed_ms(),
            |waited| clock.slept(waited),
        );
        assert_eq!(
            landings,
            vec![("light/a".to_string(), 100, 4_400)],
            "the flash is issued at 3,900ms and runs its OWN 500ms, so it lands at \
             4,400ms; timed from the cycle's first leg it would claim 5,900ms, which \
             is further out than the accent's own step and so is read as stale"
        );
    }
}
