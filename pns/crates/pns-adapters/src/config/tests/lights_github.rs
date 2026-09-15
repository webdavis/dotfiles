use super::*;

/// The one behaviour whose COLOURS are configuration. Both of them, the blink
/// they share, and the shipped fallback are one question: what a lamp routed
/// for `github` actually runs at.
#[test]
fn the_github_table_takes_a_colour_pair_and_falls_back_to_the_shipped_one() {
    let shipped = lights("[lights]\n");
    assert_eq!(
        shipped.github,
        pns_domain::lamps::config::DEFAULT_GITHUB,
        "an absent table is the May pair at the done blink's own shape"
    );
    let stated = lights(
        "[lights.github]\nduration_ms = 2500\nbrightness = 60\n\
         pass = [0.2, 0.295]\nfail = [0.6, 0.38]\n",
    );
    assert_eq!(
        stated.github,
        pns_domain::lamps::config::Github {
            pulse: pns_domain::lamps::config::Pulse {
                duration_ms: 2500,
                brightness: 60,
            },
            pass: pns_domain::pulse::PulseColor { x: 0.2, y: 0.295 },
            fail: pns_domain::pulse::PulseColor { x: 0.6, y: 0.38 },
        }
    );
    // AND ONE KEY MOVES ONE KEY. The other colour stays where the shipped
    // default put it rather than being rebuilt around the one that was stated.
    let half = lights("[lights.github]\nfail = [0.6, 0.38]\n");
    assert_eq!(half.github.pass, pns_domain::pulse::GITHUB_PASS_COLOR);
}

/// A COLOUR IS REFUSED BY NAME rather than clamped. A coordinate outside the
/// unit square is not a colour the bridge can be asked for, and a pair that is
/// not two numbers is not a coordinate at all.
#[test]
fn a_colour_outside_the_unit_range_or_the_wrong_shape_is_refused_by_name() {
    for written in [
        "[lights.github]\npass = [1.5, 0.1]\n",
        "[lights.github]\npass = [-0.1, 0.1]\n",
        "[lights.github]\nfail = [0.5, 1.2]\n",
        "[lights.github]\nfail = [0.5]\n",
        "[lights.github]\nfail = [0.5, 0.4, 0.3]\n",
        "[lights.github]\nfail = 0.5\n",
        "[lights.github]\npass = [\"0.5\", \"0.4\"]\n",
    ] {
        let said = refusal(written);
        assert!(
            said.contains("lights.github") && (said.contains("pass") || said.contains("fail")),
            "{written:?} must be refused by name: {said}"
        );
    }
    // THE ENDS THEMSELVES ARE ACCEPTED, which is what makes the bound a bound.
    assert!(
        parse_config("[lights.github]\npass = [0.0, 0.0]\nfail = [1.0, 1.0]\n").is_ok(),
        "the corners of the unit square are coordinates"
    );
    // A BARE INTEGER IS THE SAME NUMBER a person is most likely to type at the
    // corners, and the refusal wording says "a pair of numbers" without
    // carving out a spelling, so it is accepted rather than refused.
    assert!(
        parse_config("[lights.github]\npass = [0, 1]\n").is_ok(),
        "an integer coordinate is still a number in range"
    );
    let said = refusal("[lights.github]\npass = [2, 0]\n");
    assert!(
        said.contains("lights.github") && said.contains("pass") && said.contains("out of range"),
        "an out-of-range integer is refused on value, not spelling: {said}"
    );
}
