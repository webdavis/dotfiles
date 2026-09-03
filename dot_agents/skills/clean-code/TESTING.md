# Testing

What counts as proof, and what only looks like it. Reference for [`SKILL.md`](SKILL.md).

Use the narrowest reliable test level.

## Two kinds of work, two obligations

**New behavior is written test-first, without exception.** Write the failing test, run it, see it
fail *for the reason you intended*, then make it pass. That covers every new protocol and decoder,
every repository and migration, the configuration version and its migrations, the snapshot's
coherence and memoization guarantees, every new typed outcome, and any behavior a specification says
exists but no test currently pins.

**A pure move is not new behavior, and a "failing test" for one is theatre**: it passes before the
move and after it. What a move owes instead is proof it changed nothing. Before moving code, confirm
its behavior is already pinned by a test that can fail; if it is not, **write that test first,
against the code in its current location, and land it before the move**. The move is then verified by
the argv differential and by the test-name set diff, which is the only honest evidence a refactor can
offer.

State in each pull request which kind of work it is, and give the matching evidence.

## Mutation verification

**Every fix, in either kind, is mutation-verified.** After a test goes green, mutate the source it
covers and prove the test catches it, against an unmutated control at the same relative depth. Where
no mutation-testing tool is installed this is done by hand, per behavior, and the table goes in the
report.

A test that stays green with the logic gutted is worthless, and this repository has found that exact
defect repeatedly. Five failure modes to check for by name:

**A probe that never proves its own edit landed.** A mutation run that reports an exit code without
first asserting the mutated bytes are on disk is reporting the control twice. Assert the edit landed
before you trust the result.

**A mutant that hangs instead of failing.** A test that blocks on a thread, a socket, a pipe or a
child has a failure mode that is not red, it is *never finishing*, and a hang reads as "still
running". Give every such test a deadline and fail on it, and **mutation-verify by making the subject
hang, not only by making it answer wrongly.** A test guarding whether a message reached a destination
at all is the least acceptable place in a crate for a test that cannot fail.

**Nothing exercises the composition root.** A slice whose tests all exercise pure functions passes
with the composing function replaced by a constant, because no test ever calls it. Check that
explicitly: replace the function that wires the pieces together with a fixed value and confirm
something goes red.

**An unbacked "equivalent mutant" claim.** A mutant declared equivalent cannot be checked by running
the tests, since a genuinely equivalent one is green by definition and so is an uncovered one. The
claim needs an argument about the **intermediate states**, not just the final output: show the two
versions cannot diverge on any observable the code produces along the way.

**A control that only proves the feature ran.** A negative test whose control shows the code path
executed, rather than that the specific guard fired, catches nothing when the guard is deleted.

## Speed is a gate

Every test passes within one second, measured, or it goes. Enforce it in the suite's own support
module: a case over the budget warns, one over the ceiling fails unless an explicit escape names a
structural cause.

The CI runner is roughly four times slower than the development machine (measured 2026-09-02: 1.3 s
locally, 5.5 s on the runner, same test), so a test over about 1.2 s locally fails CI. Bound property
tests in case count. Contention and crash-recovery tests use small fixtures and poll for evidence.
The test runner executes units in parallel competing for the same CPU, so measure under that
configuration.

**Do not use arbitrary sleeps for synchronization.** Poll for evidence, use channels or barriers, or
use a controlled clock.

## Nothing reaches a real destination

No test, differential, or verification step may read the operator's real configuration, touch the
operator's real state directory, or contact a real external service, gateway, device, or desktop
notification system. Every run uses a sandbox `HOME` and scripted transports.

Know which of the tool's own commands have live effects, and exclude them from every harness. A
diagnostic command and a manual trigger are live-effect commands: a verification harness that ran
them posted two real notifications and drove the lamps on 2026-09-02. Reuse or extend the existing
argv differential rather than writing a new one.

## Tooling that exists, and tooling that does not

The language skill names which mutation, fuzzing and sanitizer tools are actually installed here, and
which of their results CI accepts. Add a property-testing or fuzzing framework only after naming the
input space it covers better than examples do.

## Test levels

**Unit tests** live beside their implementation, excluded from production builds by the language's own
mechanism. A large unit-test module may live in a private child file beside the one it covers; that is
still unit testing and still does not ship.

**Contract tests** are reusable behavioral suites for application ports with multiple
implementations. They must cover success, each failure class, idempotency, replacement, ordering, the
absence of unintended side effects, atomicity, concurrency semantics, and persistence across
instances where that is promised.

**Adapter integration tests** run real adapters against controlled infrastructure: temporary
databases, scripted transports, temporary files, exact argv runners, isolated process trees, and
controlled external fixtures.

**Acceptance tests** preserve black-box coverage of assembled workflows and process-boundary
contracts. Split large suites **by behavior**, never into `part1` and `part2`. One file per behavior
area: each protocol, each legacy entry point, each policy area, persistence, diagnostics, setup,
privacy, and process lifecycle.

**Protocol and golden tests** add fixtures for every external protocol version, and test exact tagged
shapes, unknown versions, missing fields, malformed fields, size limits, hostile text,
forward-compatible additive fields, duplicate request IDs, request and result correlation, and legacy
translations.

**Property and fuzz tests** where they add meaningful coverage: protocol decoders, state codecs, path
and identifier validation, duration parsing, Unicode sanitization, and any budgeting or scheduling
invariant.

## Testing strengths to preserve

Keep testing explicitly:

- fail-open versus fail-closed direction
- exact threshold values, and one step before and after each
- future-clock and backward-clock behavior
- malformed and hostile external data
- path traversal
- control and Unicode format characters
- no leakage of private content
- no use of the operator's real state directory
- process cleanup and bounded child execution
- no fixed sleeps
- a complete diagnostic census despite individual failures
- no side effects from observation events
- truthful operator wording

Use exact output assertions **only** where wording, stdout, stderr or exit status is an external
compatibility or operator-safety contract. Otherwise prefer typed outcome assertions.
