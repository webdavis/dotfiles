---
name: follow-up
description: Run a slice that exists because a review found something, with no re-review and no new tasks, terminating in its own PR. Use when a task was created by a review, when a task says Strategy-F, or when the user says "/strategy:follow-up".
---

# Follow-up

Work that exists BECAUSE step 4a, 4b or 7 of an earlier slice found something. It is
[planned](../planned/SKILL.md) minus steps 7 and 8, plus a terminal verification step, 6v, that the
other two do not have.

Those two steps are cut for one reason, and it is the whole point of this strategy. They are the only
steps that can enqueue work for a later step: step 7 earns another fix, which earns another review;
step 8 mints tasks. Every other step resolves its own findings, so reviews are safe to stack and
DEFERRALS are not. A task created by a follow-up review would be depth two, and there is no such thing.
**Follow-up work terminates in its own PR.**

The rule that falls out, and the register enforces it mechanically: a finding is FIXED IN-ROUND or
ACCEPTED with a written rationale. It is never deferred into a new task.

Read [the ledger grammar](../../ledger-grammar.md) before ticking anything. This strategy is `F` in both
scripts.

## Open both ledgers at step 1, before any code exists

```bash
~/.claude/pipeline/slice-checklist.sh  new <slug> F [--security] [dir]
~/.claude/pipeline/findings-register.sh new <slug> F [dir]
```

The `F` register writes a `| 6v |` row in the declared-verdicts table where the `A` one writes `| 7 |`,
and it refuses a TASK disposition outright.

## The steps

| #    | step                                                                    | leaves         |
| ---- | ----------------------------------------------------------------------- | -------------- |
| 1    | Read the plan AND the spec, write the brief, and LOG it                 | brief path     |
| 2    | Review the BRIEF, before any code exists                                | quoted verdict |
| 3    | Implement: test-first, mutation-verified against an unmutated control   | commit sha     |
| 4a   | Review correctness, validator agreement, shell safety                   | quoted verdict |
| 4a-s | Review the SECURITY lens (security slices only)                         | quoted verdict |
| 4b   | Review test quality: can each assertion FAIL. Own worktree; FIXES       | quoted verdict |
| 4c   | Independent read by the orchestrator                                    | notes path     |
| 5    | Adjudicate: REPRODUCE every finding before accepting it                 | register rows  |
| 6    | Fix 4a and 4c findings only; 4b already closed its own                  | commit sha     |
| 6v   | TERMINAL verification: check the fix, run the gates, FIX in place       | quoted verdict |
| 9    | Gates, push, verify the ref landed, open PR, merge                      | PR number      |

Steps 1 through 6 behave exactly as they do under [planned](../planned/SKILL.md): the same brief gap,
the same differential-test rule, the same separate worktrees for the parallel 4a and 4b, the same
constructed mutation lens with an unmutated control, and the same code-quality section on every review
charter ranked below correctness. Read that skill for those; only the differences are below.

**A caveat on the task that sent you here.** A task filed from a review goes stale when later PRs fix
the same area. Before writing the brief, check each of its claims against current `main`. One task
asserted three things main had already fixed. Correct the task in place rather than building against it.

## Step 6v, and why it exists

Dropping step 7 means the fix would otherwise land with NOTHING checking it. 6v closes that: it confirms
every adjudicated finding is genuinely closed, confirms the fix introduced nothing new, and RUNS THE
GATES with the output pasted. It FIXES in place rather than reporting. It may NOT defer, and it is
itself never reviewed, which is what makes it terminal.

Its own vocabulary is PASS and FAIL rather than the reviewers' NO_ISSUE and NEW_ISSUE. `VERDICT: PASS`
reconciles as zero findings; FAIL deliberately does not, so a failed verification cannot read as clean.
6v is a review step for the checklist's purposes, so its evidence must quote a verdict like any other.

Two more things carry the weight step 7 used to, and neither is optional here:

- **4b FIXES what it finds** rather than reporting it. It is write-capable by design, so repairing in
  place collapses two steps at no coverage cost, and 4c and step 5 still run behind it. State this in
  the charter so nobody double-fixes: step 6 then handles only 4a's and 4c's findings.
- **Every implementer and fixer answers the self-check**, in writing:

  > Does anything I added admit the very state I was fixing, or its MIRROR, or assert something I did
  > not measure?

  Both directions. Eight consecutive fix rounds each introduced a new instance of the class they were
  fixing; the two that broke the streak both answered "yes, once" and caught it themselves, by
  measurement rather than by reading. When a fix REPLACES a check rather than adding to one, enumerate
  what the old check caught that the new one does not and diff the accept sets. If they are not
  identical plus the intended closure, you moved the hole rather than closing it.

## Scope is what makes the termination argument true

The proof that this strategy is finite assumes the finding set is BOUNDED BY THE DIFF. An unbounded
review breaks it, and under F the damage is immediate: an out-of-scope finding cannot become a task, so
it directly expands the PR that was supposed to close.

Every charter (2, 4a, 4a-s, 4b, 6v) therefore states the diff under review
(`git diff origin/main...HEAD`) and requires findings to anchor to a line it ADDED or CHANGED. A defect
in code the slice merely calls is out of scope unless the slice made it reachable or made it worse. A
pre-existing defect the slice WIDENS is in scope only up to the widening, and the remedy is the smallest
change that restores the pre-slice blast radius, never a redesign of the surrounding system.

An out-of-scope defect a reviewer happens to see is NAMED in a separate "observed, out of scope"
section, carrying no disposition. It never enters the register.

## Who runs each

Same roster as [planned](../planned/SKILL.md), with 6v going to Fable by inheritance and steps 7 and 8
having no owner because they do not exist. The verifier's printed WHO column is not enforced and is
stale; `pipeline-model-allocation.md` in this project's memory directory is the live source.

## What this strategy does NOT do

- **It does not create tasks.** Not from step 4a, not from 4b, not from 6v, not from adjudication. The
  register REFUSES a TASK disposition under F, so this is a property of the tooling rather than
  something to remember.
- It does not defer anything. Fixed in-round, or accepted with a written rationale.
- It has no step 7, so there is no second review of the fix and no fix-review loop.
- It does not ratchet the step-0 corpus, which was step 8's other half. That half is moot in practice
  because step 0 has never run, and it must come back for plan-derived work if the corpus is ever built.

## The siblings

- [planned](../planned/SKILL.md): plan-derived work, all ten steps, and the only strategy that may file
  follow-up tasks. If the work came from the plan rather than from a review, you are in the wrong skill.
- [test-first](../test-first/SKILL.md): no prose brief and no brief review, for behaviour that can be
  stated as a failing test before it can be stated as a paragraph.
