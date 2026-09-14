//! The recap's Git block, pinned: what git, worktrunk and `gh` are rendered
//! into so a skill can paste it instead of composing it by hand.

use crate::recap::git_block::{
    Branch, COLLAPSE_ABOVE, GitFacts, PullRequest, PullRequestLookup, git_block,
};

fn change(status: char, path: &str) -> crate::recap::git_block::Change {
    crate::recap::git_block::Change {
        status,
        path: path.to_string(),
        renamed_to: None,
    }
}

fn branch(name: &str, lookup: PullRequestLookup) -> Branch {
    Branch {
        name: name.to_string(),
        pull_request: lookup,
    }
}

fn facts(stack: Vec<Branch>, changes: Option<Vec<crate::recap::git_block::Change>>) -> GitFacts {
    GitFacts {
        worktree: "/Users/stephen/.herdr/worktrees/dotfiles/feat-pns-recap-agent".to_string(),
        trunk: "main".to_string(),
        stack,
        changes,
    }
}

fn open(number: u64) -> PullRequestLookup {
    PullRequestLookup::Found(PullRequest {
        number,
        state: "open".to_string(),
    })
}

#[test]
fn a_branch_with_a_pull_request_names_its_number_and_its_state() {
    let block = git_block(&facts(
        vec![branch("feat/pns-recap-agent", open(591))],
        Some(vec![change('M', "pns/crates/pns/src/command_recap.rs")]),
    ));
    assert!(
        block.contains("- Branch: `feat/pns-recap-agent` (open, 1 of 1 in stack)"),
        "{block}"
    );
    assert!(block.contains("- PR: #591 (open)"), "{block}");
    assert!(
        block.contains("- Stack: `feat/pns-recap-agent` (1 PR, trunk main)"),
        "{block}"
    );
    assert!(
        block.contains("M  pns/crates/pns/src/command_recap.rs"),
        "{block}"
    );
}

#[test]
fn a_branch_with_no_pull_request_says_none_and_never_a_number() {
    // NEVER GUESS A PR NUMBER is the layout's own rule, and this is the case
    // it was written for: `gh` answered, and its answer was that there is no
    // pull request yet.
    let block = git_block(&facts(
        vec![branch("feat/pns-recap-agent", PullRequestLookup::Absent)],
        Some(Vec::new()),
    ));
    assert!(block.contains("- PR: none"), "{block}");
    assert!(
        block.contains("- Branch: `feat/pns-recap-agent` (no PR, 1 of 1 in stack)"),
        "{block}"
    );
    assert!(
        block.contains("- Stack: `feat/pns-recap-agent` (0 PRs, trunk main)"),
        "{block}"
    );
    assert!(!block.contains('#'), "a number was invented: {block}");
}

#[test]
fn a_listing_nobody_could_run_is_unknown_rather_than_none() {
    // TWO DIFFERENT FACTS, AND THE LAYOUT HAS ONE WORD FOR ONLY ONE OF THEM.
    // "none" is `gh` saying there is no pull request; a `gh` that never
    // ran said nothing at all, and printing "none" for it is the guess the
    // rule forbids.
    let block = git_block(&facts(
        vec![branch(
            "feat/pns-recap-agent",
            PullRequestLookup::Unavailable,
        )],
        Some(Vec::new()),
    ));
    assert!(
        block.contains("- PR: unknown (gh did not answer)"),
        "{block}"
    );
}

#[test]
fn a_two_deep_stack_draws_the_trunk_then_each_branch_under_the_last() {
    let block = git_block(&facts(
        vec![
            branch(
                "feat/pns-recap-agent",
                PullRequestLookup::Found(PullRequest {
                    number: 591,
                    state: "merged".to_string(),
                }),
            ),
            branch("feat/pns-recap-git", open(592)),
        ],
        Some(Vec::new()),
    ));
    let graph: Vec<&str> = block
        .lines()
        .skip_while(|line| line.trim() != "Stack Graph")
        .take(5)
        .collect();
    assert_eq!(
        graph,
        [
            " Stack Graph",
            " -----------",
            "  `main` (*trunk*)",
            "  └─ `feat/pns-recap-agent`  #591  *merged*  ✓",
            "     └─ `feat/pns-recap-git`  #592  *open*  ×  ← *current*",
        ],
        "{block}"
    );
    // THE CURRENT BRANCH IS THE TOP OF THE STACK, which is what the Git block
    // above the graph is about.
    assert!(
        block.contains("- Branch: `feat/pns-recap-git` (open, 2 of 2 in stack)"),
        "{block}"
    );
    assert!(
        block.contains("- Stack: `feat/pns-recap-agent` (2 PRs, trunk main)"),
        "{block}"
    );
}

#[test]
fn the_stack_graph_and_the_file_list_share_one_fenced_block() {
    // THE READABILITY RULE, LITERALLY: one fence, so the tree and the status
    // column stay aligned in a terminal and in Discord alike.
    let block = git_block(&facts(
        vec![branch("feat/pns-recap-agent", open(591))],
        Some(vec![
            change('A', "pns/crates/pns-domain/src/recap/git_block.rs"),
            change('D', "pns/crates/pns/src/legacy.rs"),
        ]),
    ));
    let fences: Vec<usize> = block
        .lines()
        .enumerate()
        .filter(|(_, line)| line.trim() == "```")
        .map(|(index, _)| index)
        .collect();
    assert_eq!(fences.len(), 2, "{block}");
    let inside = &block.lines().collect::<Vec<_>>()[fences[0]..fences[1]];
    assert!(
        inside.iter().any(|line| line.trim() == "Stack Graph"),
        "{block}"
    );
    assert!(
        inside.contains(&"A  pns/crates/pns-domain/src/recap/git_block.rs"),
        "{block}"
    );
}

#[test]
fn a_rename_names_both_ends_and_a_diff_nobody_could_read_says_so() {
    let renamed = crate::recap::git_block::Change {
        status: 'R',
        path: "pns/crates/pns/src/command_recap.rs".to_string(),
        renamed_to: Some("pns/crates/pns/src/command_recap/window.rs".to_string()),
    };
    let block = git_block(&facts(
        vec![branch("feat/pns-recap-agent", open(591))],
        Some(vec![renamed]),
    ));
    assert!(
        block.contains(
            "R  pns/crates/pns/src/command_recap.rs -> pns/crates/pns/src/command_recap/window.rs"
        ),
        "{block}"
    );

    // THE TRUNK IT WAS HANDED, never the literal `main`: the adapter asks
    // every repository for its own, and a clone whose trunk is `master` gets a
    // diff read against `origin/master`, so a line naming `origin/main` would
    // report a branch nothing was compared with.
    let mut elsewhere = facts(vec![branch("feat/x", open(1))], None);
    elsewhere.trunk = "master".to_string();
    let unreadable = git_block(&elsewhere);
    assert!(
        unreadable.contains("(the diff against origin/master could not be read)"),
        "{unreadable}"
    );
    elsewhere.changes = Some(Vec::new());
    let nothing = git_block(&elsewhere);
    assert!(
        nothing.contains("(no files changed against origin/master)"),
        "{nothing}"
    );
}

#[test]
fn a_file_list_past_the_collapse_line_says_its_counts_instead_of_its_rows() {
    // THE LAYOUT'S OWN RULE, on the command that generates the list rather
    // than on the fit that posts it: "collapse a long file list to counts per
    // status rather than truncating mid-list".
    let changes: Vec<crate::recap::git_block::Change> = (0..=COLLAPSE_ABOVE)
        .map(|index| change(['A', 'M', 'D'][index % 3], &format!("src/file{index}.rs")))
        .collect();
    let block = git_block(&facts(
        vec![branch("feat/pns-recap-agent", open(591))],
        Some(changes),
    ));
    assert!(block.contains("A 7  M 7  D 7"), "{block}");
    assert!(!block.contains("file20.rs"), "{block}");

    // AND ONE ROW UNDER THE LINE IS STILL A LIST. The threshold is a
    // threshold, not a taste.
    let short: Vec<crate::recap::git_block::Change> = (0..COLLAPSE_ABOVE)
        .map(|index| change('M', &format!("src/file{index}.rs")))
        .collect();
    let block = git_block(&facts(
        vec![branch("feat/pns-recap-agent", open(591))],
        Some(short),
    ));
    assert!(block.contains("M  src/file19.rs"), "{block}");
}

#[test]
fn somewhere_git_answered_nothing_says_that_once_instead_of_contradicting_itself() {
    // NOT REACHABLE FROM THE DOCUMENTED USE (the verb is run inside the
    // worktree), but it is output an agent would paste verbatim, and the four
    // ordinary lines used to disagree with each other: no branch on two of
    // them and `main` named as the stack and marked current on the rest.
    let block = git_block(&GitFacts {
        worktree: String::new(),
        trunk: "main".to_string(),
        stack: Vec::new(),
        changes: None,
    });
    assert_eq!(
        block,
        "**Git**\n\
         - Branch: (not a git repository)\n\
         - Worktree: (not a git repository)\n\
         - PR: (not a git repository)\n\
         - Stack: (not a git repository)\n"
    );
}
