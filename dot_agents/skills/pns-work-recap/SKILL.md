---
name: pns-work-recap
description: Produce the structured work recap of the current branch, worktree, PR and task state in the shared Recap layout. Use when the user says "/pns-work-recap" (spelled "/pns:work-recap" in Claude Code) or asks for a recap, progress summary, or end-of-day summary.
---

# Work recap

The layout, the readability rules and the delivery rule all live in the
**Work recaps** section of `~/.claude/CLAUDE.md`. Read that section and follow
it there; it is the single source for the shape of a recap. This skill only
covers how to fill it in.

## Gather the Git facts

Run this in the worktree the work happened in, never from memory:

```bash
pns recap git
```

It prints the **Git** block and the fenced block holding the stack graph and
the file list, already in the layout. Paste it; do not retype it. It reads the
branch, the worktree, the trunk, the stack and the diff from git, and the PR
number and state from `gh`, so `none` means `gh` answered that there is
no pull request and `unknown` means `gh` could not be reached. Never guess a
number in either case.

Two fields it cannot know: the worktree line always says `kept`, so correct it
to `removed` when the worktree is going away, and `git log --oneline
origin/main..HEAD` is still what tells you what the commits were about.

## Fill every section

Every section of the layout appears, in order, even when its answer is short:

- **Git**, the stack graph and the file list from `pns recap git`, which
  collapses the file list to counts per status once it runs long.
- **Summary** from the work itself, one line per thing done, each carrying its
  state.
- **In-Progress**, with a `Blocked on:` line that says `nothing` when nothing
  blocks it.
- **Upcoming Agent Tasks**, naming any gate each one waits on.
- **User Tasks**, numbered, each the exact command or decision the operator
  owes.

An empty section says so in one line rather than being dropped.

## Print it, then post it

The recap goes in the chat reply. To forward it to the `#pns-recap` Discord
channel, pipe the same text through pns:

```bash
pns recap agent --stdin <recap.md
```

It sanitizes the body and fits it under Discord's message limit, collapsing the
file list and then shedding whole sections rather than truncating a line, and
it never sheds **User Tasks**. It prints one line saying where the post landed.
Say the recap was posted only when that line says it was.
