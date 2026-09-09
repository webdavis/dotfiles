use super::*;

fn hermes_404() -> Failure {
    Failure {
        id: 47,
        destination: DESTINATION_HERMES.to_string(),
        route: "testpath".to_string(),
        address: "127.0.0.1:8644".to_string(),
        agent: "posture heartbeat".to_string(),
        command: "pns --agent posture --channel testpath".to_string(),
        outcome: DeliveryOutcome::Status(404),
        retries: 1,
        max_attempts: 20,
    }
}

fn temporary() -> Failure {
    Failure {
        outcome: DeliveryOutcome::Status(503),
        retries: 3,
        ..hermes_404()
    }
}

/// The field order is the contract both forms share, so it is asserted on the
/// rendered text rather than on the list that produces it.
#[test]
fn the_full_form_carries_the_designed_fields_in_the_designed_order() {
    let rendered = full(&hermes_404());
    let labels: Vec<&str> = rendered
        .lines()
        .skip(1)
        .filter_map(|line| line.trim_start().split_once(':').map(|(label, _)| label))
        .collect();
    assert_eq!(
        labels,
        [
            "status",
            "meaning",
            "webhook route",
            "sent by",
            "failed command",
            "fix"
        ]
    );
    assert!(rendered.starts_with("pns: delivery failed\n"));
    assert!(rendered.contains("HTTP 404 (Not Found)"));
    assert!(rendered.contains("the hermes gateway has no route named testpath"));
}

/// The notification form drops exactly one field, and for a stated reason: the
/// command already carries the agent, and a form under a budget cannot say
/// anything twice.
#[test]
fn the_notification_form_drops_only_sent_by_and_keeps_the_rest_in_order() {
    let rendered = notification(&hermes_404(), NotificationSurface::Banner);
    let labels: Vec<&str> = rendered
        .lines()
        .filter_map(|line| line.split_once(':').map(|(label, _)| label))
        .collect();
    assert_eq!(
        labels,
        [
            "status",
            "meaning",
            "webhook route",
            "failed command",
            "fix"
        ]
    );
    assert!(
        !rendered.contains("sent by"),
        "the agent is already in the command"
    );
    assert!(rendered.contains("--agent posture"));
}

/// The measured budget, counting newlines, against inputs no producer is
/// stopped from sending. A body that goes over is cut by the SURFACE at
/// whatever character it reaches, which takes the fix line off the end.
#[test]
fn a_notification_holds_the_measured_budget_however_long_the_route_and_command_are() {
    for (route, command) in [
        ("testpath".to_string(), "pns --agent posture".to_string()),
        ("a".repeat(200), "pns --agent posture".to_string()),
        (
            "testpath".to_string(),
            "pns --agent x".to_string() + &"y".repeat(300),
        ),
        ("a".repeat(200), "b".repeat(300)),
    ] {
        for surface in [
            NotificationSurface::Banner,
            NotificationSurface::Phone {
                serve: true,
                hermes_failed: false,
            },
            NotificationSurface::Phone {
                serve: false,
                hermes_failed: false,
            },
            NotificationSurface::Phone {
                serve: false,
                hermes_failed: true,
            },
        ] {
            let failure = Failure {
                route: route.clone(),
                command: command.clone(),
                ..hermes_404()
            };
            let rendered = notification(&failure, surface);
            assert!(
                rendered.chars().count() <= NOTIFICATION_MAX_CHARS,
                "{} chars for a {}-char route and a {}-char command",
                rendered.chars().count(),
                route.len(),
                command.len()
            );
        }
    }
}

/// The fix line is the one the reader acts on, so it survives the budget even
/// when everything else has to give way.
#[test]
fn the_fix_line_survives_a_route_and_command_that_would_fill_the_whole_budget() {
    let failure = Failure {
        route: "a".repeat(200),
        command: "b".repeat(300),
        ..hermes_404()
    };
    let rendered = notification(&failure, NotificationSurface::Banner);
    assert!(
        rendered.ends_with("fix: run `pns failures` for the full error and how to fix it"),
        "{rendered}"
    );
}

/// Five rows, and the phone's three are what the branch exists for: a phone
/// reader has no terminal in reach, so the line has to be a pointer they can
/// follow with two taps.
#[test]
fn the_fix_line_points_at_the_surface_the_reader_is_standing_at() {
    let failure = hermes_404();
    let fix = |surface| {
        notification(&failure, surface)
            .lines()
            .last()
            .unwrap()
            .to_string()
    };
    // The terminal row is read off the full form, which is the only place a
    // terminal surface renders: `notification` will not take one.
    assert!(
        full(&failure)
            .lines()
            .last()
            .unwrap()
            .contains("~/.hermes/config.yaml")
    );
    assert_eq!(
        fix(NotificationSurface::Banner),
        "fix: run `pns failures` for the full error and how to fix it"
    );
    assert_eq!(
        fix(NotificationSurface::Phone {
            serve: true,
            hermes_failed: true
        }),
        "fix: open moshi's servers list, pick pns :8646",
        "the page is the whole record, whichever leg failed"
    );
    assert_eq!(
        fix(NotificationSurface::Phone {
            serve: false,
            hermes_failed: false
        }),
        "fix: full error in Discord, #priority"
    );
    assert_eq!(
        fix(NotificationSurface::Phone {
            serve: false,
            hermes_failed: true
        }),
        "fix: run `pns failures` on dresden",
        "nothing else is reachable when the gateway leg is what broke"
    );
}

/// A temporary failure has nothing to repair, so it says to stand down on every
/// surface. That one line is what makes the two classes tell apart at a glance.
#[test]
fn a_temporary_failure_says_to_stand_down_on_every_surface_and_counts_the_attempts() {
    let rendered = full(&temporary());
    let last = rendered.lines().last().unwrap();
    let (label, value) = last.trim().split_once(':').unwrap();
    assert_eq!(
        (label, value.trim()),
        (
            "fix",
            "nothing to do, pns will retry (3 of 20 attempts used)"
        )
    );
    for surface in [
        NotificationSurface::Banner,
        NotificationSurface::Phone {
            serve: true,
            hermes_failed: false,
        },
        NotificationSurface::Phone {
            serve: false,
            hermes_failed: true,
        },
    ] {
        assert!(
            notification(&temporary(), surface)
                .ends_with("fix: nothing to do, pns will retry (3 of 20 attempts used)"),
            "{surface:?}"
        );
    }
}

/// A 401 from hermes and a 401 from moshi name DIFFERENT secrets, which is why
/// the wording is keyed by destination and code together rather than by code.
#[test]
fn the_same_code_from_two_destinations_names_two_different_secrets() {
    let hermes = Failure {
        outcome: DeliveryOutcome::Status(401),
        ..hermes_404()
    };
    let mobile = Failure {
        destination: DESTINATION_MOBILE.to_string(),
        address: "http://127.0.0.1:8646".to_string(),
        ..hermes.clone()
    };
    assert!(full(&hermes).contains(HERMES_KEY));
    assert!(full(&mobile).contains(MOBILE_TOKEN));
    assert!(!full(&hermes).contains(MOBILE_TOKEN));
    assert!(!full(&mobile).contains(HERMES_KEY));
}

/// A meaning never says "the route" or "the key" in the abstract, because a
/// reader holding a phone cannot resolve a pronoun against a system they are
/// not looking at.
#[test]
fn every_meaning_names_its_concrete_subject_rather_than_a_pronoun() {
    for code in [400, 401, 403, 404, 405, 410, 422, 502] {
        let failure = Failure {
            route: "uniquename".to_string(),
            outcome: DeliveryOutcome::Status(code),
            ..hermes_404()
        };
        let meaning = notification(&failure, NotificationSurface::Banner)
            .lines()
            .nth(1)
            .unwrap()
            .to_string();
        assert!(
            meaning.contains("uniquename")
                || meaning.contains(HERMES_KEY)
                || meaning.contains("127.0.0.1:8644"),
            "HTTP {code} named nothing concrete: {meaning}"
        );
    }
}

/// An answer with no status still says which of the two silences it was, since
/// they call for different repairs: one is a URL pns built wrong, the other is
/// a gateway that is not there.
#[test]
fn the_two_answers_that_carry_no_status_are_told_apart_by_name() {
    let no_response = Failure {
        outcome: DeliveryOutcome::NoResponse,
        ..hermes_404()
    };
    let bad_url = Failure {
        outcome: DeliveryOutcome::NoStatus,
        ..hermes_404()
    };
    assert!(full(&no_response).contains("no response"));
    assert!(full(&no_response).contains("nothing answered at 127.0.0.1:8644"));
    assert!(full(&bad_url).contains("bad URL"));
    assert!(full(&bad_url).contains("is malformed, nothing was sent"));
}

/// The heading varies per FAILURE, not per producer. Four probes sharing one
/// triple produced ONE notification; every posture delivery failure would carry
/// the same triple, so a second failure could quietly displace the first.
#[test]
fn two_failures_from_one_producer_carry_two_distinct_headings() {
    let first = hermes_404();
    let second = Failure {
        id: 48,
        ..first.clone()
    };
    assert_ne!(first.title(), second.title());
    assert!(first.title().contains("#47"));
    assert!(second.title().contains("#48"));
}

/// The classifier is total, so the wording is too: a code with no table row
/// still says which way pns will treat it, because that is what decides whether
/// the reader has anything to do.
#[test]
fn a_code_with_no_table_row_still_says_which_way_it_will_be_treated() {
    let permanent = Failure {
        outcome: DeliveryOutcome::Status(451),
        ..hermes_404()
    };
    let temporary = Failure {
        outcome: DeliveryOutcome::Status(507),
        ..hermes_404()
    };
    assert!(full(&permanent).contains("will not accept a repeat"));
    assert!(full(&permanent).contains("HTTP 451"));
    assert!(full(&temporary).contains("this time"));
    assert!(full(&temporary).contains("nothing to do, pns will retry"));
}
