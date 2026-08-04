---
description: Prune local branches whose upstream is gone, worktrunk-aware, deleting nothing unconfirmed.
allowed-tools:
  - Bash(git fetch:*)
  - Bash(git branch:*)
  - Bash(git worktree:*)
  - Bash(git status:*)
  - Bash(git rev-parse:*)
  - Bash(wt:*)
---

# Clean Gone Branches

Delete local branches whose remote tracking branch is gone, without fighting worktrunk.

Worktrunk owns the worktrees in this repository and sets `delete-branch = false`, so a branch ref
that outlives its merged pull request is deliberate, not leftover garbage. Treat that config as
correct and never change it from here.

## Steps

1. Run `git fetch --all --prune`.
1. Run `git branch -vv` and collect every branch whose upstream is marked `[gone]`. Drop the current
   branch and `main` from that list unconditionally.
1. Run `git worktree list` and split the remaining branches into two groups: those with no worktree,
   and those checked out in a worktree.
1. Present both groups with the exact command proposed for each branch:
   - no worktree: `git branch -D <branch>`
   - has a worktree: `wt remove <branch> --force-delete`, which is worktrunk's own teardown and
     removes the worktree along with the branch
1. Ask the operator to confirm, and name the branches in the question. This is a destructive action
   and it takes per-invocation confirmation; a yes given earlier in the session does not carry over.
   Wait for an explicit answer.
1. Run only the deletions that were confirmed, one command at a time, and report each result.

## Safeguards

- Never delete the current branch, `main`, or a branch whose upstream is not `[gone]`.
- Never run `git worktree remove --force`, `git branch -D`, or `wt remove --force-delete` before the
  confirmation in step 5. Each of those discards work that no remote holds.
- A branch checked out in a worktree cannot be deleted by `git branch -D`. Route it through `wt`
  rather than forcing git past the worktree.
- Report a dirty worktree instead of forcing removal past it, and let the operator decide.
- Do not edit worktrunk's config, and do not pass `--yes` to `wt` on the operator's behalf.
