use super::*;
use crate::lamps::parse_window;

fn rule(profile: &str) -> Rule {
    Rule {
        profile: profile.to_string(),
        ..Rule::default()
    }
}

fn inputs() -> Inputs {
    Inputs {
        weekday: Some(3),
        minutes_of_day: Some(10 * 60),
        location: Some("home".to_string()),
        focus: Some("Work".to_string()),
        calendar_busy: Some(false),
    }
}

#[test]
fn each_input_matches_and_fails_to_match() {
    let cases: Vec<(Rule, bool)> = vec![
        (
            Rule {
                days: vec![3],
                ..rule("work")
            },
            true,
        ),
        (
            Rule {
                days: vec![0, 6],
                ..rule("work")
            },
            false,
        ),
        (
            Rule {
                hours: parse_window("09:00-17:00"),
                ..rule("work")
            },
            true,
        ),
        (
            Rule {
                hours: parse_window("22:00-06:00"),
                ..rule("work")
            },
            false,
        ),
        (
            Rule {
                location: Some("home".to_string()),
                ..rule("work")
            },
            true,
        ),
        (
            Rule {
                location: Some("office".to_string()),
                ..rule("work")
            },
            false,
        ),
        (
            Rule {
                focus: Some("Work".to_string()),
                ..rule("work")
            },
            true,
        ),
        (
            Rule {
                focus: Some("Sleep".to_string()),
                ..rule("work")
            },
            false,
        ),
        (
            Rule {
                calendar_busy: Some(false),
                ..rule("work")
            },
            true,
        ),
        (
            Rule {
                calendar_busy: Some(true),
                ..rule("work")
            },
            false,
        ),
    ];
    for (rule, matches) in cases {
        let resolved = resolve(std::slice::from_ref(&rule), &inputs(), None, Some(0));
        assert_eq!(
            resolved.profile == "work",
            matches,
            "{rule:?} should {}match",
            if matches { "" } else { "not " }
        );
    }
}

#[test]
fn an_unreadable_input_matches_nothing_and_falls_through_to_default() {
    let unread = Inputs {
        weekday: None,
        minutes_of_day: None,
        location: None,
        focus: None,
        calendar_busy: None,
    };
    for rule in [
        Rule {
            days: vec![3],
            ..rule("work")
        },
        Rule {
            hours: parse_window("09:00-17:00"),
            ..rule("work")
        },
        Rule {
            location: Some("home".to_string()),
            ..rule("work")
        },
        Rule {
            focus: Some("Work".to_string()),
            ..rule("work")
        },
        Rule {
            calendar_busy: Some(false),
            ..rule("work")
        },
    ] {
        let resolved = resolve(&[rule], &unread, None, Some(0));
        assert_eq!(resolved.profile, DEFAULT_PROFILE);
        assert_eq!(resolved.chose, Chose::Fallback);
    }
}

#[test]
fn the_first_matching_rule_wins_and_the_names_of_its_inputs_come_out() {
    let rules = [
        Rule {
            days: vec![0],
            ..rule("night")
        },
        Rule {
            days: vec![3],
            hours: parse_window("09:00-17:00"),
            ..rule("work")
        },
        rule("default"),
    ];
    let resolved = resolve(&rules, &inputs(), None, Some(0));
    assert_eq!(resolved.profile, "work");
    assert_eq!(
        resolved.chose,
        Chose::Rule { index: 2 },
        "the index counts from one"
    );
    assert_eq!(resolved.matched, vec!["days", "hours"]);
}

#[test]
fn a_rule_naming_no_input_matches_always_and_no_rule_matching_is_default() {
    let always = resolve(&[rule("work")], &inputs(), None, Some(0));
    assert_eq!(always.profile, "work");
    assert!(
        always.matched.is_empty(),
        "it matched on nothing, which is why it matched"
    );
    let none = resolve(&[], &inputs(), None, Some(0));
    assert_eq!(none.profile, DEFAULT_PROFILE);
    assert_eq!(none.chose, Chose::Fallback);
}

#[test]
fn a_rule_naming_two_inputs_does_not_match_when_one_of_them_does_not() {
    let rules = [Rule {
        days: vec![3],
        focus: Some("Sleep".to_string()),
        ..rule("work")
    }];
    assert_eq!(resolve(&rules, &inputs(), None, Some(0)).profile, DEFAULT_PROFILE);
}

#[test]
fn an_override_beats_every_rule_until_it_expires() {
    let rules = [rule("work")];
    let standing = Override {
        profile: "night".to_string(),
        until: Some(100),
    };
    let inside = resolve(&rules, &inputs(), Some(&standing), Some(99));
    assert_eq!(inside.profile, "night");
    assert_eq!(inside.chose, Chose::Manual { until: Some(100) });
    let at_the_second = resolve(&rules, &inputs(), Some(&standing), Some(100));
    assert_eq!(
        at_the_second.profile, "work",
        "the expiry second is already over"
    );
}

#[test]
fn an_unbounded_override_stands_and_an_unreadable_clock_does_not_end_one() {
    let rules = [rule("work")];
    let forever = Override {
        profile: "night".to_string(),
        until: None,
    };
    assert_eq!(
        resolve(&rules, &inputs(), Some(&forever), Some(9_999)).profile,
        "night"
    );
    let bounded = Override {
        profile: "night".to_string(),
        until: Some(1),
    };
    assert_eq!(
        resolve(&rules, &inputs(), Some(&bounded), None).profile,
        "night",
        "a clock nobody could read cannot say it expired"
    );
}
