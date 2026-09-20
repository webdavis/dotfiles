use super::*;

#[test]
fn every_lights_number_is_bounded_on_both_sides_and_refused_by_name_outside_them() {
    // BOTH ENDS, ALWAYS. A floor alone leaves a value that parses and cannot
    // work; a ceiling alone leaves the same at the other end.
    for (written, key) in [
        ("[lights]\narm_interval = \"9s\"\n", "arm_interval"),
        ("[lights]\narm_interval = \"31s\"\n", "arm_interval"),
        ("[lights]\narm_interval = \"0s\"\n", "arm_interval"),
        ("[lights.done]\nduration = \"199ms\"\n", "duration"),
        ("[lights.done]\nduration = \"5001ms\"\n", "duration"),
        ("[lights.done]\nduration = \"0s\"\n", "duration"),
        (
            "[lights.done]\nbrightness_percent = 0\n",
            "brightness_percent",
        ),
        (
            "[lights.done]\nbrightness_percent = 101\n",
            "brightness_percent",
        ),
        ("[lights.blocked]\nlow_percent = 0\n", "low_percent"),
        ("[lights.blocked]\nhigh_percent = 101\n", "high_percent"),
        ("[lights.blocked]\nlease_expiry = \"59s\"\n", "lease_expiry"),
        (
            "[lights.blocked]\nlease_expiry = \"604801s\"\n",
            "lease_expiry",
        ),
        ("[lights.blocked]\nlease_expiry = \"0s\"\n", "lease_expiry"),
        ("[lights.loop]\nflare_percent = 0\n", "flare_percent"),
        ("[lights.loop]\nflare_percent = 101\n", "flare_percent"),
        (
            "[lights.loop]\nflare_duration = \"199ms\"\n",
            "flare_duration",
        ),
        (
            "[lights.loop]\nflare_duration = \"5001ms\"\n",
            "flare_duration",
        ),
        ("[lights.loop]\nflare_duration = \"0s\"\n", "flare_duration"),
        ("[lights.loop]\narm_after = \"0s\"\n", "arm_after"),
        ("[lights.loop]\narm_after = \"86401s\"\n", "arm_after"),
        ("[lights.loop]\nlease_expiry = \"59s\"\n", "lease_expiry"),
        ("[lights.loop]\nlease_expiry = \"86401s\"\n", "lease_expiry"),
        ("[lights.loop]\nlease_expiry = \"0s\"\n", "lease_expiry"),
        ("[lights.unseen]\narm_after = \"86401s\"\n", "arm_after"),
    ] {
        let said = refusal(written);
        assert!(
            said.contains(key) && (said.contains("range") || said.contains("outside")),
            "{written:?} must be refused by name with the range echoed: {said}"
        );
    }
    // THE ENDS THEMSELVES ARE ACCEPTED, which is what makes the bound a
    // bound rather than an off-by-one.
    for written in [
        "[lights]\narm_interval = \"10s\"\n",
        "[lights]\narm_interval = \"30s\"\n",
        "[lights.done]\nduration = \"200ms\"\nbrightness_percent = 1\n",
        "[lights.done]\nduration = \"5s\"\nbrightness_percent = 100\n",
        "[lights.loop]\narm_after = \"1s\"\nlease_expiry = \"60s\"\n",
        // THE ACCENT'S FLOOR AND THE BRIGHTNESS CEILING, both reachable.
        // Its own ceiling is not: `accent_agrees` keeps the flash under
        // `duration`, which is itself capped at `MAX_FADE_MS`, so
        // `flare_duration` can never reach the range's top end and there is
        // no honest row to write for one.
        "[lights.loop]\nhigh_percent = 99\nflare_percent = 100\nflare_duration = \"200ms\"\n",
        "[lights.unseen]\narm_after = \"0s\"\n",
        "[lights.blocked]\nlease_expiry = \"60s\"\n",
        "[lights.blocked]\nlease_expiry = \"168h\"\n",
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
        lights("[lights.blocked]\nlease_expiry = \"16h\"\n")
            .blocked
            .lease_expiry_secs,
        57_600,
        "the shipped default, stated explicitly"
    );
    assert_eq!(
        lights("[lights.blocked]\nlease_expiry = \"1h\"\n")
            .blocked
            .lease_expiry_secs,
        3_600,
        "a number that is NOT the default, so a parser that silently kept the \
             default instead of reading the table would still be caught"
    );
}

#[test]
fn a_backstop_that_gives_up_before_the_reminder_nudges_is_refused_naming_both_keys() {
    // A CONFIG THAT CANNOT DO WHAT IT SAYS. The backstop darkens an
    // unanswered wait's lamp at `lease_expiry` and the reminder cards the
    // same wait at `delay`. Written the shorter way round, the lamp is
    // given up on before the nudge it belongs to has ever fired, so the
    // nudge's own lamp could never be lit. Both numbers are stated by the
    // operator, so the refusal names both keys and both values rather than
    // picking one of them to be wrong.
    let said = refusal("[remind]\ndelay = \"10m\"\n[lights.blocked]\nlease_expiry = \"60s\"\n");
    for named in [
        "lights.blocked",
        "lease_expiry",
        "60",
        "remind",
        "delay",
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
        parse_config("[remind]\ndelay = \"10m\"\n[lights.blocked]\nlease_expiry = \"600s\"\n")
            .is_ok(),
        "a backstop equal to the schedule sits on the bound and must be accepted"
    );

    // THE SHIPPED DEFAULTS SATISFY IT, which is the whole reason this
    // refusal costs no operator a config change.
    assert!(
        parse_config("[remind]\ndelay = \"5m\"\n[lights.blocked]\nlease_expiry = \"16h\"\n")
            .is_ok(),
        "the values dot_config/pns/private_config.toml.tmpl ships must parse"
    );

    // A REMINDER THAT IS OFF CONTRADICTS NOTHING, because no nudge ever fires
    // for the backstop to run in front of, and off is the key left out.
    let written = "[lights.blocked]\nlease_expiry = \"60s\"\n";
    assert!(
        parse_config(written).is_ok(),
        "{written:?} names no schedule to contradict and must be accepted"
    );
}

#[test]
fn every_retired_lights_timing_key_is_refused_by_name_with_its_new_spelling_beside_it() {
    // A FILE THAT MISSED THE RENAME IS REFUSED, not half-read: each of
    // these parsed and set a schedule yesterday, so accepting the table
    // without them would leave the lamps running at a default the
    // operator believes they changed.
    for (retired, replacement) in [
        ("[lights]\nrefresh_secs = 12\n", "arm_interval"),
        (
            "[lights.blocked]\ngive_up_after_secs = 57600\n",
            "lease_expiry",
        ),
        ("[lights.loop]\nlease_timeout_secs = 3900\n", "lease_expiry"),
        ("[lights.loop]\nthreshold_secs = 300\n", "arm_after"),
        ("[lights.unseen]\nafter_secs = 300\n", "arm_after"),
        ("[lights.done]\nduration_ms = 4000\n", "duration"),
        ("[lights.done]\nbrightness = 100\n", "brightness_percent"),
        ("[lights.blocked]\nhigh = 100\n", "high_percent"),
        ("[lights.blocked]\nlow = 30\n", "low_percent"),
        ("[lights.loop]\nflare = 100\n", "flare_percent"),
        ("[lights.loop]\nflare_ms = 200\n", "flare_duration"),
    ] {
        let said = refusal(retired);
        let key = retired
            .lines()
            .nth(1)
            .and_then(|line| line.split(' ').next())
            .expect("a key");
        assert!(
            said.contains(key) && said.contains(replacement),
            "the refusal names the retired key and its replacement: {said}"
        );
    }
}

#[test]
fn a_lights_timing_key_written_as_a_bare_count_is_refused_by_name() {
    // The old spelling's VALUE is the other half of the rename: a bare
    // number meant seconds here and minutes elsewhere, which is what the
    // duration vocabulary exists to end.
    for written in [
        "[lights]\narm_interval = 12\n",
        "[lights.blocked]\nlease_expiry = 57600\n",
        "[lights.loop]\narm_after = 300\n",
        "[lights.loop]\nlease_expiry = 3900\n",
        "[lights.unseen]\narm_after = 300\n",
    ] {
        let said = refusal(written);
        assert!(
            said.contains("duration"),
            "{written:?} must be refused as not a duration: {said}"
        );
    }
}

#[test]
fn the_two_lease_expiries_and_the_two_arm_afters_are_the_same_word_on_both_tables() {
    // ONE WORD PER IDEA, proven by both tables reading their own value
    // through it rather than by the roster listing it twice.
    let held =
        lights("[lights.loop]\nlease_expiry = \"20m\"\n[lights.blocked]\nlease_expiry = \"2h\"\n");
    assert_eq!(held.looping.lease_expiry_secs, 1_200);
    assert_eq!(held.blocked.lease_expiry_secs, 7_200);

    let arming =
        lights("[lights.loop]\narm_after = \"90s\"\n[lights.unseen]\narm_after = \"30s\"\n");
    assert_eq!(arming.looping.arm_after_secs, 90);
    assert_eq!(arming.unseen.arm_after_secs, 30);

    // AND `arm_interval` IS THE DAEMON'S RE-ARM INTERVAL `refresh_secs` was.
    assert_eq!(
        lights("[lights]\narm_interval = \"25s\"\n").arm_interval_secs,
        25
    );
}
