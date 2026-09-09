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
            pns_domain::lights::held::Held::Blocked,
            pns_domain::pulse::BLOCKED_COLOR,
            pns_domain::lights::breath::breath_cycle(&shipped.blocked.breath),
        ),
        (
            pns_domain::lights::held::Held::Looping,
            pns_domain::pulse::LOOP_COLOR,
            pns_domain::lights::breath::breathe_then_flare_cycle(
                &shipped.looping.breathe_then_flare,
            ),
        ),
        (
            pns_domain::lights::held::Held::UnreadFailure,
            pns_domain::pulse::FAILURE_COLOR,
            pns_domain::lights::breath::breath_cycle(&shipped.unread.breath),
        ),
        (
            pns_domain::lights::held::Held::UnreadSuccess,
            pns_domain::pulse::UNREAD_SUCCESS_COLOR,
            pns_domain::lights::breath::breath_cycle(&shipped.unread.breath),
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
            (
                color,
                pns_domain::lights::breath::breath_cycle(&shipped.dim)
            ),
            "{held:?} keeps its colour in the dim form"
        );
    }
    // THE LOCKED FIGURES, carried by the decision rather than echoed from a
    // constant: magenta at 100 down to 30 in two-second fades, the two
    // unread colours at 60 down to 10 in four-second ones, and the deep
    // blue rising from 10 to 80 over four seconds, flashing to 100 for two
    // hundred milliseconds at the peak, and falling back.
    assert_eq!(
        held_render(
            pns_domain::lights::held::Held::Blocked,
            &shipped,
            Showing::Full
        ),
        (
            pns_domain::pulse::PulseColor {
                x: 0.3395,
                y: 0.1379
            },
            vec![
                pns_domain::lights::breath::Leg {
                    brightness: 30,
                    duration_ms: 2000
                },
                pns_domain::lights::breath::Leg {
                    brightness: 100,
                    duration_ms: 2000
                },
            ]
        )
    );
    assert_eq!(
        held_render(
            pns_domain::lights::held::Held::Looping,
            &shipped,
            Showing::Full
        ),
        (
            pns_domain::pulse::PulseColor {
                x: 0.1532,
                y: 0.0475
            },
            vec![
                pns_domain::lights::breath::Leg {
                    brightness: 10,
                    duration_ms: 4000
                },
                pns_domain::lights::breath::Leg {
                    brightness: 80,
                    duration_ms: 4000
                },
                pns_domain::lights::breath::Leg {
                    brightness: 100,
                    duration_ms: 200
                },
            ]
        ),
        "the accent sits between the rise and the fall, which is what puts it \
         at the peak"
    );
    assert_eq!(
        held_render(
            pns_domain::lights::held::Held::UnreadSuccess,
            &shipped,
            Showing::Full
        )
        .0,
        pns_domain::pulse::PulseColor { x: 0.50, y: 0.40 },
        "daylight for news that merely went unseen"
    );
    assert_eq!(
        held_render(
            pns_domain::lights::held::Held::UnreadFailure,
            &shipped,
            Showing::Full
        )
        .0,
        pns_domain::pulse::PulseColor { x: 0.675, y: 0.322 },
        "and the failure pulse's own red for news that a run died"
    );
    assert_eq!(
        held_render(
            pns_domain::lights::held::Held::Blocked,
            &shipped,
            Showing::Dimmed
        )
        .1,
        vec![
            pns_domain::lights::breath::Leg {
                brightness: 1,
                duration_ms: 3000
            },
            pns_domain::lights::breath::Leg {
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
    let breath = pns_domain::lamps::config::Breath {
        duration_ms: 2000,
        high: 100,
        low: 30,
    };
    let cycle = pns_domain::lights::breath::breath_cycle(&breath);
    let fades = pns_domain::lights::breath::breath_fades(
        12_000,
        &cycle,
        pns_domain::lights::breath::Resume::default(),
    );
    assert_eq!(
        breath_arm_body(pns_domain::pulse::BLOCKED_COLOR, &fades[0]),
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
    let accent = pns_domain::lights::breath::breathe_then_flare_cycle(
        &pns_domain::lamps::config::BreatheThenFlare {
            breath: pns_domain::lamps::config::Breath {
                duration_ms: 4000,
                high: 80,
                low: 10,
            },
            flare: 100,
            flare_ms: 200,
        },
    );
    let flare = pns_domain::lights::breath::breath_fades(
        12_000,
        &accent,
        pns_domain::lights::breath::Resume::default(),
    )[2];
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
