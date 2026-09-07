//! The lamps, pinned: breath.

use super::*;
use fixtures::{BLOCKED, FULL_INTERVAL_MS, SLOW};

// --- the breath driver --------------------------------------------------

#[test]
fn a_zero_resume_reproduces_the_original_breath_with_one_more_fade_added() {
    // THE ORIGINAL SIX-FADE VECTOR, PRESERVED AS A PREFIX. The seamless
    // schedule does not restart the breath, it simply keeps issuing into
    // the slack the old, stop-at-the-peak schedule left unused.
    let fades = breath_fades(FULL_INTERVAL_MS, &breath_cycle(&BLOCKED), Resume::default());
    assert_eq!(
        fades,
        vec![
            Fade {
                brightness: 30,
                duration_ms: 2_000,
                start_ms: 0
            },
            Fade {
                brightness: 100,
                duration_ms: 2_000,
                start_ms: 1_950
            },
            Fade {
                brightness: 30,
                duration_ms: 2_000,
                start_ms: 3_900
            },
            Fade {
                brightness: 100,
                duration_ms: 2_000,
                start_ms: 5_850
            },
            Fade {
                brightness: 30,
                duration_ms: 2_000,
                start_ms: 7_800
            },
            Fade {
                brightness: 100,
                duration_ms: 2_000,
                start_ms: 9_750
            },
            Fade {
                brightness: 30,
                duration_ms: 2_000,
                start_ms: 11_700
            },
        ],
        "three full cycles of the locked blocked shape, plus the seventh \
         fade the seamless schedule now fits into a twelve-second interval"
    );
}

#[test]
fn each_fade_leads_the_one_before_it_so_the_lamp_never_pauses_at_an_end() {
    let fades = breath_fades(FULL_INTERVAL_MS, &breath_cycle(&BLOCKED), Resume::default());
    for pair in fades.windows(2) {
        assert_eq!(
            pair[1].start_ms - pair[0].start_ms,
            BLOCKED.duration_ms - FADE_LEAD_MS,
            "the next fade is issued FADE_LEAD_MS before the previous one ends"
        );
    }
}

#[test]
fn every_last_fade_is_issued_inside_the_budget_and_lands_after_it() {
    // THE LAW, NOT A COINCIDENCE (verified at every quarter-second budget
    // the config's own bounds ever hand this function): the slack before
    // the last issue always sits in (0, step], and that fade's own
    // duration always carries the lamp past the budget it was issued
    // in. A schedule that instead FIT the last fade's whole duration
    // inside the budget (the old, stop-at-the-peak shape, and a
    // completion-fitted rewrite of this one) fails the second assertion
    // at every budget, and the old EVEN-rounded count fails the first at
    // 11_500ms.
    for breath in [BLOCKED, SLOW] {
        let step_ms = breath.duration_ms - FADE_LEAD_MS;
        let mut budget_ms = 8_000;
        while budget_ms <= 12_000 {
            let fades = breath_fades(budget_ms, &breath_cycle(&breath), Resume::default());
            let last = fades.last().expect("8s or more is never empty");
            let slack = budget_ms - last.start_ms;
            assert!(
                slack > 0 && slack <= step_ms,
                "{}ms fades at budget {budget_ms}ms: slack {slack}ms is outside \
                 (0, {step_ms}]",
                breath.duration_ms
            );
            assert!(
                last.start_ms + breath.duration_ms > budget_ms,
                "{}ms fades at budget {budget_ms}ms: the last fade must still be \
                 running when the budget ends, and it ends at {}ms",
                breath.duration_ms,
                last.start_ms + breath.duration_ms
            );
            budget_ms += 250;
        }
    }
}

#[test]
fn a_resumed_breath_carries_on_from_the_leg_the_record_names() {
    // THE WHOLE VECTOR AND NOT ITS FIRST FADE. A resume that flipped only
    // the fade it starts with would breathe the right direction once and
    // then run the same parity as an unresumed tick, which is a lamp
    // fading to the end it is already sitting at every other cycle: no
    // motion at all for two seconds, at the join this slice exists to
    // close. The numbers are the seamless schedule resumed 1,250ms in,
    // which is what a twelve-second tick hands the one after it.
    let resumed_from_the_peak = breath_fades(
        FULL_INTERVAL_MS,
        &breath_cycle(&BLOCKED),
        Resume {
            first_due_ms: 1_250,
            next_leg: 0,
        },
    );
    assert_eq!(
        resumed_from_the_peak,
        vec![
            Fade {
                brightness: BLOCKED.low,
                duration_ms: 2_000,
                start_ms: 1_250
            },
            Fade {
                brightness: BLOCKED.high,
                duration_ms: 2_000,
                start_ms: 3_200
            },
            Fade {
                brightness: BLOCKED.low,
                duration_ms: 2_000,
                start_ms: 5_150
            },
            Fade {
                brightness: BLOCKED.high,
                duration_ms: 2_000,
                start_ms: 7_100
            },
            Fade {
                brightness: BLOCKED.low,
                duration_ms: 2_000,
                start_ms: 9_050
            },
            Fade {
                brightness: BLOCKED.high,
                duration_ms: 2_000,
                start_ms: 11_000
            },
        ],
        "a lamp resuming from the high end moves down first, and alternates \
         from there"
    );
    let resumed_from_the_floor = breath_fades(
        FULL_INTERVAL_MS,
        &breath_cycle(&BLOCKED),
        Resume {
            first_due_ms: 1_250,
            next_leg: 1,
        },
    );
    assert_eq!(
        resumed_from_the_floor
            .iter()
            .map(|fade| fade.brightness)
            .collect::<Vec<u8>>(),
        vec![
            BLOCKED.high,
            BLOCKED.low,
            BLOCKED.high,
            BLOCKED.low,
            BLOCKED.high,
            BLOCKED.low
        ],
        "and vice versa, for every fade rather than the first"
    );
    assert_eq!(
        resumed_from_the_floor
            .iter()
            .map(|fade| fade.start_ms)
            .collect::<Vec<u64>>(),
        resumed_from_the_peak
            .iter()
            .map(|fade| fade.start_ms)
            .collect::<Vec<u64>>(),
        "the direction moves the brightnesses and nothing else: both schedules \
         are issued at the same milliseconds"
    );
}

#[test]
fn a_resumes_first_due_ms_shifts_every_fades_start_by_the_same_amount() {
    let shifted = breath_fades(
        FULL_INTERVAL_MS,
        &breath_cycle(&BLOCKED),
        Resume {
            first_due_ms: 500,
            next_leg: 0,
        },
    );
    let unshifted = breath_fades(
        FULL_INTERVAL_MS - 500,
        &breath_cycle(&BLOCKED),
        Resume::default(),
    );
    let shifted_starts: Vec<u64> = shifted.iter().map(|fade| fade.start_ms - 500).collect();
    let unshifted_starts: Vec<u64> = unshifted.iter().map(|fade| fade.start_ms).collect();
    assert_eq!(
        shifted_starts, unshifted_starts,
        "a resume due 500ms late issues the same schedule 500ms later, against \
         a budget 500ms shorter"
    );
}

#[test]
fn a_budget_that_cannot_fit_even_one_fade_is_empty() {
    assert!(breath_fades(0, &breath_cycle(&BLOCKED), Resume::default()).is_empty());
    assert!(
        breath_fades(
            1_000,
            &breath_cycle(&BLOCKED),
            Resume {
                first_due_ms: 1_000,
                next_leg: 0
            }
        )
        .is_empty(),
        "a resume due exactly AT the budget has nowhere left to fade"
    );
    // AND PAST IT, which is the half of "at or past" the equality case
    // cannot reach. Without the guard the remaining budget is computed by
    // subtracting a larger number from a smaller one, so this is the case
    // that separates a schedule which refuses from one that wraps.
    assert!(
        breath_fades(
            1_000,
            &breath_cycle(&BLOCKED),
            Resume {
                first_due_ms: 1_001,
                next_leg: 0
            }
        )
        .is_empty(),
        "a resume due PAST the budget has nowhere left to fade either"
    );
}

#[test]
fn the_dim_form_is_the_same_cadence_at_the_faintest_levels_the_hardware_has() {
    // THE DIM SHAPE IS NOT A SPECIAL CASE. It is the same driver over
    // different numbers, which is what makes "dimmed" one more shape rather
    // than a second code path that can drift.
    let dim = crate::lamps::config::Breath {
        duration_ms: 3000,
        high: 7,
        low: 1,
    };
    let fades = breath_fades(FULL_INTERVAL_MS, &breath_cycle(&dim), Resume::default());
    assert_eq!(
        fades,
        vec![
            Fade {
                brightness: 1,
                duration_ms: 3_000,
                start_ms: 0
            },
            Fade {
                brightness: 7,
                duration_ms: 3_000,
                start_ms: 2_950
            },
            Fade {
                brightness: 1,
                duration_ms: 3_000,
                start_ms: 5_900
            },
            Fade {
                brightness: 7,
                duration_ms: 3_000,
                start_ms: 8_850
            },
            Fade {
                brightness: 1,
                duration_ms: 3_000,
                start_ms: 11_800
            },
        ],
    );
}

mod accent;
mod fixtures;
