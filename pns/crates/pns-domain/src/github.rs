//! The one GitHub event shape, whatever produced it.
//!
//! ONE SHAPE FOR EVERY TRANSPORT (design 2026-09-14). A poll and a webhook
//! both answer with this value, and everything downstream (the lamp, the
//! channel, the phone) reads it without ever learning which one arrived.

/// What KIND of GitHub thing happened. A CLOSED SET, for `Behaviour`'s own
/// reason: an open string is a value that reaches a config-driven map, matches
/// nothing and produces silence nobody can see. A reason or webhook event with
/// no variant here is DROPPED by whatever mapped it, never folded into a
/// catch-all, because a catch-all kind is how `assign` and `subscribed` turn a
/// useful lamp into a lamp that is always on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GithubKind {
    WorkflowRun,
    Check,
    ReviewRequest,
    Mention,
    Release,
    SecurityAlert,
}

/// The six kinds, in the spelling the `github` extension uses.
pub const GITHUB_KIND_WORDS: [(&str, GithubKind); 6] = [
    ("workflow_run", GithubKind::WorkflowRun),
    ("check", GithubKind::Check),
    ("review_request", GithubKind::ReviewRequest),
    ("mention", GithubKind::Mention),
    ("release", GithubKind::Release),
    ("security_alert", GithubKind::SecurityAlert),
];

/// How it turned out.
///
/// THREE-VALUED AND NOT TWO. `Neutral` exists because a review request, a
/// mention and a release have no pass or fail, and forcing them into `Passed`
/// would flash the pass colour at a human asking for something. A `Neutral`
/// event still delivers; it just never reaches a lamp, which is stated on the
/// event rather than at the lamp: a lamp that cannot represent an event is the
/// event's own problem to declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GithubOutcome {
    Passed,
    Failed,
    Neutral,
}

/// The three outcomes, in the spelling the `github` extension uses.
pub const GITHUB_OUTCOME_WORDS: [(&str, GithubOutcome); 3] = [
    ("passed", GithubOutcome::Passed),
    ("failed", GithubOutcome::Failed),
    ("neutral", GithubOutcome::Neutral),
];

impl GithubOutcome {
    /// Which colour of the `github` lamp this outcome runs at, or nothing at
    /// all for the outcome that has no colour to run at.
    pub fn flash(self) -> Option<crate::lights::flash::Flash> {
        match self {
            GithubOutcome::Passed => Some(crate::lights::flash::Flash::GithubPass),
            GithubOutcome::Failed => Some(crate::lights::flash::Flash::GithubFail),
            GithubOutcome::Neutral => None,
        }
    }
}

/// One GitHub event, as both transports produce it.
///
/// `identity` IS WHAT MAKES THE SAME EVENT ARRIVE TWICE AND BE DELIVERED ONCE:
/// the repository, the kind and the provider's own id for the thing, in one
/// opaque word. Nothing here interprets it; the deduplication that reads it
/// arrives with the transports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubEvent {
    /// The FULL name, owner included (`webdavis/dotfiles`), because GitHub
    /// only ever knows repositories: the day the tools are split out, a short
    /// name would be a collision instead of an addition.
    pub repo: String,
    pub kind: GithubKind,
    pub outcome: GithubOutcome,
    /// The workflow, check, pull request or release name.
    pub title: String,
    /// Where tapping the notification lands.
    pub url: String,
    pub identity: String,
    pub occurred_at: u64,
}

pub mod notifications;
pub mod poll;

#[cfg(test)]
mod tests;
