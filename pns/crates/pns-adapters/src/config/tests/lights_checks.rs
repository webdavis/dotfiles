use super::*;

/// The one behaviour whose COLOURS are configuration. Both of them, the blink
/// they share, and the shipped fallback are one question: what a lamp routed
/// for `checks` actually runs at.
#[test]
fn the_checks_table_takes_a_colour_pair_and_falls_back_to_the_shipped_one() {
    let shipped = lights("[lights]\n");
    assert_eq!(
        shipped.checks,
        pns_domain::lamps::config::DEFAULT_CHECKS,
        "an absent table is the May pair at the done blink's own shape"
    );
    let stated = lights(
        "[lights.checks]\nduration_ms = 2500\nbrightness = 60\n\
         pass_color = [0.2, 0.295]\nfail_color = [0.6, 0.38]\n",
    );
    assert_eq!(
        stated.checks,
        pns_domain::lamps::config::Checks {
            pulse: pns_domain::lamps::config::Pulse {
                duration_ms: 2500,
                brightness: 60,
            },
            pass_color: pns_domain::pulse::PulseColor { x: 0.2, y: 0.295 },
            fail_color: pns_domain::pulse::PulseColor { x: 0.6, y: 0.38 },
        }
    );
    // AND ONE KEY MOVES ONE KEY. The other colour stays where the shipped
    // default put it rather than being rebuilt around the one that was stated.
    let half = lights("[lights.checks]\nfail_color = [0.6, 0.38]\n");
    assert_eq!(half.checks.pass_color, pns_domain::pulse::CHECKS_PASS_COLOR);
}

/// A COLOUR IS REFUSED BY NAME rather than clamped. A coordinate outside the
/// unit square is not a colour the bridge can be asked for, and a pair that is
/// not two numbers is not a coordinate at all.
#[test]
fn a_colour_outside_the_unit_range_or_the_wrong_shape_is_refused_by_name() {
    for written in [
        "[lights.checks]\npass_color = [1.5, 0.1]\n",
        "[lights.checks]\npass_color = [-0.1, 0.1]\n",
        "[lights.checks]\nfail_color = [0.5, 1.2]\n",
        "[lights.checks]\nfail_color = [0.5]\n",
        "[lights.checks]\nfail_color = [0.5, 0.4, 0.3]\n",
        "[lights.checks]\nfail_color = 0.5\n",
        "[lights.checks]\npass_color = [\"0.5\", \"0.4\"]\n",
    ] {
        let said = refusal(written);
        assert!(
            said.contains("lights.checks")
                && (said.contains("pass_color") || said.contains("fail_color")),
            "{written:?} must be refused by name: {said}"
        );
    }
    // THE ENDS THEMSELVES ARE ACCEPTED, which is what makes the bound a bound.
    assert!(
        parse_config("[lights.checks]\npass_color = [0.0, 0.0]\nfail_color = [1.0, 1.0]\n").is_ok(),
        "the corners of the unit square are coordinates"
    );
    // A BARE INTEGER IS THE SAME NUMBER a person is most likely to type at the
    // corners, and the refusal wording says "a pair of numbers" without
    // carving out a spelling, so it is accepted rather than refused.
    assert!(
        parse_config("[lights.checks]\npass_color = [0, 1]\n").is_ok(),
        "an integer coordinate is still a number in range"
    );
    let said = refusal("[lights.checks]\npass_color = [2, 0]\n");
    assert!(
        said.contains("lights.checks")
            && said.contains("pass_color")
            && said.contains("out of range"),
        "an out-of-range integer is refused on value, not spelling: {said}"
    );
}

#[test]
fn the_headings_and_colour_keys_these_replaced_are_refused_by_name() {
    // A TABLE UNDER ITS OLD HEADING IS A LAMP THAT STOPPED SAYING ANYTHING,
    // so each one is refused with the `[lights]` vocabulary that names the
    // heading to write instead.
    for (retired, replacement) in [("github", "checks"), ("unread", "unseen")] {
        let said = refusal(&format!("[lights.{retired}]\nduration_ms = 2500\n"));
        assert!(said.contains(retired), "{said}");
        assert!(said.contains(replacement), "{said}");
    }
    for (retired, replacement) in [("pass", "pass_color"), ("fail", "fail_color")] {
        let said = refusal(&format!("[lights.checks]\n{retired} = [0.2, 0.295]\n"));
        assert!(said.contains(&format!("`{retired}`")), "{said}");
        assert!(said.contains(replacement), "{said}");
    }
}
