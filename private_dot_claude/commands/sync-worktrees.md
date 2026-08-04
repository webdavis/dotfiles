---
description: Rebase every worktree against its upstream by running worktrunk's `wt up`, then report.
allowed-tools:
  - Bash(wt:*)
  - Bash(git worktree:*)
  - Bash(git status:*)
---

# Sync Worktrees

Bring every worktree up to date with its upstream. Worktrunk already owns this operation, so this
command runs it and reports; it adds no logic of its own.

## Steps

1. Run `wt up`. That fetches with `--prune` and then rebases each worktree onto its own upstream,
   skipping any worktree with no upstream and any worktree already mid-rebase, and aborting a rebase
   that conflicts rather than leaving the worktree stuck.
1. Report the outcome per worktree, quoting worktrunk's own lines verbatim for anything it skipped
   or could not rebase.
1. Run `wt list` when the output is ambiguous, and report that instead of guessing.

## Safeguards

- Do not resolve a conflict that worktrunk backed out of. Name the worktree and the branch and stop;
  the operator picks that up in the worktree itself.
- Do not pass `--yes`, and do not add flags worktrunk was not asked for.
- Do not commit, push, or delete anything here. This command only rebases.
