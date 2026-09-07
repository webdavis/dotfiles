//! The hue channel, pinned: body.

use super::fixtures::*;

// --- the bodies ----------------------------------------------------------

#[test]
fn the_pulse_body_carries_the_locked_colour_duration_and_brightness() {
    // THE DECISION CARRIES THE VALUE, at the seam: this asserts what the
    // render layer WRITES for a done pulse, not that a constant equals
    // itself. Change any locked figure and this line changes with it.
    let shipped = lights("[lights]\n");
    let (color, pulse, brightness) =
        pulse_render(Behaviour::Done, &shipped, Showing::Full).expect("done is a pulse");
    assert_eq!(
        pulse_body(&pulse, color, brightness),
        r#"{"dimming":{"brightness":100.0},"signaling":{"colors":[{"xy":{"x":0.17,"y":0.7}}],"duration":4000,"signal":"on_off_color"}}"#,
        "deep green, four seconds, full brightness"
    );
    let (color, pulse, brightness) =
        pulse_render(Behaviour::Failed, &shipped, Showing::Full).expect("failed is a pulse");
    assert_eq!(
        pulse_body(&pulse, color, brightness),
        r#"{"dimming":{"brightness":100.0},"signaling":{"colors":[{"xy":{"x":0.675,"y":0.322}}],"duration":4000,"signal":"on_off_color"}}"#,
        "red, four seconds, full brightness"
    );
}

#[test]
fn a_dimmed_pulse_fires_at_the_dim_floor_and_a_suppressed_one_does_not_fire() {
    let shipped = lights("[lights]\n");
    let (_, _, brightness) =
        pulse_render(Behaviour::Done, &shipped, Showing::Dimmed).expect("dimmed still fires");
    assert_eq!(
        brightness, shipped.dim.low,
        "the same blink at the faintest level the hardware has; a blink has no low \
         end to fade to, so the floor is the whole of what dim means for it"
    );
    assert!(
        pulse_render(Behaviour::Done, &shipped, Showing::Dark).is_none(),
        "and a suppressed pulse writes nothing at all"
    );
    for held in [Behaviour::Blocked, Behaviour::Unread, Behaviour::Looping] {
        assert!(
            pulse_render(held, &shipped, Showing::Full).is_none(),
            "{held:?} is a held state and has no pulse shape to fall back to"
        );
    }
}

#[test]
fn each_held_state_renders_its_own_locked_colour_and_shape() {
    let shipped = lights("[lights]\n");
    let expected = [
        (
            crate::lights::Held::Blocked,
            crate::pulse::BLOCKED_COLOR,
            crate::lights::breath_cycle(&shipped.blocked.breath),
        ),
        (
            crate::lights::Held::Looping,
            crate::pulse::LOOP_COLOR,
            crate::lights::breathe_then_flare_cycle(&shipped.looping.breathe_then_flare),
        ),
        (
            crate::lights::Held::UnreadFailure,
            crate::pulse::FAILURE_COLOR,
            crate::lights::breath_cycle(&shipped.unread.breath),
        ),
        (
            crate::lights::Held::UnreadSuccess,
            crate::pulse::UNREAD_SUCCESS_COLOR,
            crate::lights::breath_cycle(&shipped.unread.breath),
        ),
    ];
    for (held, color, cycle) in expected {
        assert_eq!(
            held_render(held, &shipped, Showing::Full),
            (color, cycle),
            "{held:?} runs its own colour at its own shape"
        );
        // THE DIM FORM IS ONE SHAPE FOR EVERY BEHAVIOUR, which is what the
        // operator locked: the colour still says which state it is, and only
        // the shape says the house is asleep. THE LOOP LOSES ITS ACCENT
        // HERE: the dim form is a plain two-leg breath for every behaviour,
        // so the flare is a property of the render and not of the state.
        assert_eq!(
            held_render(held, &shipped, Showing::Dimmed),
            (color, crate::lights::breath_cycle(&shipped.dim)),
            "{held:?} keeps its colour in the dim form"
        );
    }
    // THE LOCKED FIGURES, carried by the decision rather than echoed from a
    // constant: magenta at 100 down to 30 in two-second fades, the two
    // unread colours at 60 down to 10 in four-second ones, and the deep
    // blue rising from 10 to 80 over four seconds, flashing to 100 for two
    // hundred milliseconds at the peak, and falling back.
    assert_eq!(
        held_render(crate::lights::Held::Blocked, &shipped, Showing::Full),
        (
            crate::pulse::PulseColor {
                x: 0.3395,
                y: 0.1379
            },
            vec![
                crate::lights::Leg {
                    brightness: 30,
                    duration_ms: 2000
                },
                crate::lights::Leg {
                    brightness: 100,
                    duration_ms: 2000
                },
            ]
        )
    );
    assert_eq!(
        held_render(crate::lights::Held::Looping, &shipped, Showing::Full),
        (
            crate::pulse::PulseColor {
                x: 0.1532,
                y: 0.0475
            },
            vec![
                crate::lights::Leg {
                    brightness: 10,
                    duration_ms: 4000
                },
                crate::lights::Leg {
                    brightness: 80,
                    duration_ms: 4000
                },
                crate::lights::Leg {
                    brightness: 100,
                    duration_ms: 200
                },
            ]
        ),
        "the accent sits between the rise and the fall, which is what puts it \
         at the peak"
    );
    assert_eq!(
        held_render(crate::lights::Held::UnreadSuccess, &shipped, Showing::Full).0,
        crate::pulse::PulseColor { x: 0.50, y: 0.40 },
        "daylight for news that merely went unseen"
    );
    assert_eq!(
        held_render(crate::lights::Held::UnreadFailure, &shipped, Showing::Full).0,
        crate::pulse::PulseColor { x: 0.675, y: 0.322 },
        "and the failure pulse's own red for news that a run died"
    );
    assert_eq!(
        held_render(crate::lights::Held::Blocked, &shipped, Showing::Dimmed).1,
        vec![
            crate::lights::Leg {
                brightness: 1,
                duration_ms: 3000
            },
            crate::lights::Leg {
                brightness: 7,
                duration_ms: 3000
            },
        ],
        "the locked dim form"
    );
}

#[test]
fn the_arm_states_the_colour_and_the_first_fade_and_every_fade_after_it_states_neither() {
    // ONE WRITE RATHER THAN TWO, because a colour write followed by a fade
    // is a visible jump: the lamp would land at whatever brightness it was
    // already at, in the new colour, before starting to move.
    let breath = crate::config::Breath {
        duration_ms: 2000,
        high: 100,
        low: 30,
    };
    let cycle = crate::lights::breath_cycle(&breath);
    let fades = crate::lights::breath_fades(12_000, &cycle, crate::lights::Resume::default());
    assert_eq!(
        breath_arm_body(crate::pulse::BLOCKED_COLOR, &fades[0]),
        r#"{"color":{"xy":{"x":0.3395,"y":0.1379}},"dimming":{"brightness":30.0},"dynamics":{"duration":2000},"on":{"on":true}}"#,
    );
    assert_eq!(
        fade_body(&fades[1]),
        r#"{"dimming":{"brightness":100.0},"dynamics":{"duration":2000}}"#,
        "no colour and no `on`: the arm stated both, and repeating them is two \
         more fields the bridge reconciles mid-transition on every fade"
    );
    // AND EACH FADE IS ISSUED AT ITS OWN LEG'S DURATION, which is what the
    // accent needs: a body built from the shape rather than the fade would
    // tell the bridge to take four seconds over a two hundred millisecond
    // flash.
    let accent = crate::lights::breathe_then_flare_cycle(&crate::config::BreatheThenFlare {
        breath: crate::config::Breath {
            duration_ms: 4000,
            high: 80,
            low: 10,
        },
        flare: 100,
        flare_ms: 200,
    });
    let flare = crate::lights::breath_fades(12_000, &accent, crate::lights::Resume::default())[2];
    assert_eq!(
        fade_body(&flare),
        r#"{"dimming":{"brightness":100.0},"dynamics":{"duration":200}}"#,
        "the accent is issued at its own two hundred milliseconds"
    );
}

#[test]
fn what_puts_a_held_lamp_out_is_off_and_not_a_restore() {
    // Nothing snapshotted what the lamp was doing before the breath took it,
    // so there is nothing honest to put back.
    assert_eq!(clear_body(), r#"{"on":{"on":false}}"#);
    let bridge = bridge();
    clear_held(&bridge, &["light/a".to_string(), "light/b".to_string()]);
    assert_eq!(
        bridge.puts.borrow().as_slice(),
        &[
            ("light/a".to_string(), r#"{"on":{"on":false}}"#.to_string()),
            ("light/b".to_string(), r#"{"on":{"on":false}}"#.to_string()),
        ],
        "one PUT per held path, off the recorded names with no listing resolved"
    );
}
// --- the quiet window ---------------------------------------------------

#[test]
fn a_table_that_names_no_quiet_hours_has_no_window() {
    assert_eq!(
        quiet_window(&table("bridge = \"b\"\nkey = \"k\"")),
        Ok(None),
        "an operator who never asked to be quieted keeps today's behavior"
    );
}

#[test]
fn a_window_parses_to_minutes_since_local_midnight() {
    assert_eq!(
        quiet_window(&table("quiet_hours = \"22:00-07:00\"")),
        Ok(parse_window("22:00-07:00")),
        "22:00 is 1320 minutes in and 07:00 is 420"
    );
    assert_eq!(
        quiet_window(&table("quiet_hours = \"22:00-07:00\""))
            .expect("parses")
            .expect("a window")
            .ends_at(),
        420,
        "and the END minute is the one a bare mute reads off it: answering \
         the start would run a bedtime mute almost a whole day"
    );
}

#[test]
fn a_quiet_hours_that_is_not_two_clock_readings_is_refused_by_name() {
    for stated in [
        "22:00",
        "24:00-07:00",
        "22:60-07:00",
        "10pm-7am",
        "2:00-07:00",
        "22:00-07:00 ",
        "   ",
    ] {
        let refusal = quiet_window(&table(&format!("quiet_hours = \"{stated}\"")))
            .expect_err("a window this shape names no hours");
        assert!(
            refusal.contains("hue.quiet_hours") && refusal.contains(stated),
            "the refusal names the key and echoes what was written: {refusal}"
        );
    }
}

#[test]
fn a_quiet_hours_of_the_wrong_type_is_refused_by_name_and_by_type() {
    for (stated, kind) in [("2200", "integer"), ("true", "boolean"), ("[]", "array")] {
        let refusal = quiet_window(&table(&format!("quiet_hours = {stated}")))
            .expect_err("a window that is not a string names no hours");
        assert!(
            refusal.contains("hue.quiet_hours") && refusal.contains(kind),
            "the refusal names the key and what was written instead: {refusal}"
        );
    }
}

#[test]
fn a_blanked_quiet_hours_is_no_window_rather_than_a_refusal() {
    assert_eq!(
        quiet_window(&table("quiet_hours = \"\"")),
        Ok(None),
        "blanking a value plainly means none, the way an empty bridge or key does"
    );
}

#[test]
fn a_same_day_window_is_quiet_from_its_start_and_loud_again_at_its_end() {
    // 22:00 to 23:00, the plainest same-day window.
    let evening = parse_window("22:00-23:00").expect("valid window");
    assert!(
        !quiet_now(Some(&evening), Some(1319)),
        "the minute before the window is loud"
    );
    assert!(
        quiet_now(Some(&evening), Some(1320)),
        "the start is inside the window"
    );
    assert!(
        quiet_now(Some(&evening), Some(1379)),
        "and so is the last minute before its end"
    );
    assert!(
        !quiet_now(Some(&evening), Some(1380)),
        "the end is loud on the dot, so two adjacent windows cannot overlap"
    );
}

#[test]
fn a_window_whose_start_is_after_its_end_is_quiet_on_both_sides_of_midnight() {
    // 22:00-07:00, the window the template documents.
    let overnight = parse_window("22:00-07:00").expect("valid window");
    for (minute, quiet, moment) in [
        (1319, false, "21:59, before it opens"),
        (1320, true, "22:00, the start"),
        (1439, true, "23:59, the last minute of the day"),
        (0, true, "00:00, the first minute of the next one"),
        (419, true, "06:59, still inside"),
        (420, false, "07:00, the end"),
        (720, false, "noon, nowhere near it"),
    ] {
        assert_eq!(
            quiet_now(Some(&overnight), Some(minute)),
            quiet,
            "{moment} is on the wrong side of a window that wraps"
        );
    }
}

#[test]
fn a_window_whose_start_equals_its_end_is_never_quiet() {
    // An empty half-open range, and deliberately not a special case: the
    // all-day mute already exists as `enabled = false`. Every minute of
    // the day is checked, because "never" is the whole claim.
    let empty = parse_window("10:00-10:00").expect("valid window");
    for minute in 0..1440 {
        assert!(
            !quiet_now(Some(&empty), Some(minute)),
            "minute {minute} fell inside a window that spans no time"
        );
    }
}
