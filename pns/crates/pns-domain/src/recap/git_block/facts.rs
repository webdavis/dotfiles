//! What the Git block is rendered FROM: the vocabulary git, worktrunk and
//! `gh` are read into, with no opinion about how any of it is said.

/// What `gh` had to say about one branch's pull request.
///
/// THREE ANSWERS RATHER THAN AN OPTION, because "there is no pull request" and
/// "nothing could ask" are different facts and the layout has a word for only
/// one of them. Printing `none` for a `gh` that never ran is exactly the
/// guess the rule "never guess a PR number" forbids.
#[derive(Debug, Clone, PartialEq)]
pub enum PullRequestLookup {
    /// `gh` answered with a pull request.
    Found(PullRequest),
    /// `gh` answered, and the answer was that this branch has none.
    Absent,
    /// Nothing could ask: `gh` is not installed, or it refused, or it timed
    /// out.
    Unavailable,
}

/// One pull request as the Git block speaks about it: the receipt and the one
/// word for where it stands.
#[derive(Debug, Clone, PartialEq)]
pub struct PullRequest {
    pub number: u64,
    pub state: String,
}

/// One branch of the stack.
#[derive(Debug, Clone, PartialEq)]
pub struct Branch {
    pub name: String,
    pub pull_request: PullRequestLookup,
}

/// One path the diff named, and the letter git gave it.
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub status: char,
    pub path: String,
    /// Where a rename landed. `None` for every other status.
    pub renamed_to: Option<String>,
}

/// What the Git block is rendered from.
///
/// THE CURRENT BRANCH IS THE LAST ENTRY OF `stack` rather than a field of its
/// own, so the block and the graph cannot disagree about which branch the
/// recap is for. An empty stack is a detached HEAD.
#[derive(Debug, Clone, PartialEq)]
pub struct GitFacts {
    pub worktree: String,
    pub trunk: String,
    pub stack: Vec<Branch>,
    /// The diff against the trunk, or `None` when it could not be read. An
    /// empty list is an answer; a missing one is not.
    pub changes: Option<Vec<Change>>,
}
