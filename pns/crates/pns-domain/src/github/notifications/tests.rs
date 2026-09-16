use super::*;

/// A `ci_activity` thread as the notifications API states one, with the
/// `CheckSuite` subject type those really carry.
fn ci_activity() -> NotificationThread {
    NotificationThread {
        id: "20111".to_string(),
        reason: "ci_activity".to_string(),
        subject_type: "CheckSuite".to_string(),
        subject_title: "lint workflow run, Attempt #1 failed".to_string(),
        subject_url: "https://api.github.com/repos/webdavis/dotfiles/check-suites/4471".to_string(),
        repo_full_name: "webdavis/dotfiles".to_string(),
        repo_html_url: "https://github.com/webdavis/dotfiles".to_string(),
        updated_at: 1_700_000_000,
    }
}

#[test]
fn a_ci_activity_thread_is_a_workflow_run_whatever_its_subject_type_says() {
    // THE MUTANT THIS PINS: the subject type consulted first, which turns
    // every workflow-run notification into a check and loses the one kind the
    // operator asked for by name.
    let event = polled_event(&ci_activity()).expect("ci_activity maps");
    assert_eq!(event.kind, GithubKind::WorkflowRun);
    assert_eq!(event.repo, "webdavis/dotfiles");
    assert_eq!(event.title, "lint workflow run, Attempt #1 failed");
    assert_eq!(event.occurred_at, 1_700_000_000);
}

#[test]
fn a_check_suite_for_any_other_reason_is_still_a_check() {
    // The positive control for the order above: the subject type has to keep
    // answering, or the second arm is dead code.
    let thread = NotificationThread {
        reason: "subscribed".to_string(),
        ..ci_activity()
    };
    assert_eq!(
        polled_event(&thread).map(|event| event.kind),
        Some(GithubKind::Check)
    );
}

#[test]
fn every_mapped_reason_reaches_its_own_kind() {
    for (reason, subject_type, kind) in [
        ("ci_activity", "CheckSuite", GithubKind::WorkflowRun),
        ("review_requested", "PullRequest", GithubKind::ReviewRequest),
        ("mention", "Issue", GithubKind::Mention),
        ("team_mention", "PullRequest", GithubKind::Mention),
        (
            "security_alert",
            "RepositoryVulnerabilityAlert",
            GithubKind::SecurityAlert,
        ),
        (
            "security_advisory_credit",
            "RepositoryAdvisory",
            GithubKind::SecurityAlert,
        ),
        ("subscribed", "Release", GithubKind::Release),
    ] {
        let thread = NotificationThread {
            reason: reason.to_string(),
            subject_type: subject_type.to_string(),
            ..ci_activity()
        };
        assert_eq!(
            polled_event(&thread).map(|event| event.kind),
            Some(kind),
            "reason {reason} with subject {subject_type}"
        );
    }
}

#[test]
fn a_reason_this_build_maps_no_kind_to_is_dropped_rather_than_folded_in() {
    // THE MUTANT THIS PINS: a catch-all kind. These are most of a busy
    // account's notifications, and a lamp that lit for them would be on all
    // day.
    for (reason, subject_type) in [
        ("assign", "Issue"),
        ("author", "PullRequest"),
        ("comment", "Issue"),
        ("state_change", "PullRequest"),
        ("subscribed", "Commit"),
        ("manual", "Issue"),
        ("approval_requested", "WorkflowRun"),
        ("invitation", "Repository"),
        ("member_feature_requested", "Repository"),
    ] {
        let thread = NotificationThread {
            reason: reason.to_string(),
            subject_type: subject_type.to_string(),
            ..ci_activity()
        };
        assert_eq!(
            polled_event(&thread),
            None,
            "reason {reason} with subject {subject_type}"
        );
    }
}

#[test]
fn a_thread_naming_no_repository_is_no_event_at_all() {
    // It has no channel to resolve and no lamp to reach: the one shape that
    // is malformed rather than unmapped.
    let thread = NotificationThread {
        repo_full_name: String::new(),
        ..ci_activity()
    };
    assert_eq!(polled_event(&thread), None);
}

#[test]
fn a_polled_event_never_claims_an_outcome_the_thread_does_not_carry() {
    // THE MUTANT THIS PINS: `Passed` compiled in as the poll's outcome, which
    // would flash the pass colour at a workflow run that failed. A
    // notification thread states no conclusion, so `Neutral` is the honest
    // answer and the only one this transport can give.
    assert_eq!(
        polled_event(&ci_activity()).map(|event| event.outcome),
        Some(GithubOutcome::Neutral)
    );
}

// --- the identity both transports have to compute the same way -------------

#[test]
fn the_identity_is_the_subjects_own_id_and_not_the_threads() {
    // THE MUTANT THIS PINS: the thread id used as the provider id, which the
    // push transport cannot see at all, so the same event would arrive twice.
    let event = polled_event(&ci_activity()).expect("ci_activity maps");
    assert_eq!(event.identity, "webdavis/dotfiles|workflow_run|4471");
    assert!(
        !event.identity.contains("20111"),
        "the thread id is not the provider id: {}",
        event.identity
    );
}

#[test]
fn a_thread_whose_subject_url_states_no_id_falls_back_to_the_thread_id() {
    // A `CheckSuite` has historically carried a null `subject.url`. One
    // duplicate against the push transport beats no identity at all.
    for subject_url in [
        "",
        "https://api.github.com/repos/webdavis/dotfiles/check-suites/",
        "https://api.github.com/repos/webdavis/dotfiles/check-suites/not-a-number",
    ] {
        let thread = NotificationThread {
            subject_url: subject_url.to_string(),
            ..ci_activity()
        };
        assert_eq!(
            polled_event(&thread).map(|event| event.identity),
            Some("webdavis/dotfiles|workflow_run|20111".to_string()),
            "case {subject_url:?}"
        );
    }
}

#[test]
fn two_kinds_of_the_same_subject_id_in_one_repository_are_two_identities() {
    // The repository and the kind are in the key for this reason: provider
    // ids are per-endpoint, so check suite 4471 and pull request 4471 are
    // different things.
    assert_ne!(
        identity("webdavis/dotfiles", GithubKind::WorkflowRun, "4471"),
        identity("webdavis/dotfiles", GithubKind::Check, "4471")
    );
    assert_ne!(
        identity("webdavis/dotfiles", GithubKind::WorkflowRun, "4471"),
        identity("webdavis/pns", GithubKind::WorkflowRun, "4471")
    );
}

// --- the link, which is never an api url and never empty -------------------

#[test]
fn a_pull_request_subject_becomes_the_web_link_for_that_pull_request() {
    let thread = NotificationThread {
        reason: "review_requested".to_string(),
        subject_type: "PullRequest".to_string(),
        subject_url: "https://api.github.com/repos/webdavis/dotfiles/pulls/689".to_string(),
        ..ci_activity()
    };
    assert_eq!(
        polled_event(&thread).map(|event| event.url),
        Some("https://github.com/webdavis/dotfiles/pull/689".to_string())
    );
}

#[test]
fn an_issue_subject_becomes_the_web_link_for_that_issue() {
    let thread = NotificationThread {
        reason: "mention".to_string(),
        subject_type: "Issue".to_string(),
        subject_url: "https://api.github.com/repos/webdavis/dotfiles/issues/42".to_string(),
        ..ci_activity()
    };
    assert_eq!(
        polled_event(&thread).map(|event| event.url),
        Some("https://github.com/webdavis/dotfiles/issues/42".to_string())
    );
}

#[test]
fn every_other_subject_lands_on_the_repository_and_never_on_an_api_url() {
    // THE MUTANT THIS PINS: `subject.url` copied through, which hands the
    // operator a JSON document instead of a page.
    for subject_url in [
        "https://api.github.com/repos/webdavis/dotfiles/check-suites/4471",
        "https://api.github.com/repos/webdavis/dotfiles/releases/9",
        "https://api.github.com/repos/webdavis/dotfiles/pulls/689/comments/1",
        "https://api.github.com/repos/webdavis/dotfiles/pulls/not-a-number",
        "",
        "nonsense",
    ] {
        let thread = NotificationThread {
            subject_url: subject_url.to_string(),
            ..ci_activity()
        };
        assert_eq!(
            polled_event(&thread).map(|event| event.url),
            Some("https://github.com/webdavis/dotfiles".to_string()),
            "case {subject_url:?}"
        );
    }
}

#[test]
fn a_repository_link_with_a_trailing_slash_does_not_double_it() {
    let thread = NotificationThread {
        reason: "mention".to_string(),
        subject_type: "Issue".to_string(),
        subject_url: "https://api.github.com/repos/webdavis/dotfiles/issues/42".to_string(),
        repo_html_url: "https://github.com/webdavis/dotfiles/".to_string(),
        ..ci_activity()
    };
    assert_eq!(
        polled_event(&thread).map(|event| event.url),
        Some("https://github.com/webdavis/dotfiles/issues/42".to_string())
    );
}
