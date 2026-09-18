---
name: morning
description: Print the day's brief with the `morning` binary and read it back as where to start today. Use when the operator says "morning", asks where to start today, or asks for the day's brief, the overnight state, or what is waiting on them.
---

# Morning

`morning` is the one command that answers "where do I start today". It prints one framed page and
exits: the last apply's result and date, the applies the ledger says are owed, the open pull requests
with their CI state, the newest overnight recap, the operator's own items, and today's tasks. Every
source is named in `~/.config/morning/config.toml`; the binary reads them and nothing else.

## Steps

1. Run `morning`.
1. Quote the page verbatim, so the operator sees exactly what it said.
1. Then say, in at most five lines, where to start: the one thing that blocks the most, anything red
   (a failed apply, a failing check), and anything only the operator can do.
1. Name every section that printed `unavailable` and what it said, rather than answering as if the
   source had been read.

## Safeguards

- `morning` reads and prints. It never applies, merges or edits anything, and neither does this
  skill: do not run `chezmoi apply`, merge a pull request, or complete a task off the back of it.
- Do not go fetch a source `morning` could not read. An unavailable section is the answer for this
  page; whether to chase it is the operator's call.
- The page carries no secret and needs none. `gh` and `td` are spawned with their own credentials.
