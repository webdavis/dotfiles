//! What the GitHub poll's own tests hand each other.
//!
//! ONE COPY OF EACH, because the two halves (what a tick does, and what one
//! event submits as) both name an event and an instant, and a second spelling
//! of either is a fixture that can disagree with itself.

use super::*;
use pns_domain::github::notifications::NotificationThread;
use pns_domain::github::{GithubKind, GithubOutcome};

pub(super) const NOW: u64 = 1_700_000_000;
pub(super) const CURSOR: &str = "Thu, 25 Oct 2026 15:16:27 GMT";

pub(super) fn event(repo: &str, identity: &str) -> GithubEvent {
    GithubEvent {
        repo: repo.to_string(),
        kind: GithubKind::WorkflowRun,
        outcome: GithubOutcome::Neutral,
        title: "lint".to_string(),
        url: format!("https://github.com/{repo}"),
        identity: identity.to_string(),
        occurred_at: 1_789_398_987,
    }
}

/// A prior poll's state, non-empty cursor included: the ordinary tick
/// these tests mean to pin, as distinct from the first poll ever.
pub(super) fn a_prior_poll() -> PollState {
    PollState {
        last_modified: "Thu, 25 Oct 2026 00:00:00 GMT".to_string(),
        interval_secs: 60,
        seen: Vec::new(),
    }
}

/// One thread as the wire states it, for the two tests that need one.
pub(super) fn thread(repo: &str, subject_id: &str) -> NotificationThread {
    NotificationThread {
        id: "20111".to_string(),
        reason: "ci_activity".to_string(),
        subject_type: "CheckSuite".to_string(),
        subject_title: "lint".to_string(),
        subject_url: format!("https://api.github.com/repos/{repo}/check-suites/{subject_id}"),
        repo_full_name: repo.to_string(),
        repo_html_url: format!("https://github.com/{repo}"),
        updated_at: 1_789_398_987,
    }
}
