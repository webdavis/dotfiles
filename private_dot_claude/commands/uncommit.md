---
description: Undo the last commit with a soft reset, keeping every change staged.
allowed-tools:
  - Bash(git status:*)
  - Bash(git diff:*)
  - Bash(git log:*)
  - Bash(git reset:*)
  - Bash(git branch:*)
  - Bash(git rev-parse:*)
  - Bash(git rev-list:*)
---

# Uncommit

Move the last commit back into the staging area. The changes survive; only the commit goes away.

## Steps

1. Run `git log -1 --format='%h %s'` and show which commit is about to be undone.
1. Decide whether the commit is published: `git rev-parse --abbrev-ref '@{u}'` names the upstream,
   and `git rev-list --count '@{u}'..HEAD` reports how many local commits sit above it. A count of
   zero means HEAD is already on the remote, so undoing it rewrites published history.
1. When HEAD is published, stop and say so. Removing it needs a follow-up force-push, so it takes
   per-invocation confirmation from the operator. A yes given earlier does not carry over. Wait for
   an explicit answer before touching anything.
1. Run `git reset --soft HEAD~1`.
1. Run `git status --short` and report what is now staged, so the recovered changes are visible.

## Safeguards

- `--soft` only. Never `git reset --hard` and never `git reset --mixed` from this command; both
  throw away work that no commit holds any more.
- Never uncommit on `main`.
- Never run the force-push yourself. When one is needed it is `git push --force-with-lease`, never
  `--force`, and it is the operator's call.
- Bail when HEAD is a merge commit, when a rebase or cherry-pick is in progress, or when HEAD is the
  repository's only commit, and say which one blocked it.
- Undo exactly one commit. For more than one, say how many and let the operator confirm the depth.
