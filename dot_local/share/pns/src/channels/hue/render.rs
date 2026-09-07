//! The colour, motion and brightness selected for each light state.

use super::Showing;

/// The colour and the CYCLE one held state runs at, dim form or full.
///
/// THE ONE MAPPING from a state to what it looks like, read by the tick and by
/// nothing else. Its two halves travel together because a dim breath in a full
/// colour, or the reverse, is a lamp saying half of one thing.
///
/// THE SHAPE IS SETTLED HERE AND NOWHERE ELSE. This is the only place that
/// knows the loop runs a three-leg motion while everything else runs a two-leg
/// breath; the driver below schedules whichever cycle it is handed. That is
/// also why the accent is a property of the RENDER rather than of the
/// behaviour: a loop lamp inside its dim window runs the shared dim breath, so
/// the same state has an accent at full and none dimmed.
pub fn held_render(
    held: crate::lights::Held,
    lights: &crate::config::Lights,
    showing: Showing,
) -> (crate::pulse::PulseColor, Vec<crate::lights::Leg>) {
    let (color, cycle) = match held {
        crate::lights::Held::Blocked => (
            crate::pulse::BLOCKED_COLOR,
            crate::lights::breath_cycle(&lights.blocked.breath),
        ),
        crate::lights::Held::Looping => (
            crate::pulse::LOOP_COLOR,
            crate::lights::breathe_then_flare_cycle(&lights.looping.breathe_then_flare),
        ),
        crate::lights::Held::UnreadFailure => (
            crate::pulse::FAILURE_COLOR,
            crate::lights::breath_cycle(&lights.unread.breath),
        ),
        crate::lights::Held::UnreadSuccess => (
            crate::pulse::UNREAD_SUCCESS_COLOR,
            crate::lights::breath_cycle(&lights.unread.breath),
        ),
    };
    // THE DIM FORM IS ONE SHAPE FOR EVERY BEHAVIOUR, which is what the operator
    // locked: the colour still says which state it is, and the shape says the
    // house is asleep.
    match showing {
        Showing::Dimmed => (color, crate::lights::breath_cycle(&lights.dim)),
        Showing::Dark | Showing::Full => (color, cycle),
    }
}

/// The colour and brightness one pulse fires at.
pub fn pulse_render(
    behaviour: crate::config::Behaviour,
    lights: &crate::config::Lights,
    showing: Showing,
) -> Option<(crate::pulse::PulseColor, crate::config::Pulse, u8)> {
    let (color, pulse) = match behaviour {
        crate::config::Behaviour::Done => (crate::pulse::SUCCESS_COLOR, lights.done),
        crate::config::Behaviour::Failed => (crate::pulse::FAILURE_COLOR, lights.failed),
        // A HELD STATE IS NOT A PULSE, and there is no nearest shape to fall
        // back to: a lamp asked to flash a state it holds would be armed with
        // something nobody measured.
        _ => return None,
    };
    // A DIMMED PULSE IS THE SAME BLINK AT THE DIM FLOOR, which is the faintest
    // the hardware goes; there is no low end for a blink to fade to.
    match showing {
        Showing::Dark => None,
        Showing::Full => Some((color, pulse, pulse.brightness)),
        Showing::Dimmed => Some((color, pulse, lights.dim.low)),
    }
}
