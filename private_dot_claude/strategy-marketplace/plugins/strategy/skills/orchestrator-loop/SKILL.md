---
name: orchestrator-loop
description: Run a slice where the orchestrator is inside the loop, writing the failing tests and the seams itself instead of briefing an implementer, with two concurrent reviews and no deferred findings. Use when the behaviour can be stated as a test, when a task says Strategy-B, or when the user says "/strategy:orchestrator-loop".
---

# Orchestrator-loop

**The ORCHESTRATOR is inside the loop.** In the other two it writes a brief and hands it to an
implementer; here it writes the FAILING TESTS and the trait seams ITSELF, and those tests are the
specification. There is no prose brief and no brief review, because a concrete failing test says what a
paragraph of instructions only gestures at. The tests are the spec, the red state and the design in one
artifact. The second difference is that its two reviews run CONCURRENTLY rather than one after the
other.

**It is also closed-loop in the findings sense**: every finding is fixed in this iteration and none
becomes a task. The three names do not sit on one axis. Two of them
([open-loop](../open-loop/SKILL.md) and [closed-loop](../closed-loop/SKILL.md)) name what happens to a
FINDING; this one names WHO is inside the loop, and on the findings axis it behaves like
[closed-loop](../closed-loop/SKILL.md).

## When to use this

The behaviour can be stated as a failing test before it can be stated as a paragraph. If you find
yourself writing a paragraph of instructions for an implementer instead, you want
[open-loop](../open-loop/SKILL.md).

**This was called Strategy-B.** Unlike the other two it has no letter of its own: the verifier scripts
take only `A` and `F`, and it runs the `F` ledgers, as below.

This strategy exists because [open-loop](../open-loop/SKILL.md) was rejected as too slow and too heavy: a
slice ran two to three hours and roughly a million tokens across four agent dispatches. This one runs
about an hour.

Read [the ledger grammar](../../ledger-grammar.md) before ticking anything.

## It runs the ledgers as F

The two verifier scripts know exactly two strategies, `A` and `F`. There is no third letter, so this
one opens `F` ledgers and deviates the step it does not have:

```bash
~/.claude/pipeline/slice-checklist.sh  new <slug> F [--security] [dir]
~/.claude/pipeline/findings-register.sh new <slug> F [dir]
```

- **Step 1** is not deviated. Its artifact here is the RED TEST COMMIT rather than a prose brief, so its
  evidence is that commit sha.
- **Step 2** is deviated. Mark its box `[DEV]`, put "no prose brief: the failing tests are the brief,
  commit \<sha\>" in its EVIDENCE field, and repeat the reason under **Deviations**. A `[DEV]` box with
  an unfilled evidence field fails exactly like an unticked one.
- In the register's declared-verdicts table, set step 2's verdict to `n/a`. An `n/a` step must then
  carry no finding rows, which is correct: a review that did not run found nothing.
- Everything else the `F` ledgers require still applies, **including step 6v**. Dropping step 7 without
  6v would leave the fix with nothing checking it at all.

## Per slice, in order

1. **The orchestrator writes the failing tests and the trait seams**, inline. Red first, always.
1. **An implementer makes them green**, plus a differential check against the original implementation
   wherever one exists.
1. **sol reviews the diff at ULTRA on a TIGHT charter**, in parallel with a mutation-testing agent.
   Same slice, SEPARATE worktrees: the mutating agent edits production code and reverts, so sharing one
   worktree produces false SURVIVED results, and a false SURVIVED reads as a coverage gap that sends the
   next round to plug a hole that does not exist.
1. **The findings are fixed BEFORE the next slice starts.** Nothing carries over.
1. **The orchestrator adjudicates, pushes, waits for CI, and merges.**

Budget, and re-forecast it against real numbers rather than letting the estimate drift: tests 10
minutes, implement 15, sol and mutation in parallel 15, fixes 10, push and CI and merge 12.

Independent slices MAY run in parallel; reliability is the bound rather than a fixed count, and each
one carries its own checklist verified step by step. Parallel sol attempts are allowed, but if one
fails while another runs, wait for the running one, restart the failed one after it, and run every
subsequent sol review sequentially.

## The testing charter

This is the part that makes the tests worth writing first. Hard-to-test code is read as a SOLID
violation signal, not as a testing problem.

1. **Test through the PUBLIC interface, always.** Never make something public, and never reach into a
   private piece, because it is easier to test. Internals are covered through the public surface.
1. **Push the public interface itself toward VALUES** where that is honest: values in, plan out. Then a
   zero-double test is legitimately a front-door test.
1. **Where the public behaviour IS an effect, SPY the effect at the boundary.** Record what crossed the
   seam and assert on that. Inputs are fed by thin one-method stubs.
1. **A fake with behaviour in it is a design smell.** The double's complexity measures how badly the
   boundary is placed: zero doubles, then a spy, then a thin stub, then a behavioural fake, worst last.
   Heavy rigs stay confined to adapter tests.

The tripwire for the implementer, and put it in the charter: needing a fake with LOGIC in it means STOP
and reconsider the boundary. Could a value be handed in instead? Could the trait be narrower? Record the
decision either way.

Mid-slice seam refactors are default-authorized when small and in scope, with the refactor test-covered
before the feature lands on top. Say so in the slice log when you take one.

The outer suites that drive the real binary stay load-bearing. They alone prove the wiring between the
pure core and the edges, and a slice with a fully tested pure core and an untested composition root has
been measured to survive having that composition function's whole body replaced by a constant.

## Charter breadth is the dominant term

**sol at ultra, tight charter, 133-line file: 189 seconds, measured.** The review was real, not shallow:
it found an option-injection case and a `"."` filename case in code that had already passed a full
review, and ran its own mutants to confirm every test could fail.

The orchestrator had been estimating 25 to 40 minutes per sol review, from three samples that were all
LARGE diffs with seven-part charters, and treating that as a constant. It is not. sol's time scales with
charter breadth and diff size, and a narrow charter on a small diff is minutes. That one wrong
assumption inflated every estimate by roughly ten times and was the entire reason the full pipeline
looked infeasible.

Standing lesson, and it generalises past this strategy: measure the dominant term before designing
around it.

One caveat on ultra: it spawns roughly four delegated agents, so for a small single-artifact diff the
delegation may be pure overhead, and one continuous pass at max is better at CONNECTING facts across
lenses. Untested. Worth an A/B if slices run long.

## What this strategy does NOT do

- **No prose brief and no brief review.** If you find yourself writing a paragraph of instructions for
  an implementer, you are running [open-loop](../open-loop/SKILL.md) instead, and should say so.
- **It creates no tasks.** It runs the `F` ledgers, and the register mechanically refuses a TASK
  disposition. Findings are fixed in-round or accepted with a written rationale.
- It has no step 7, so there is no second review of the fix and no fix-review loop.
- It does not carry findings into the next slice. They land first.

## The siblings

- [open-loop](../open-loop/SKILL.md): all ten steps, a written brief that gets reviewed before any code
  exists, and the only strategy whose findings may leave the loop as tasks.
- [closed-loop](../closed-loop/SKILL.md): for work that exists BECAUSE a review found something. Same
  findings discipline and the same `F` ledgers as this one, but the orchestrator is back outside the
  loop: a real brief at step 1 and a real brief review at step 2.
