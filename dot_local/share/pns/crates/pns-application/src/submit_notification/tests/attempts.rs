use super::{Attempt, Submission, event, missed_decision, run, submission};
use pns_domain::Overrides;

// --- what each attempt is allowed to write ------------------------------

#[test]
fn a_nudge_does_not_repeat_the_first_submission_tail() {
    let (event, decision, overrides) = (event(), missed_decision(), Overrides::default());
    let taken = Submission {
        attempt: Attempt::Nudge,
        ..submission(&event, &decision, &overrides)
    };
    assert!(run(taken).is_empty());
}

#[test]
fn an_observation_does_not_repeat_the_first_submission_tail() {
    let (event, decision, overrides) = (event(), missed_decision(), Overrides::default());
    let taken = Submission {
        attempt: Attempt::Observation,
        ..submission(&event, &decision, &overrides)
    };
    assert!(run(taken).is_empty());
}

#[test]
fn no_attempt_appends_a_duplicate_decision() {
    let (event, decision, overrides) = (event(), missed_decision(), Overrides::default());
    for attempt in [Attempt::First, Attempt::Observation, Attempt::Nudge] {
        let taken = Submission {
            attempt,
            ..submission(&event, &decision, &overrides)
        };
        assert!(!run(taken).iter().any(|step| step.starts_with("decision")));
    }
}
