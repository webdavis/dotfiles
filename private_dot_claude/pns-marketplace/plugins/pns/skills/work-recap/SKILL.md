---
name: work-recap
description: Produce the structured work recap of the current branch, worktree, PR and task state in the shared Recap layout. Use when the user says /pns:work-recap or asks for a recap, progress summary, or end-of-day summary.
---

# Work recap

The layout, the readability rules and the delivery rule all live in the
**Work recaps** section of `~/.claude/CLAUDE.md`. Read that section and follow
it there; it is the single source for the shape of a recap. This skill only
covers how to fill it in.

## Gather the Git facts

Run these in the worktree the work happened in, never from memory:

```bash
git rev-parse --abbrev-ref HEAD
git rev-parse --show-toplevel
git diff --name-status origin/main...HEAD
git log --oneline origin/main..HEAD
npx -y gh-axi pr view --json number,state,title
```

`gh-axi` is the only way to a PR number. If `pr view` reports no pull request,
the PR line is `none`; never guess a number.

## Fill every section

Every section of the layout appears, in order, even when its answer is short:

- **Git** and the stack graph from the commands above.
- The file list from `git diff --name-status`, collapsed to counts per status
  once it runs long.
- **Summary** from the work itself, one line per thing done, each carrying its
  state.
- **In-Progress**, with a `Blocked on:` line that says `nothing` when nothing
  blocks it.
- **Upcoming Agent Tasks**, naming any gate each one waits on.
- **User Tasks**, numbered, each the exact command or decision the operator
  owes.

An empty section says so in one line rather than being dropped.

## Print it

The recap goes in the chat reply. Nothing forwards it anywhere yet, so do not
say it was posted.
