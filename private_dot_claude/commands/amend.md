---
description: Amend the last commit, refusing to rewrite pushed history without explicit confirmation.
allowed-tools:
  - Bash(git status:*)
  - Bash(git diff:*)
  - Bash(git add:*)
  - Bash(git commit:*)
  - Bash(git log:*)
  - Bash(git branch:*)
  - Bash(git rev-parse:*)
  - Bash(git rev-list:*)
  - Skill
  - Read
---

# Amend

Fold the current changes into the last commit, keeping or regenerating its conventional message.

## Steps

1. Run `git log -1 --format='%h %s'` and `git status --short` so the target commit and the pending
   changes are both on the table.
1. Decide whether the commit is published: `git rev-parse --abbrev-ref '@{u}'` names the upstream,
   and `git rev-list --count '@{u}'..HEAD` reports how many local commits sit above it. A count of
   zero means HEAD is already on the remote, so amending rewrites published history.
1. When HEAD is published, stop and say so. Amending it needs a follow-up force-push, so it takes
   per-invocation confirmation from the operator. A yes given earlier does not carry over. Wait for
   an explicit answer before touching anything.
1. Stage the changes that belong in the commit, by path.
1. Amend. Keep the existing message with `git commit --amend --no-edit` when the subject still
   describes the commit. When it no longer does, invoke the `conventional-commits` skill, write a
   fresh message, and pass it with `git commit --amend -m "<subject>"`.
1. Report the rewritten commit's short SHA and subject, and, when it was published, the exact
   force-push command the operator now needs.

## Safeguards

- The message never carries a `Co-Authored-By` trailer, any Claude or Anthropic co-author line, or a
  "Generated with Claude Code" footer.
- No em-dashes in the message.
- Never amend a commit on `main`.
- Never run the force-push yourself as part of this command. When one is needed it is
  `git push --force-with-lease`, never `--force`, and it is the operator's call.
- Bail when HEAD is a merge commit, when a rebase or cherry-pick is in progress, or when the
  repository has no commits yet. Report which one blocked it.
