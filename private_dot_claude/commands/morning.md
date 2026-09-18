---
description: Print the day's brief with the `morning` binary and read it back as where to start today.
allowed-tools:
  - Bash(morning:*)
---

# Morning

Hand me the day's brief. `morning` already knows where everything lives, so this command runs it and
reads the page; it adds no sources of its own.

## Steps

1. Run `morning`. It prints one framed page: the last apply's result and date, the applies the ledger
   says are owed, the open pull requests with their CI state, the newest overnight recap, the
   operator's own items, and today's tasks.
1. Quote the page verbatim first, so the operator sees exactly what it said.
1. Then say, in at most five lines, where to start: the one thing that blocks the most, anything red
   (a failed apply, a failing check), and anything only the operator can do.
1. Name every section that printed `unavailable` and what it said, rather than answering as if the
   source had been read.

## Safeguards

- `morning` reads and prints. It never applies, merges or edits anything, and neither does this
  command: do not run `chezmoi apply`, merge a pull request, or complete a task off the back of it.
- Do not go fetch a source `morning` could not read. An unavailable section is the answer for this
  page; whether to chase it is the operator's call.
- Do not edit the ledger, and do not open or close Todoist tasks here.
