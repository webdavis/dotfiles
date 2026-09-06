//! The lamps, pinned: accent.

use super::fixtures::*;

// --- the loop's accent at the peak --------------------------------------

#[test]
fn the_loop_motion_flashes_at_the_peak_between_the_rise_and_the_fall() {
    // THE OPERATOR-LOCKED SHAPE, read straight off the schedule: from 10,
    // rise to 80 over four seconds, flash to 100 for two hundred
    // milliseconds AT THE PEAK, and fall back to 10 over four seconds.
    // The accent is a leg of the cycle rather than a decoration on the
    // rise, which is what puts it between the two fades instead of inside
    // one of them.
    let fades = breath_fades(
        FULL_INTERVAL_MS,
        &breathe_then_flare_cycle(&LOOP_MOTION),
        Resume::default(),
    );
    assert_eq!(
        fades,
        vec![
            Fade {
                brightness: 10,
                duration_ms: 4_000,
                start_ms: 0
            },
            Fade {
                brightness: 80,
                duration_ms: 4_000,
                start_ms: 3_950
            },
            Fade {
                brightness: 100,
                duration_ms: 200,
                start_ms: 7_900
            },
            Fade {
                brightness: 10,
                duration_ms: 4_000,
                start_ms: 8_050
            },
        ],
        "the accent follows the rise and precedes the fall, and it is issued \
         at its own two hundred milliseconds rather than at the breath's four \
         seconds, so the fall follows it 150ms later rather than 3,950ms later"
    );
}

#[test]
fn the_accent_leads_the_fades_around_it_by_the_same_lead_every_other_fade_gets() {
    // THE SEAMLESS TURN-AROUND, WHERE IT MATTERS MOST. `FADE_LEAD_MS` is
    // what stops the lamp sitting still at an end, and the accent sits
    // exactly at the end where a pause would be most visible. Each fade is
    // still issued its own duration less the lead after the one before it,
    // the accent included, so the lamp is moving into the flash and moving
    // out of it.
    let fades = breath_fades(
        FULL_INTERVAL_MS,
        &breathe_then_flare_cycle(&LOOP_MOTION),
        Resume::default(),
    );
    for pair in fades.windows(2) {
        assert_eq!(
            pair[1].start_ms - pair[0].start_ms,
            pair[0].duration_ms - FADE_LEAD_MS,
            "every fade is issued FADE_LEAD_MS before the one before it ends, \
             including the two either side of the accent"
        );
    }
}

#[test]
fn a_tick_that_ended_on_the_rise_takes_the_accent_next_and_not_the_fall() {
    // THE REGRESSION A TWO-VALUED RECORD WOULD SHIP. The rise occupies a
    // whole step of the cycle, so roughly half of all ticks end having just
    // issued it; a record that could only say "high" or "low" would send
    // every one of those straight to the fall, and half the cycles would
    // lose the accent the operator locked.
    let cycle = breathe_then_flare_cycle(&LOOP_MOTION);
    let on_the_rise = HeldEntry {
        path: "light/l1".to_string(),
        resume: Some(Phase {
            end_unix_ms: 13_700,
            landed_on: LOOP_MOTION.breath.high,
            held: Held::Looping,
        }),
    };
    assert_eq!(
        resume_from(Some(&on_the_rise), 12_400, Held::Looping, &cycle),
        Resume {
            first_due_ms: 1_250,
            next_leg: 2
        },
        "the leg after the rise is the accent"
    );
    assert_eq!(
        breath_fades(
            FULL_INTERVAL_MS,
            &cycle,
            Resume {
                first_due_ms: 1_250,
                next_leg: 2
            }
        )[0],
        Fade {
            brightness: 100,
            duration_ms: 200,
            start_ms: 1_250
        },
        "so the tick that inherits it flashes before it falls"
    );
}

#[test]
fn a_tick_that_ended_on_the_accent_falls_next_and_inherits_only_the_accents_own_step() {
    // THE ACCENT'S LANDING IS ITS OWN. A fade recorded at the breath's four
    // seconds when it really took two hundred milliseconds would tell the
    // next tick to wait almost four seconds for a lamp that has already
    // stopped moving.
    let cycle = breathe_then_flare_cycle(&LOOP_MOTION);
    let on_the_accent = HeldEntry {
        path: "light/l1".to_string(),
        resume: Some(Phase {
            end_unix_ms: 10_000,
            landed_on: LOOP_MOTION.flare,
            held: Held::Looping,
        }),
    };
    let accent_step_ms = step_ms(LOOP_MOTION.flare_ms);
    assert_eq!(
        resume_from(
            Some(&on_the_accent),
            10_000 - FADE_LEAD_MS - accent_step_ms,
            Held::Looping,
            &cycle
        ),
        Resume {
            first_due_ms: accent_step_ms,
            next_leg: 0
        },
        "the leg after the accent is the fall, and one accent step out is the \
         furthest a live record can leave it"
    );
    assert_eq!(
        resume_from(
            Some(&on_the_accent),
            10_000 - FADE_LEAD_MS - accent_step_ms - 1,
            Held::Looping,
            &cycle
        ),
        Resume::default(),
        "a millisecond further out than the ACCENT'S own step could have left \
         it is a clock that moved, even though the breath's own legs could \
         honestly reach that far"
    );
}

#[test]
fn a_level_the_cycle_about_to_run_has_no_leg_for_starts_the_lamp_fresh() {
    // THE SHAPE CHANGE, which the state comparison alone does not catch: a
    // loop lamp entering its dim window keeps the same state and swaps a
    // three-leg motion for a two-leg breath. The record names a brightness
    // the dim form never fades to, so there is no leg to carry on from.
    let dim = crate::config::Breath {
        duration_ms: 3000,
        high: 7,
        low: 1,
    };
    let at_full = HeldEntry {
        path: "light/l1".to_string(),
        resume: Some(Phase {
            end_unix_ms: 13_700,
            landed_on: LOOP_MOTION.breath.high,
            held: Held::Looping,
        }),
    };
    assert_eq!(
        resume_from(Some(&at_full), 12_400, Held::Looping, &breath_cycle(&dim)),
        Resume::default(),
        "the dim form starts at its own beginning rather than resuming into a \
         leg that is not there"
    );
    assert_eq!(
        resume_from(
            Some(&at_full),
            12_400,
            Held::Looping,
            &breathe_then_flare_cycle(&LOOP_MOTION)
        ),
        Resume {
            first_due_ms: 1_250,
            next_leg: 2
        },
        "and the same record against the shape it was written under still \
         resumes"
    );
}
