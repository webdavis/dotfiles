//! What git, worktrunk and gh-axi say about the worktree the work happened in.
//!
//! ONE READ, NO WRITE. Every spawn below is a query: a branch listing, a diff,
//! a pull-request listing. Nothing here checks out, fetches, rebases or posts,
//! which is what bounds a command an agent runs at the end of a turn.
//!
//! WORKTRUNK HAS NO STACK TO ASK FOR. `wt list` reports worktrees and their
//! status, and `gh-axi stack` needs `github/gh-stack`, which is not installed
//! on this machine (both MEASURED 2026-09-14). So the stack is derived from git
//! ancestry, which is the same fact either tool would have reported: a branch
//! is below HEAD in the stack when its tip is an ancestor of HEAD and is not
//! already in the trunk.

mod listing;

use crate::run_bounded;
use pns_domain::recap::git_block::{Branch, Change, GitFacts, PullRequestLookup};
use std::process::Command;
use std::time::Duration;

/// Everything the Git block is rendered from, read out of `cwd`.
///
/// EVERY READ DEGRADES ON ITS OWN. A repository with no `origin` still names
/// its branch and its worktree; a gh-axi that is not installed costs the PR
/// line and nothing else. The one thing that is never guessed is a pull
/// request number.
pub fn git_facts(cwd: &str) -> GitFacts {
    let trunk = trunk(cwd);
    GitFacts {
        worktree: git(cwd, &["rev-parse", "--show-toplevel"]).unwrap_or_default(),
        stack: stack(cwd, &trunk)
            .into_iter()
            .map(|name| Branch {
                pull_request: pull_request(cwd, &name),
                name,
            })
            .collect(),
        changes: changes(cwd, &trunk),
        trunk,
    }
}

/// The trunk this repository's own remote points at, or `main`.
///
/// ASKED RATHER THAN ASSUMED, because a tool other people install cannot
/// hardcode one repository's branch name. `origin/HEAD` is what `git clone`
/// sets and what `git remote set-head` repairs; a checkout that has neither
/// gets the default the layout names.
fn trunk(cwd: &str) -> String {
    git(
        cwd,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    )
    .and_then(|head| head.rsplit('/').next().map(str::to_string))
    .filter(|trunk| !trunk.is_empty())
    .unwrap_or_else(|| DEFAULT_TRUNK.to_string())
}

/// The stack, bottom first, with the current branch last.
///
/// TWO LISTINGS AND NO PER-BRANCH SPAWN for the membership question: what is
/// merged into HEAD, less what is already in the trunk, is what sits between
/// them. The ORDER then costs one count per stack branch, which is one or two
/// in practice.
///
/// FAIL CLOSED ON THE TRUNK LISTING. Without it, every stale local branch
/// merged into HEAD would read as part of this stack, so a listing that failed
/// leaves the current branch alone rather than a stack nobody built.
fn stack(cwd: &str, trunk: &str) -> Vec<String> {
    let current = crate::git::git_branch(cwd);
    let below = match (
        merged_into(cwd, "HEAD"),
        merged_into(cwd, &format!("{ORIGIN}/{trunk}")),
    ) {
        (Some(head), Some(in_trunk)) => head
            .into_iter()
            .filter(|name| *name != current && !in_trunk.contains(name))
            .collect(),
        _ => Vec::new(),
    };
    let mut ordered: Vec<(usize, String)> = below
        .into_iter()
        .map(|name| (commits_ahead(cwd, trunk, &name), name))
        .collect();
    ordered.sort();
    let mut stack: Vec<String> = ordered.into_iter().map(|(_, name)| name).collect();
    if !current.is_empty() {
        stack.push(current);
    }
    stack
}

/// The local branches whose tip is an ancestor of `commit`.
fn merged_into(cwd: &str, commit: &str) -> Option<Vec<String>> {
    Some(
        git(
            cwd,
            &["branch", "--merged", commit, "--format=%(refname:short)"],
        )?
        .lines()
        .map(str::to_string)
        .filter(|name| !name.is_empty())
        .collect(),
    )
}

/// How far one branch sits above the trunk, which is what orders the stack.
fn commits_ahead(cwd: &str, trunk: &str, branch: &str) -> usize {
    git(
        cwd,
        &[
            "rev-list",
            "--count",
            &format!("{ORIGIN}/{trunk}..{branch}"),
        ],
    )
    .and_then(|count| count.parse().ok())
    .unwrap_or(usize::MAX)
}

/// The diff against the trunk, or None when it could not be read.
///
/// THREE DOTS, which is the layout's own command: the diff since the branch
/// left the trunk, so commits the trunk gained afterwards are not reported as
/// this branch's work.
fn changes(cwd: &str, trunk: &str) -> Option<Vec<Change>> {
    let listing = git(
        cwd,
        &["diff", "--name-status", &format!("{ORIGIN}/{trunk}...HEAD")],
    )?;
    Some(listing.lines().filter_map(change).collect())
}

/// One `--name-status` row. A rename carries both ends; every other status
/// carries one path and the second field is the whole answer.
fn change(row: &str) -> Option<Change> {
    let mut fields = row.split('\t');
    let status = fields.next()?.chars().next()?;
    let path = fields.next()?.to_string();
    Some(Change {
        status,
        path,
        renamed_to: fields.next().map(str::to_string),
    })
}

/// What gh-axi says about one branch's pull request.
///
/// `pr view` TAKES A NUMBER, so the branch is resolved through `pr list --head`
/// (MEASURED 2026-09-14: `gh-axi pr view --help` names no `--json` flag and no
/// branch form). That is still gh-axi answering rather than pns guessing,
/// which is the rule the layout states.
///
/// THROUGH `npx`, LIKE EVERY OTHER CALLER OF IT ON THIS MACHINE. gh-axi is not
/// installed as a binary, and `npx -y` resolves it from the cache in around two
/// seconds (MEASURED). A context whose PATH carries no `npx` reads as
/// unavailable, which costs this one line.
fn pull_request(cwd: &str, branch: &str) -> PullRequestLookup {
    let mut command = Command::new(NPX);
    command.args([
        "-y", "gh-axi", "pr", "list", "--head", branch, "--state", "all", "--limit", "1",
    ]);
    if !cwd.is_empty() {
        command.current_dir(cwd);
    }
    let Some(listing) = run_bounded(command, None, AXI_DEADLINE, AXI_READ_MAX) else {
        return PullRequestLookup::Unavailable;
    };
    listing::listed(&listing)
}

/// One git read, trimmed, or None when git refused or could not be run.
fn git(cwd: &str, arguments: &[&str]) -> Option<String> {
    if cwd.is_empty() || !std::path::Path::new(cwd).is_dir() {
        return None;
    }
    let mut command = Command::new("git");
    command.args(["-C", cwd]).args(arguments);
    run_bounded(command, None, GIT_DEADLINE, GIT_READ_MAX).map(|read| read.trim().to_string())
}

/// The remote every bound above is stated against, which is the one the layout
/// names.
const ORIGIN: &str = "origin";
/// What the trunk is when `origin/HEAD` says nothing.
const DEFAULT_TRUNK: &str = "main";
/// The listing tool, resolved through PATH. See `pull_request`.
const NPX: &str = "npx";
/// How long a git read may take. Every one of them is local, so anything
/// slower than this is a wedged repository rather than an answer.
const GIT_DEADLINE: Duration = Duration::from_secs(10);
/// How much of a git read is kept. A branch's whole diff against the trunk is
/// one path per line; this is thousands of them.
const GIT_READ_MAX: u64 = 512 * 1024;
/// How long the pull-request listing may take. SIXTY SECONDS, because `npx`
/// may have to fetch gh-axi before it can run it; the warm call MEASURED 2.2
/// seconds.
const AXI_DEADLINE: Duration = Duration::from_secs(60);
/// How much of the listing is kept. One row plus its help lines.
const AXI_READ_MAX: u64 = 64 * 1024;
