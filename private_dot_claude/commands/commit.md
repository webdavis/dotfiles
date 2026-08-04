---
description: Stage one logical unit of work and create a single conventional commit. No push.
allowed-tools:
  - Bash(git status:*)
  - Bash(git diff:*)
  - Bash(git add:*)
  - Bash(git commit:*)
  - Bash(git log:*)
  - Bash(git rev-parse:*)
  - Skill
  - Read
---

# Commit

Create ONE conventional commit from the current changes. Never push.

## Steps

1. Run `git status --short` and `git diff --stat` to see what is uncommitted.
1. Decide whether the changes are one logically distinct unit of work. If they are more than one,
   list the units, ask which unit to commit now, and stage only that unit. Never fold unrelated
   units into a single commit.
1. Stage by path with `git add <path>...`. Use `git add -A` only when the whole working tree is the
   one unit, and say so when you do.
1. Invoke the `conventional-commits` skill and write the message to its specification: a
   `type(scope): subject` line, an optional body saying why, optional footers.
1. Commit with `git commit -m "<subject>"`, adding further `-m` arguments for body paragraphs.
1. Report the new commit's short SHA and subject line.

## Safeguards

- The message never carries a `Co-Authored-By` trailer, any Claude or Anthropic co-author line, or a
  "Generated with Claude Code" footer. The commit reads as the operator's own work.
- No em-dashes in the subject, the body, or the footers.
- Bail when nothing is staged and nothing can be staged; report the empty tree instead of committing.
- Do not amend, reset, rebase, or push. This command only ever adds one commit.
- If a hook rejects the commit, report its output verbatim and stop. Never retry with `--no-verify`.
