---
description: Commit, push the branch, and open a pull request through gh-axi with a reviewed body.
allowed-tools:
  - Bash(git status:*)
  - Bash(git diff:*)
  - Bash(git add:*)
  - Bash(git commit:*)
  - Bash(git push:*)
  - Bash(git switch:*)
  - Bash(git branch:*)
  - Bash(git log:*)
  - Bash(git rev-parse:*)
  - Bash(npx -y gh-axi:*)
  - Skill
  - Read
  - Write
---

# Commit, Push, PR

Commit the current work, push the branch, and open a pull request. Every GitHub operation runs
through `npx -y gh-axi`. The bare `gh` CLI is never invoked directly; it exists only as gh-axi's
runtime dependency.

## Steps

1. Run `git rev-parse --abbrev-ref HEAD`. When the branch is `main`, create a topic branch first with
   `git switch -c <type>/<short-slug>` named after the change. Never commit to `main`.
1. Stage one logical unit by path, invoke the `conventional-commits` skill for the message, and
   commit. The message never carries a `Co-Authored-By` trailer, any Claude or Anthropic co-author
   line, or a "Generated with Claude Code" footer.
1. Push with `git push -u origin <branch>`.
1. Draft the pull request body into a file (for example `/tmp/pr-body.md`) using the template in the
   next section. Do not call gh-axi yet.

## Pull request body template

Write all five sections, in this order, into the body file:

```
## Context

## Summary

## How it was verified

## Effect of merging

## Review guide
```

- `## Context` says why the change exists: the problem and what prompted it. Two short paragraphs at
  most, with any shorthand defined where it first appears.
- `## Summary` says what changed: three to five short bullets, each one self-contained, so deleting
  any bullet strips no context another bullet needs. Name key files by path. Add a `## What changed`
  section only when the pull request is too large for `## Summary` to carry it.
- `## How it was verified` lists the commands that were run and what they reported. Evidence, never
  "should work".
- `## Effect of merging` says what merging does and what it does not do, including whether anything
  changes on a live machine.
- `## Review guide` is triage, not enumeration: the reading order, which files are load-bearing and
  which are mechanical churn, a behavioral note per load-bearing file, and where a second pair of
  eyes helps most. On a small pull request one or two lines is the right length. It never describes
  how the change was produced internally.

## Review the draft before posting

This step is mandatory and it runs before any gh-axi call. Re-read the drafted body and fix every
violation of the two lists below, then re-read it once more.

Structure:

- All five headings are present, spelled exactly as above, in that order.
- Every `## Summary` bullet stands alone.
- `## How it was verified` names real commands and real results.

Anti-AI-pattern checklist:

- No em-dashes anywhere in the body.
- No "not just X but Y" and no other negative parallelism.
- No inflated significance: no "comprehensive", "robust", "seamless", "crucial", "game-changing".
- No vague attribution: no "studies show", "it is widely considered", "best practice suggests".
- No rule-of-three padding: three items only when there are exactly three things.
- Never speak about the audience in the third person. Imperative directions such as "see the
  coverage matrix" are fine; "a reader will find" is not.

## Post the pull request

Only once the review above passes:

```
npx -y gh-axi pr create --title "<conventional subject>" --body-file /tmp/pr-body.md
```

Pass the body with `--body-file`. Never pass it as an inline argument, which mangles newlines.
Report the pull request number and URL.

## Safeguards

- Never post an unreviewed body. If the review step was skipped, redo it before creating anything.
- Never invoke `gh` directly for any of this, and never allowlist it.
- Never push to `main`, and never force-push to `main` under any circumstances.
- A force-push to a topic branch uses `--force-with-lease`, never `--force`, and needs
  per-invocation confirmation from the operator. A yes given earlier does not carry over.
- If a hook rejects the commit or the push, report its output verbatim and stop. Never retry with
  `--no-verify`.
- Do not merge the pull request. Opening it is where this command stops.
