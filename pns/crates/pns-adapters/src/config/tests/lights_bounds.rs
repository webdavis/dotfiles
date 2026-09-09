use super::*;

#[test]
fn every_lights_number_is_bounded_on_both_sides_and_refused_by_name_outside_them() {
    // BOTH ENDS, ALWAYS. A floor alone leaves a value that parses and cannot
    // work; a ceiling alone leaves the same at the other end.
    for (written, key) in [
        ("[lights]\nrefresh_secs = 9\n", "refresh_secs"),
        ("[lights]\nrefresh_secs = 31\n", "refresh_secs"),
        ("[lights.done]\nduration_ms = 199\n", "duration_ms"),
        ("[lights.done]\nduration_ms = 5001\n", "duration_ms"),
        ("[lights.done]\nbrightness = 0\n", "brightness"),
        ("[lights.done]\nbrightness = 101\n", "brightness"),
        ("[lights.blocked]\nlow = 0\n", "low"),
        ("[lights.blocked]\nhigh = 101\n", "high"),
        (
            "[lights.blocked]\ngive_up_after_secs = 59\n",
            "give_up_after_secs",
        ),
        (
            "[lights.blocked]\ngive_up_after_secs = 604801\n",
            "give_up_after_secs",
        ),
        ("[lights.loop]\nflare = 0\n", "flare"),
        ("[lights.loop]\nflare = 101\n", "flare"),
        ("[lights.loop]\nflare_ms = 199\n", "flare_ms"),
        ("[lights.loop]\nflare_ms = 5001\n", "flare_ms"),
        ("[lights.loop]\nthreshold_secs = 0\n", "threshold_secs"),
        ("[lights.loop]\nthreshold_secs = 86401\n", "threshold_secs"),
        (
            "[lights.loop]\nlease_timeout_secs = 59\n",
            "lease_timeout_secs",
        ),
        (
            "[lights.loop]\nlease_timeout_secs = 86401\n",
            "lease_timeout_secs",
        ),
        ("[lights.unread]\nafter_secs = 86401\n", "after_secs"),
    ] {
        let said = refusal(written);
        assert!(
            said.contains(key) && said.contains("range"),
            "{written:?} must be refused by name with the range echoed: {said}"
        );
    }
    // THE ENDS THEMSELVES ARE ACCEPTED, which is what makes the bound a
    // bound rather than an off-by-one.
    for written in [
        "[lights]\nrefresh_secs = 10\n",
        "[lights]\nrefresh_secs = 30\n",
        "[lights.done]\nduration_ms = 200\nbrightness = 1\n",
        "[lights.done]\nduration_ms = 5000\nbrightness = 100\n",
        "[lights.loop]\nthreshold_secs = 1\nlease_timeout_secs = 60\n",
        // THE ACCENT'S FLOOR AND THE BRIGHTNESS CEILING, both reachable.
        // Its own ceiling is not: `accent_agrees` keeps the flash under
        // `duration_ms`, which is itself capped at `MAX_FADE_MS`, so
        // `flare_ms` can never reach the range's top end and there is no
        // honest row to write for one.
        "[lights.loop]\nhigh = 99\nflare = 100\nflare_ms = 200\n",
        "[lights.unread]\nafter_secs = 0\n",
        "[lights.blocked]\ngive_up_after_secs = 60\n",
        "[lights.blocked]\ngive_up_after_secs = 604800\n",
    ] {
        assert!(
            parse_config(written).is_ok(),
            "{written:?} sits on a bound and must be accepted"
        );
    }
}

#[test]
fn the_blocked_backstop_reads_the_configured_number_rather_than_a_hardcoded_default() {
    // A KNOB WORTH NOTHING IF THE PARSER READS THE TABLE AND KEEPS THE
    // DEFAULT ANYWAY, so this proves the stated value lands rather than
    // merely that a valid table parses.
    assert_eq!(
        lights("[lights.blocked]\ngive_up_after_secs = 57600\n")
            .blocked
            .give_up_after_secs,
        57_600,
        "the shipped default, stated explicitly"
    );
    assert_eq!(
        lights("[lights.blocked]\ngive_up_after_secs = 3600\n")
            .blocked
            .give_up_after_secs,
        3_600,
        "a number that is NOT the default, so a parser that silently kept the \
             default instead of reading the table would still be caught"
    );
}

#[test]
fn a_backstop_that_gives_up_before_the_nag_nudges_is_refused_naming_both_keys() {
    // A CONFIG THAT CANNOT DO WHAT IT SAYS. The backstop darkens an
    // unanswered wait's lamp at `give_up_after_secs` and the nag cards the
    // same wait at `after_secs`. Written the shorter way round, the lamp is
    // given up on before the nudge it belongs to has ever fired, so the
    // nudge's own lamp could never be lit. Both numbers are stated by the
    // operator, so the refusal names both keys and both values rather than
    // picking one of them to be wrong.
    let said = refusal("[nag]\nafter_secs = 600\n[lights.blocked]\ngive_up_after_secs = 60\n");
    for named in [
        "lights.blocked",
        "give_up_after_secs",
        "60",
        "nag",
        "after_secs",
        "600",
    ] {
        assert!(
            said.contains(named),
            "the refusal must name {named:?}: {said}"
        );
    }

    // EQUAL IS NOT SHORTER and is accepted: the backstop reaches its bound
    // exactly as the nudge fires, which is a tight config rather than a
    // contradictory one.
    assert!(
        parse_config("[nag]\nafter_secs = 600\n[lights.blocked]\ngive_up_after_secs = 600\n")
            .is_ok(),
        "a backstop equal to the schedule sits on the bound and must be accepted"
    );

    // THE SHIPPED DEFAULTS SATISFY IT, which is the whole reason this
    // refusal costs no operator a config change.
    assert!(
        parse_config("[nag]\nafter_secs = 300\n[lights.blocked]\ngive_up_after_secs = 57600\n")
            .is_ok(),
        "the values dot_config/pns/private_config.toml.tmpl ships must parse"
    );

    // A NAG THAT IS OFF CONTRADICTS NOTHING, because no nudge ever fires
    // for the backstop to run in front of. Both spellings of off.
    for written in [
        "[lights.blocked]\ngive_up_after_secs = 60\n",
        "[nag]\nafter_secs = 0\n[lights.blocked]\ngive_up_after_secs = 60\n",
    ] {
        assert!(
            parse_config(written).is_ok(),
            "{written:?} names no schedule to contradict and must be accepted"
        );
    }
}
