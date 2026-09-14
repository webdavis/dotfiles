//! The recap's Git block, its stack graph and its file list, rendered from
//! facts somebody else read.
//!
//! POLICY ONLY, like every other module here: no spawn, no repository, no
//! network. `pns_adapters::recap::git_facts` runs git, worktrunk and `gh`;
//! this decides what the three of them are SAID as.
//!
//! THE LAYOUT IS NOT THIS MODULE'S TO CHOOSE. It is the operator's, stated in
//! the "Work recaps" section of the shared agent rules, and the whole reason
//! this exists is that an agent composing it by hand got the stale parts of it
//! wrong. Every string below is that layout.

mod facts;
pub use facts::{Branch, Change, GitFacts, PullRequest, PullRequestLookup};

use std::fmt::Write as _;

/// The Git block, then the stack graph and the file list inside ONE fence.
///
/// ONE FENCE IS A READABILITY RULE, not a formatting preference: the tree's
/// indentation and the file list's status column only line up under a
/// monospace run, in a terminal and in Discord alike.
pub fn git_block(facts: &GitFacts) -> String {
    // NO BRANCH AND NO WORKTREE IS NOT A DETACHED HEAD, it is somewhere git
    // answered nothing at all, and the ordinary lines then contradict each
    // other: no branch on two of them and the trunk named as both the stack and
    // the current row on the rest. Said once, with no graph and no file list to
    // disagree with it.
    if facts.stack.is_empty() && facts.worktree.is_empty() {
        return format!(
            "**Git**\n- Branch: {NO_REPOSITORY}\n- Worktree: {NO_REPOSITORY}\n\
             - PR: {NO_REPOSITORY}\n- Stack: {NO_REPOSITORY}\n"
        );
    }
    let mut block = String::from("**Git**\n");
    let current = facts.stack.last();
    let _ = writeln!(block, "- Branch: {}", branch_line(facts));
    let _ = writeln!(block, "- Worktree: `{}` (kept)", facts.worktree);
    let _ = writeln!(
        block,
        "- PR: {}",
        current.map_or_else(|| DETACHED.to_string(), pull_request_line)
    );
    let _ = writeln!(block, "- Stack: {}", stack_line(facts));
    let _ = write!(block, "\n{FENCE}\n{}", graph(facts));
    let _ = write!(
        block,
        "\n{}{FENCE}\n",
        file_list(facts.changes.as_deref(), &facts.trunk)
    );
    block
}

/// The counts a collapsed file list says instead of its rows, in the layout's
/// own order and spacing (`A 3  M 4  D 1`).
///
/// SHARED WITH THE AGENT RECAP'S FIT, which collapses the same list for the
/// same reason on a body that is over the character ceiling. One spelling of
/// one rule.
pub fn status_counts(statuses: impl Iterator<Item = char>) -> String {
    let mut tallied: Vec<(char, usize)> = Vec::new();
    for status in statuses {
        match tallied.iter_mut().find(|(letter, _)| *letter == status) {
            Some((_, count)) => *count += 1,
            None => tallied.push((status, 1)),
        }
    }
    // THE LAYOUT'S OWN ORDER FIRST, then whatever else git said, so the
    // ordinary line reads the way the rule writes it and an unusual letter is
    // still counted rather than dropped.
    tallied.sort_by_key(|(letter, _)| {
        STATUS_ORDER
            .iter()
            .position(|known| known == letter)
            .unwrap_or(STATUS_ORDER.len())
    });
    tallied
        .iter()
        .map(|(letter, count)| format!("{letter} {count}"))
        .collect::<Vec<_>>()
        .join("  ")
}

/// How many rows a file list may carry before it is worth its counts instead.
///
/// TWENTY, measured against the budget rather than against taste: twenty rows
/// of this repository's own paths spend around 900 characters, which is half of
/// one Discord message before the recap has said anything about the work.
pub const COLLAPSE_ABOVE: usize = 20;

/// The `- Branch:` line: the branch, where its pull request stands, and where
/// it sits in the stack.
fn branch_line(facts: &GitFacts) -> String {
    let Some(branch) = facts.stack.last() else {
        return DETACHED.to_string();
    };
    format!(
        "`{}` ({}, {} of {} in stack)",
        branch.name,
        match &branch.pull_request {
            PullRequestLookup::Found(request) => request.state.clone(),
            PullRequestLookup::Absent => "no PR".to_string(),
            PullRequestLookup::Unavailable => "PR unknown".to_string(),
        },
        facts.stack.len(),
        facts.stack.len()
    )
}

/// The `- PR:` line, which never invents a number. See `PullRequestLookup`.
fn pull_request_line(branch: &Branch) -> String {
    match &branch.pull_request {
        PullRequestLookup::Found(request) => format!("#{} ({})", request.number, request.state),
        PullRequestLookup::Absent => "none".to_string(),
        PullRequestLookup::Unavailable => "unknown (gh did not answer)".to_string(),
    }
}

/// The `- Stack:` line: the branch at the bottom names the stack, and the
/// count is pull requests rather than branches, because that is what a reader
/// of the graph can follow.
fn stack_line(facts: &GitFacts) -> String {
    let requests = facts
        .stack
        .iter()
        .filter(|branch| matches!(branch.pull_request, PullRequestLookup::Found(_)))
        .count();
    let name = facts
        .stack
        .first()
        .map_or_else(|| facts.trunk.clone(), |branch| branch.name.clone());
    format!(
        "`{name}` ({requests} PR{}, trunk {})",
        if requests == 1 { "" } else { "s" },
        facts.trunk
    )
}

/// The trunk, then one row per branch above it, each indented under the last.
fn graph(facts: &GitFacts) -> String {
    let rows: Vec<&Branch> = facts
        .stack
        .iter()
        .filter(|branch| branch.name != facts.trunk)
        .collect();
    let mut graph = format!(" Stack Graph\n -----------\n  `{}` (*trunk*)", facts.trunk);
    if rows.is_empty() {
        // HEAD IS THE TRUNK, or detached above nothing. The trunk row is the
        // whole graph and it is what `← *current*` belongs on.
        let _ = write!(graph, "{CURRENT}");
    }
    for (depth, branch) in rows.iter().enumerate() {
        let last = depth + 1 == rows.len();
        let _ = write!(
            graph,
            "\n{:indent$}└─ {}{}",
            "",
            row(branch),
            if last { CURRENT } else { "" },
            indent = 2 + 3 * depth
        );
    }
    graph.push('\n');
    graph
}

/// One branch's own columns: the name, the receipt, where it stands, and the
/// mark a reader scans for.
fn row(branch: &Branch) -> String {
    let mut columns = vec![format!("`{}`", branch.name)];
    match &branch.pull_request {
        PullRequestLookup::Found(request) => {
            columns.push(format!("#{}", request.number));
            columns.push(format!("*{}*", request.state));
            columns.push(
                if request.state == "merged" {
                    "✓"
                } else {
                    "×"
                }
                .to_string(),
            );
        }
        PullRequestLookup::Absent => {
            columns.push("(no PR)".to_string());
            columns.push("×".to_string());
        }
        PullRequestLookup::Unavailable => {
            columns.push("(PR unknown)".to_string());
            columns.push("×".to_string());
        }
    }
    columns.join("  ")
}

/// The file list: one row per path, its counts once it runs long, or one line
/// saying the diff could not be read.
fn file_list(changes: Option<&[Change]>, trunk: &str) -> String {
    let Some(changes) = changes else {
        return format!("(the diff against {ORIGIN}/{trunk} could not be read)\n");
    };
    if changes.is_empty() {
        return format!("(no files changed against {ORIGIN}/{trunk})\n");
    }
    if changes.len() > COLLAPSE_ABOVE {
        return format!(
            "{}\n",
            status_counts(changes.iter().map(|change| change.status))
        );
    }
    changes
        .iter()
        .map(|change| match &change.renamed_to {
            Some(landed) => format!("{}  {} -> {landed}\n", change.status, change.path),
            None => format!("{}  {}\n", change.status, change.path),
        })
        .collect()
}

/// The remote the diff the file list reports was read against, which is the
/// one the layout names and the one `recap::worktree` states every bound
/// against. THE TRUNK IS NOT A CONST beside it: it is asked for per
/// repository, so the two degraded lines name the branch the diff actually
/// ran on rather than this machine's habit.
const ORIGIN: &str = "origin";
/// The order the layout's own example writes the statuses in.
const STATUS_ORDER: [char; 4] = ['A', 'M', 'R', 'D'];
/// What marks the branch the recap is about.
const CURRENT: &str = "  ← *current*";
/// What the Branch and PR lines say when there is no branch to name.
const DETACHED: &str = "(detached HEAD)";
/// What every line says when git answered nothing at all.
const NO_REPOSITORY: &str = "(not a git repository)";
/// The one fence the graph and the file list share.
const FENCE: &str = "```";
