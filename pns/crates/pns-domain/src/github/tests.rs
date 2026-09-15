use super::*;

/// THE MUTANT THIS PINS: an outcome quietly folded into the pass colour, so a
/// review request flashes the lamp green at a human asking for something.
#[test]
fn a_neutral_outcome_has_no_colour_while_the_other_two_have_one_each() {
    assert_eq!(
        GithubOutcome::Passed.flash(),
        Some(crate::lights::flash::Flash::GithubPass)
    );
    assert_eq!(
        GithubOutcome::Failed.flash(),
        Some(crate::lights::flash::Flash::GithubFail)
    );
    assert_eq!(
        GithubOutcome::Neutral.flash(),
        None,
        "a review request, a mention and a release have no pass or fail to pulse"
    );
}

/// The vocabulary both transports map onto, spelled once. A word dropped here
/// is an event shape a producer can no longer name.
#[test]
fn every_kind_and_outcome_word_is_pinned() {
    assert_eq!(
        GITHUB_KIND_WORDS.map(|(word, _)| word),
        [
            "workflow_run",
            "check",
            "review_request",
            "mention",
            "release",
            "security_alert",
        ]
    );
    assert_eq!(
        GITHUB_OUTCOME_WORDS.map(|(word, _)| word),
        ["passed", "failed", "neutral"]
    );
}
