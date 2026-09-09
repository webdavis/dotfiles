use super::*;

#[test]
fn a_breath_whose_low_is_above_its_high_is_refused_rather_than_rendered_upside_down() {
    // EVERY FADE MOVES TOWARD ONE OF THE TWO NAMED ENDS, and with them
    // swapped a fade to `high` would move the lamp down.
    for written in [
        "[lights.blocked]\nhigh = 20\nlow = 40\n",
        "[lights.unread]\nhigh = 20\nlow = 40\n",
        "[lights.loop]\nhigh = 20\nlow = 40\n",
        "[lights.dim]\nhigh = 2\nlow = 4\n",
    ] {
        let said = refusal(written);
        assert!(
            said.contains("low 40") || said.contains("low 4"),
            "{written:?} must name both ends: {said}"
        );
        assert!(
            said.contains("move the lamp down"),
            "and say what it costs: {said}"
        );
    }
    assert!(
        parse_config("[lights.blocked]\nhigh = 40\nlow = 40\n").is_ok(),
        "equal ends are a lamp that holds steady, which is a shape rather than \
             a mistake"
    );
}

#[test]
fn an_accent_that_does_not_rise_above_the_peak_or_stay_brief_is_refused() {
    // THE MOTION IS A BREATH WITH A FLASH AT ITS PEAK, so an accent at or
    // below `high` has nothing to accent and one as long as the fades
    // around it is a third fade rather than a flash.
    for (written, names) in [
        ("[lights.loop]\nflare = 80\n", "at or below high 80"),
        ("[lights.loop]\nflare = 40\n", "at or below high 80"),
        (
            "[lights.loop]\nflare_ms = 4000\n",
            "at or above duration_ms 4000",
        ),
        (
            "[lights.loop]\nflare_ms = 4500\n",
            "at or above duration_ms 4000",
        ),
    ] {
        let said = refusal(written);
        assert!(
            said.contains(names),
            "{written:?} must name both values: {said}"
        );
        assert!(
            said.contains("lights.loop"),
            "and the table it is about: {said}"
        );
    }
    // AND THE OTHER BREATHING SHAPES HAVE NO ACCENT TO REFUSE, because they
    // have no accent at all: the knob exists only where it applies.
    for elsewhere in [
        "[lights.blocked]\nflare = 100\n",
        "[lights.unread]\nflare_ms = 200\n",
        "[lights.dim]\nflare = 100\n",
    ] {
        assert!(
            refusal(elsewhere).contains("flare"),
            "{elsewhere:?} names a key that behaviour does not have"
        );
    }
    assert!(
        parse_config("[lights.loop]\nhigh = 80\nflare = 81\nflare_ms = 3999\n").is_ok(),
        "one step above the peak and one millisecond under the breath are both \
             still an accent"
    );
}

#[test]
fn the_accent_can_never_become_the_slowest_leg_of_the_loops_own_cycle() {
    // WHAT HOLDS THE SCHEDULING MARGIN. A resumed breath may start as much
    // as one leg's step into what a tick has left of its interval, so the
    // worst case any config can produce is its LONGEST leg. Keeping the
    // accent under `duration_ms` keeps that longest leg the breath's own,
    // exactly where it sat before the accent existed, so `flare_ms` is not
    // a second way to write a leg too slow for the interval it runs in.
    for (duration_ms, flare_ms) in [(200, 200), (1000, 5000), (4000, 4000), (5000, 5000)] {
        let written =
            format!("[lights.loop]\nduration_ms = {duration_ms}\nflare_ms = {flare_ms}\n");
        assert!(
            parse_config(&written).is_err(),
            "{written:?} lets the accent match or outlast the fades around it"
        );
    }
    // AND THE SHIPPED MOTION'S OWN LONGEST LEG IS ITS BREATH'S, unchanged by
    // the accent it now carries.
    let motion = Lights::default().looping.breathe_then_flare;
    let cycle = pns_domain::lights::breath::breathe_then_flare_cycle(&motion);
    assert_eq!(
        cycle
            .iter()
            .map(|leg| pns_domain::lights::breath::step_ms(leg.duration_ms))
            .max(),
        Some(pns_domain::lights::breath::step_ms(
            motion.breath.duration_ms
        )),
        "the accent is never the leg a resume inherits its worst case from"
    );
}

#[test]
fn a_lights_value_of_the_wrong_type_is_refused_by_name_and_by_type() {
    for (written, key) in [
        ("[lights]\nrefresh_secs = \"20\"\n", "refresh_secs"),
        ("[lights.done]\nduration_ms = true\n", "duration_ms"),
        ("[lights.dim]\nlow = 10.5\n", "low"),
        ("[lights]\ndone = 3\n", "lights.done"),
        ("[lights]\nlamp = 3\n", "lamp"),
    ] {
        let said = refusal(written);
        assert!(said.contains(key), "{written:?} must name `{key}`: {said}");
    }
}
