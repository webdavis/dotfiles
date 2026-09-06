//! The lamps, pinned: phase.

use super::fixtures::*;

// --- the held record's phase --------------------------------------------

#[test]
fn a_held_entrys_phase_round_trips_through_its_rendered_token() {
    let high = HeldEntry {
        path: "light/l1".to_string(),
        resume: Some(Phase {
            end_unix_ms: 1_234_567,
            landed_on: 100,
            held: Held::Blocked,
        }),
    };
    assert_eq!(render_held_token(&high), "light/l1@1234567:100:blocked");
    assert_eq!(parse_held_token("light/l1@1234567:100:blocked"), high);

    let low = HeldEntry {
        path: "light/l1".to_string(),
        resume: Some(Phase {
            end_unix_ms: 1_234_567,
            landed_on: 10,
            held: Held::Looping,
        }),
    };
    assert_eq!(render_held_token(&low), "light/l1@1234567:10:loop");
    assert_eq!(parse_held_token("light/l1@1234567:10:loop"), low);

    // AND THE ACCENT'S OWN LEVEL, which is the one a two-valued record
    // could not carry: the loop lamp landing on its flash is neither the
    // high end nor the low one.
    let flare = HeldEntry {
        path: "light/l1".to_string(),
        resume: Some(Phase {
            end_unix_ms: 1_234_567,
            landed_on: LOOP_MOTION.flare,
            held: Held::Looping,
        }),
    };
    assert_eq!(render_held_token(&flare), "light/l1@1234567:100:loop");
    assert_eq!(parse_held_token("light/l1@1234567:100:loop"), flare);
}

#[test]
fn every_held_state_has_its_own_record_word_and_reads_back_as_itself() {
    // FOUR STATES, FOUR WORDS, and the two unread flavours are not one:
    // they share a routable word and NOT a colour, so a red failure that
    // inherited a green success's phase would delay the red by a fade.
    let states = [
        Held::Blocked,
        Held::Looping,
        Held::UnreadFailure,
        Held::UnreadSuccess,
    ];
    let mut words: Vec<&str> = states.iter().map(|held| held.word()).collect();
    words.sort_unstable();
    words.dedup();
    assert_eq!(words.len(), states.len(), "two states share one word");
    for held in states {
        assert_eq!(Held::from_word(held.word()), Some(held));
    }
    assert_eq!(
        Held::from_word("dozing"),
        None,
        "a word this build does not know is no phase, never a wrong one"
    );
}

#[test]
fn a_bare_token_reads_as_no_phase_and_a_malformed_one_falls_back_to_bare() {
    assert_eq!(
        parse_held_token("light/l1"),
        HeldEntry::bare("light/l1"),
        "a token with no `@` at all is a lamp with no phase recorded"
    );
    for malformed in [
        "light/l1@notanumber:100:blocked",
        "light/l1@1234567:sideways:blocked",
        "light/l1@1234567:900:blocked",
        "light/l1@1234567:100:dozing",
        "light/l1@1234567:100",
        "light/l1@1234567",
        "light/l1@",
    ] {
        assert_eq!(
            parse_held_token(malformed),
            HeldEntry::bare("light/l1"),
            "{malformed} is unreadable, never unparseable: it reads as no phase"
        );
    }
}

#[test]
fn resuming_off_no_entry_or_no_phase_starts_the_breath_fresh() {
    assert_eq!(
        resume_from(None, 1_000, Held::Blocked, &breath_cycle(&BLOCKED)),
        Resume::default()
    );
    assert_eq!(
        resume_from(
            Some(&HeldEntry::bare("light/l1")),
            1_000,
            Held::Blocked,
            &breath_cycle(&BLOCKED)
        ),
        Resume::default(),
        "a bare entry is a lamp this record holds with no phase recorded"
    );
}

#[test]
fn a_phase_another_state_left_behind_is_started_over_rather_than_resumed() {
    // THE PAUSE A FIXTURE-ONLY RESUME COSTS. The slow shapes land their
    // last fade almost four seconds past the interval that issued it, so a
    // lamp that was looping and is now blocked would wait that fade out
    // before its first blocked body reached the bridge: the locked precedence
    // arriving up to a whole fade late.
    let looping = HeldEntry {
        path: "light/l1".to_string(),
        resume: Some(Phase {
            end_unix_ms: 15_850,
            landed_on: SLOW.high,
            held: Held::Looping,
        }),
    };
    assert_eq!(
        resume_from(
            Some(&looping),
            12_400,
            Held::Blocked,
            &breath_cycle(&BLOCKED)
        ),
        Resume::default(),
        "a state change starts down at once instead of finishing the shape it \
         is replacing"
    );
    assert_eq!(
        resume_from(Some(&looping), 12_400, Held::Looping, &breath_cycle(&SLOW)),
        Resume {
            first_due_ms: 3_400,
            next_leg: 0
        },
        "and the same state still picks its own breath back up"
    );
    // THE TWO UNREAD FLAVOURS ARE TWO STATES, because they share a routable
    // word and NOT a colour: an unread lamp turning red is exactly the case
    // the ruling names.
    let success = HeldEntry {
        path: "light/l1".to_string(),
        resume: Some(Phase {
            end_unix_ms: 15_850,
            landed_on: SLOW.high,
            held: Held::UnreadSuccess,
        }),
    };
    assert_eq!(
        resume_from(
            Some(&success),
            12_400,
            Held::UnreadFailure,
            &breath_cycle(&SLOW)
        ),
        Resume::default()
    );
}

#[test]
fn a_phase_sitting_further_ahead_than_one_step_reads_as_stale() {
    // A CLOCK THAT WENT BACKWARDS. `now_ms` is wall time, and a phase is
    // recorded in wall time too, so an hour lost to a time-zone edit, an
    // NTP correction or a resumed sleep leaves a perfectly valid record
    // looking like a fade due an hour from now. That schedule starts past
    // the budget, issues nothing at all, and the lamp holds one whole
    // interval: exactly the pause this slice exists to remove.
    //
    // ONE CADENCE STEP IS THE CEILING, and it is a law rather than a
    // tolerance: the previous tick issued its last fade strictly inside its
    // own budget, that fade lands one duration later, and the next tick
    // begins at most the daemon's slop after that budget ended. So a live
    // phase is never due more than a step ahead.
    let end_unix_ms = 1_700_000_000_000;
    let held = HeldEntry {
        path: "light/l1".to_string(),
        resume: Some(Phase {
            end_unix_ms,
            landed_on: BLOCKED.low,
            held: Held::Blocked,
        }),
    };
    let step_ms = BLOCKED.duration_ms - FADE_LEAD_MS;
    assert_eq!(
        resume_from(
            Some(&held),
            end_unix_ms - FADE_LEAD_MS - step_ms,
            Held::Blocked,
            &breath_cycle(&BLOCKED)
        ),
        Resume {
            first_due_ms: step_ms,
            next_leg: 1
        },
        "a phase due exactly one step out is the furthest a live one reaches, \
         and it is still resumed"
    );
    assert_eq!(
        resume_from(
            Some(&held),
            end_unix_ms - FADE_LEAD_MS - step_ms - 1,
            Held::Blocked,
            &breath_cycle(&BLOCKED)
        ),
        Resume::default(),
        "one millisecond further out than any tick could have left it is a \
         clock that moved, so the breath starts over at once"
    );
    assert_eq!(
        resume_from(
            Some(&held),
            end_unix_ms - 3_600_000,
            Held::Blocked,
            &breath_cycle(&BLOCKED)
        ),
        Resume::default(),
        "and an hour lost to a clock correction is the case that costs a whole \
         interval of a still lamp"
    );
}

#[test]
fn resuming_off_a_recorded_phase_shifts_the_next_fade_and_takes_the_next_leg() {
    let held = HeldEntry {
        path: "light/l1".to_string(),
        resume: Some(Phase {
            end_unix_ms: 13_700,
            landed_on: BLOCKED.low,
            held: Held::Blocked,
        }),
    };
    assert_eq!(
        resume_from(Some(&held), 12_400, Held::Blocked, &breath_cycle(&BLOCKED)),
        Resume {
            first_due_ms: 1_250,
            next_leg: 1
        },
        "due FADE_LEAD_MS before the recorded end, moving away from the end it \
         landed on"
    );
    // A `now_ms` past the recorded end saturates at zero rather than going
    // negative: due at once, not due in the past.
    assert_eq!(
        resume_from(Some(&held), 20_000, Held::Blocked, &breath_cycle(&BLOCKED)),
        Resume {
            first_due_ms: 0,
            next_leg: 1
        }
    );
}

#[test]
fn a_blocked_event_starts_a_wait_and_every_other_event_ends_one() {
    for waiting in crate::pulse::LAMP_BLOCKED {
        assert_eq!(
            blocked_marker_action(waiting),
            Action::Start,
            "{waiting} is an agent waiting on the operator"
        );
    }
    for ended in ["done", "failed", "stale", "", "anything-else"] {
        assert_eq!(
            blocked_marker_action(ended),
            Action::End,
            "{ended} is a later event from that session, so the wait is over"
        );
    }
}

#[test]
fn a_session_id_that_cannot_be_a_filename_names_no_marker_at_all() {
    let state = std::path::Path::new("/state");
    assert_eq!(
        blocked_marker(state, "sess-123"),
        Some(state.join("lights-blocked").join("sess-123")),
        "an ordinary id names a file inside the needs directory"
    );
    // THE PATH-ESCAPE GUARD, through the predicate that already backs
    // `session-<id>.start` in this same directory rather than a second one.
    for refused in ["..", "../etc/passwd", "a/b", "", "a:b", "a b"] {
        assert_eq!(
            blocked_marker(state, refused),
            None,
            "{refused:?} must name no marker"
        );
    }
}

#[test]
fn a_tick_says_a_complaint_once_and_says_it_again_only_when_it_changes() {
    let lines =
        |texts: &[&str]| -> Vec<String> { texts.iter().map(|text| text.to_string()).collect() };
    assert_eq!(
        say(&[], ""),
        Say::Nothing,
        "a happy tick says nothing at all"
    );
    assert_eq!(
        say(&lines(&["HCL9 is not on the bridge"]), ""),
        Say::Aloud("HCL9 is not on the bridge".to_string()),
        "the first tick to see a typo says so"
    );
    assert_eq!(
        say(
            &lines(&["HCL9 is not on the bridge"]),
            "HCL9 is not on the bridge"
        ),
        Say::Nothing,
        "and every tick after it is silent, which is what makes the first one readable"
    );
    assert_eq!(
        say(
            &lines(&["HCL8 is not on the bridge"]),
            "HCL9 is not on the bridge"
        ),
        Say::Aloud("HCL8 is not on the bridge".to_string()),
        "a DIFFERENT complaint is news again"
    );
    assert_eq!(
        say(&[], "HCL9 is not on the bridge"),
        Say::Forget,
        "and a complaint that cleared is forgotten, so its return is news"
    );
    assert_eq!(
        say(&lines(&["one", "two"]), ""),
        Say::Aloud("one | two".to_string()),
        "several complaints are remembered as one line, since the memory is one line"
    );
    assert_eq!(
        say(&lines(&["a\nb"]), ""),
        Say::Aloud("a b".to_string()),
        "and a complaint carrying a newline cannot become two remembered lines"
    );
}
