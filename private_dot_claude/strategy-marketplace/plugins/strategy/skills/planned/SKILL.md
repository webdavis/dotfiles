---
name: planned
description: Run a slice that came from a plan or spec through the full pipeline, with both merge gates. Use when starting plan-derived work, when a task says Strategy-A, or when the user says "/strategy:planned".
---

# Planned

Work written up from the plan or the spec. Every step runs, and this is the only strategy whose reviews
may create follow-up tasks. Those tasks then run [follow-up](../follow-up/SKILL.md), which is what keeps
the tree exactly two levels deep and provably finite.

Decide the strategy when the TASK IS CREATED and write it in the task description, so it is never a
judgement call later.

Read [the ledger grammar](../../ledger-grammar.md) before ticking anything: it carries the two verifier
scripts, what counts as evidence, the verdict rule, the register's dispositions, and how to record a
deviation. This strategy is `A` in both scripts.

## Open both ledgers at step 1, before any code exists

```bash
~/.claude/pipeline/slice-checklist.sh  new <slug> A [--security] [dir]
~/.claude/pipeline/findings-register.sh new <slug> A [dir]
```

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
| 7    | Re-review the fix, EXACTLY ONCE. Never a loop                           | quoted verdict |
| 8    | Open findings become follow-up TASKS; ratchet the step-0 corpus         | task numbers   |
| 9    | Gates, push, verify the ref landed, open PR, merge                      | PR number      |

Step 1 must not happen in the same action as step 3. The log line is what creates the gap for step 2 to
happen in; without it step 2 is not skipped by choice, it is unreachable.

Step 2 catches a stale or false brief for free, before anyone builds against it. A brief is a
HYPOTHESIS, not a fact: verify each of its claims against the tree rather than obeying it, and say so
when it is wrong. Every file and line number it cites is re-measured with
`git show origin/main:<path> | grep -n`, never against a working copy or a sibling worktree, because
several stale addresses in a row all traced back to measuring in the wrong tree.

Step 3 ships a DIFFERENTIAL test when the slice models an external tool's behaviour: a corpus run
through the real binary, asserting our reading matches its reading. Not a list of cases someone thought
of. Where you can ask the real tool instead of modelling it, do that and skip the problem entirely.

Steps 4a and 4b run in PARALLEL and 4b MUTATES production code, so give each its own worktree. Sharing
one produces false SURVIVED results, which is the dangerous direction: it reads as a coverage gap, so
the next round is briefed to plug a hole that does not exist.

Step 4b needs a CONSTRUCTED lens, not just a fresh instance. Name the mutation classes to attempt:
revert the fix itself, weaken each guard's precision rather than deleting it, delete a message or a
status while leaving behaviour intact, replace a helper with the naive thing a future editor would
write, break the exemptions and confirm the CLEAN fixtures fail. Require a table of mutation, outcome,
and WHICH assertion killed it. A kill with no named assertion is not a kill, and every sweep runs an
UNMUTATED CONTROL, because a broken harness reports every mutation identically.

Every review charter (2, 4a, 4a-s, 4b, 7) ends with a CODE QUALITY section, reported separately and
ranked BELOW correctness, so a SOLID nit never outranks a missed defect. Adjudicate a quality finding
the same way as a correctness one: fix it, or accept it with a written rationale.

## Who runs each

The verifier prints a WHO column and does NOT enforce it, and that column is stale (it still says Opus
implements, sol at max, Fable at xhigh). The live roster is `pipeline-model-allocation.md` in this
project's memory directory; read it if the two disagree. As of the 2026-09-01 ruling:

- 1, 4c, 5, 8, 9: the orchestrator.
- 2, 4a-s, 4b: Fable, reached by INHERITANCE only. Never pass `model:"fable"`; that override is
  entitlement-gated and falls back to Opus silently, with no error.
- 3, 6: an implementer subagent (`model:"sonnet"`).
- 4a, 7: sol at ultra, and the redirect matters:
  `codex exec -m gpt-5.6-sol -c model_reasoning_effort="ultra" -c approval_policy=never -s read-only -C <repo> -o <out> "<prompt>" </dev/null`.
  Without `</dev/null` it waits on stdin forever.

The principle outlives any particular model. Whoever implements never reviews their own work. 4b must be
write-capable, so it can never be sol, which runs read-only and structurally cannot mutation-test.
Authentication and credential subject matter goes to Fable, because sol's runtime refuses it outright
regardless of wording, and rewording does not help.

Dispatch each step EXACTLY ONCE, and check for an agent already running it before dispatching. Two
agents on one step race the worktree. Once 4b is dispatched on a worktree the implementer is done: route
any late refinement to 4b, which fixes in place, and never message the implementer about that slice
again.

Retry an assigned reviewer ONCE, then substitute a write-capable agent and record the deviation. Never
merge with a review step simply missing. "The run died" needs positive evidence: an error marker, a
refusal string, or a process exit plus an empty file on a SECOND check after a settling delay. An empty
file on first check is the normal state of a run in progress.

## Reviews stay inside the slice's own diff

Every review charter states the diff under review (`git diff origin/main...HEAD`) and requires findings
to anchor to a line it ADDED or CHANGED. A defect in code the slice merely calls is out of scope unless
the slice made it reachable or made it worse. A pre-existing defect the slice WIDENS is in scope only up
to the widening, and the remedy is the smallest change that restores the pre-slice blast radius.

A reviewer that happens to see an out-of-scope defect NAMES it in a separate "observed, out of scope"
section carrying no disposition. Those never enter the register, and the reconciliation counts only
in-scope findings.

## Filing step 8's tasks

Every open finding becomes a task; nothing is demoted to a comment. Filing one also requires a
scheduling decision at the moment it is written: can it wait, or must it land now, and say which in the
task. Fan-out, not throughput, is what stalls a plan, so a finding on shipped and working code is
normally scheduled out.

A finding on this UNMERGED PR is not a follow-up at all. It is fixed in this round and never reaches
the queue.

## What this strategy does NOT do

- It does not force every finding into this PR. Step 8 exists, so a finding may legitimately leave as a
  task. Under the other two strategies it may not, and the register mechanically refuses.
- It does not skip step 7. The fix gets an independent re-review, exactly once, never a loop.
- It has no step 6v. Terminal verification belongs to follow-up, which has no step 7 to do that job.
- It does not let a review run unbounded over the repository.

## The siblings

- [test-first](../test-first/SKILL.md): no prose brief and no brief review, because failing tests written
  first say what a paragraph of instructions only gestures at. Pick it when the behaviour can be stated
  as a test before it can be stated as a paragraph.
- [follow-up](../follow-up/SKILL.md): for work that exists BECAUSE a review found something. Pick it when
  the task was created by step 4a, 4b or 7. It creates no tasks of its own.
