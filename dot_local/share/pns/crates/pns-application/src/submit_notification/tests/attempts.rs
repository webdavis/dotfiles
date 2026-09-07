use super::{Attempt, Submission, event, missed_decision, run, submission};
use pns_domain::Overrides;

// --- what each attempt is allowed to write ------------------------------

#[test]
fn a_nudge_writes_its_decision_line_and_stops() {
    let (event, decision, overrides) = (event(), missed_decision(), Overrides::default());
    let taken = Submission {
        attempt: Attempt::Nudge,
        ..submission(&event, &decision, &overrides)
    };
    assert_eq!(run(taken), ["decision(nag)"]);
}

#[test]
fn an_observation_writes_its_decision_line_and_stops() {
    let (event, decision, overrides) = (event(), missed_decision(), Overrides::default());
    let taken = Submission {
        attempt: Attempt::Observation,
        ..submission(&event, &decision, &overrides)
    };
    assert_eq!(run(taken), ["decision"]);
}

#[test]
fn only_a_nudge_marks_its_decision_line_as_one() {
    let (event, decision, overrides) = (event(), missed_decision(), Overrides::default());
    for (attempt, expected) in [
        (Attempt::First, "decision"),
        (Attempt::Observation, "decision"),
        (Attempt::Nudge, "decision(nag)"),
    ] {
        let taken = Submission {
            attempt,
            ..submission(&event, &decision, &overrides)
        };
        assert_eq!(run(taken).first().map(String::as_str), Some(expected));
    }
}
