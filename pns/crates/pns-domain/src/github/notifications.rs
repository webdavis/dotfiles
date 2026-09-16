//! One notification thread, and the GitHub event it becomes.
//!
//! THE POLL'S HALF OF THE ONE EVENT SHAPE. A thread as the notifications API
//! states it arrives here as plain fields, and this answers the domain event
//! or nothing at all. No JSON, no HTTP, no clock: the adapter that read the
//! wire hands over what it found and this decides what it means.

use super::{GithubEvent, GithubKind, GithubOutcome};

/// One notification thread, in the fields the poll reads off it.
///
/// PLAIN STRINGS RATHER THAN A PARSED URL OR INSTANT, because every one of
/// them is the API's own text and this crate's job is to decide, not to
/// decode: the adapter proves the shape, and a field the API omitted arrives
/// empty rather than as a second absence spelling.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NotificationThread {
    /// The thread id, which is the fallback provider id and nothing else.
    pub id: String,
    /// Why this notification exists (`ci_activity`, `review_requested`, ...).
    pub reason: String,
    /// `subject.type` (`CheckSuite`, `PullRequest`, `Release`, ...).
    pub subject_type: String,
    /// `subject.title`, the workflow, check, pull request or release name.
    pub subject_title: String,
    /// `subject.url`, an API URL whose last segment is the provider's own id.
    /// Historically NULL on a `CheckSuite`, which is why nothing here may
    /// require it.
    pub subject_url: String,
    /// `repository.full_name`, owner included.
    pub repo_full_name: String,
    /// `repository.html_url`, which is where a link falls back to.
    pub repo_html_url: String,
    /// `updated_at` as epoch seconds, resolved by the adapter that read it.
    pub updated_at: u64,
}

/// The event this thread is, or nothing when this build maps no kind to it.
///
/// AN UNMAPPED REASON IS DROPPED, never folded into a catch-all kind: `assign`,
/// `subscribed`, `comment` and `state_change` are most of a busy account's
/// notifications, and a kind that swallowed them would turn the lamp into a
/// lamp that is always on. The caller counts the drops.
///
/// THE OUTCOME IS ALWAYS `Neutral`, and that is a property of this transport
/// rather than a placeholder: a notification thread carries no conclusion at
/// all, so a poll that claimed `Passed` or `Failed` would be inventing one.
/// Resolving a real outcome means a second request for the subject itself,
/// which is a rate-limit decision nobody has taken.
pub fn polled_event(thread: &NotificationThread) -> Option<GithubEvent> {
    if thread.repo_full_name.is_empty() {
        // A thread with no repository has no channel to resolve and no lamp to
        // reach. It is the one shape that is malformed rather than unmapped.
        return None;
    }
    let kind = polled_kind(&thread.reason, &thread.subject_type)?;
    Some(GithubEvent {
        repo: thread.repo_full_name.clone(),
        kind,
        outcome: GithubOutcome::Neutral,
        title: thread.subject_title.clone(),
        url: web_link(thread),
        identity: identity(&thread.repo_full_name, kind, &provider_id(thread)),
        occurred_at: thread.updated_at,
    })
}

/// Which kind this reason and subject type is, or nothing for one no variant
/// covers.
///
/// THE REASON IS TRIED FIRST AND THE SUBJECT TYPE SECOND, which is the one
/// place the design's two mapping rules could disagree: a `ci_activity`
/// notification's subject type is `CheckSuite`, and the reason is the more
/// specific of the two statements, so it wins. A `CheckSuite` arriving for
/// some other reason is still a check.
fn polled_kind(reason: &str, subject_type: &str) -> Option<GithubKind> {
    match reason {
        "ci_activity" => return Some(GithubKind::WorkflowRun),
        "review_requested" => return Some(GithubKind::ReviewRequest),
        "mention" | "team_mention" => return Some(GithubKind::Mention),
        "security_alert" | "security_advisory_credit" => return Some(GithubKind::SecurityAlert),
        _ => {}
    }
    match subject_type {
        "CheckSuite" => Some(GithubKind::Check),
        "Release" => Some(GithubKind::Release),
        _ => None,
    }
}

/// The provider's own id for the thing this notification is about: the last
/// segment of `subject.url`, or the thread id when there is no usable one.
///
/// THE SUBJECT'S ID AND NOT THE THREAD'S, because the push transport will
/// compute this same identity from a webhook payload, where only the subject's
/// numeric id exists. A thread id there would make the two transports disagree
/// and deliver every event twice. The thread id is the fallback because
/// `subject.url` is documented as required and has historically been null on a
/// `CheckSuite`: one duplicate beats no identity at all.
fn provider_id(thread: &NotificationThread) -> String {
    thread
        .subject_url
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty() && segment.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(&thread.id)
        .to_string()
}

/// This event's deduplication key: the repository, the kind and the provider's
/// own id, in one opaque word both transports compute the same way.
pub fn identity(repo: &str, kind: GithubKind, provider_id: &str) -> String {
    let word = super::GITHUB_KIND_WORDS
        .iter()
        .find(|(_, mapped)| *mapped == kind)
        .map(|(word, _)| *word)
        .unwrap_or_default();
    format!("{repo}|{word}|{provider_id}")
}

/// Where tapping this notification lands.
///
/// `subject.url` IS AN API URL and never a link a person can follow, so it is
/// TRANSLATED where the translation is exact and ignored where it is not: the
/// `/pulls/N` and `/issues/N` shapes are the majority of what a mention or a
/// review request produces, and everything else (a check suite, a release,
/// whose api ids name nothing on the website) falls back to the repository,
/// one click from the right place. NEVER EMPTY, which is why the fallback is
/// unconditional.
fn web_link(thread: &NotificationThread) -> String {
    let base = thread.repo_html_url.trim_end_matches('/');
    let Some(tail) = thread.subject_url.split("/repos/").nth(1) else {
        return base.to_string();
    };
    let mut segments = tail.split('/').skip(2);
    match (segments.next(), segments.next(), segments.next()) {
        (Some("pulls"), Some(number), None) if numeric(number) => {
            format!("{base}/pull/{number}")
        }
        (Some("issues"), Some(number), None) if numeric(number) => {
            format!("{base}/issues/{number}")
        }
        _ => base.to_string(),
    }
}

fn numeric(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests;
