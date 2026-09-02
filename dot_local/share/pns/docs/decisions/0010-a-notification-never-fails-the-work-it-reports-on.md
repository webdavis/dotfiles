# 0010: A notification never fails the work it reports on, so the delivery path is fail-open and state mutation is fail-closed

Status: accepted, and it is the rule that governs every other fail-direction choice in the crate. This
record exists so it is settled BEFORE the first transactional-store code lands, rather than being
rediscovered per repository.

## The rule

pns runs inside somebody else's turn. A harness hook, the shell notifier, the daemon and an external
alert path all call it while real work is in flight. If pns fails loudly, it damages the work it was
merely reporting on.

So:

- **The delivery path is fail-open.** A busy, locked, missing or corrupt store does not stop a
  notification. Deliver, and record the miss wherever recording is still possible.
- **State mutation is fail-closed.** A write that cannot be made correctly is not made at all. Nothing
  half-applies, and nothing invents a value to get past an unreadable one.
- **An unknown reading is never coerced into a confident one.** Each reading states its own fail
  direction. See `docs/decisions/0003-numeric-readings-mirror-the-shell-they-replaced.md`.

## How it shows up today

- Ordinary hooks exit 0 on every path, whatever went wrong. The one exception is the forwarded gate,
  where the exit code is moshi's own and is passed through untouched.
- The dispatch loop has no `?`. Every destination is constructed before the first delivery, each leg runs
  under `catch_unwind`, and there is no `std::process::exit` on the event path.
- State writes are fail-quiet by design, in the words of `src/main.rs:record_missed`: "An event path
  whose stdout a harness hook reads must not gain a line about the state directory, and a journal entry
  that did not land costs a replay, never a card."
- The pulse fails CLOSED in the other direction, and the reason is stated at
  `src/pulse.rs:session_was_long`: "unlike a dropped phone push, a missed pulse costs nothing, so this
  one fails CLOSED rather than flashing the room on garbage."

That last one is not an exception to the rule, it is the rule applied. The question is always what the
failure costs the operator: a lost card costs them the thing pns exists to give them, and a lost pulse
costs them nothing.

## Where the two directions meet

An event that is delivered but not journalled is fine: the operator saw it. An event that is journalled
but not delivered is fine: the replay finds it. An event that is neither is the only real loss, and it is
the case both directions are chosen to avoid.

Ordering follows from this, and `src/main.rs:claim_fire` already states the pattern for the nag: markers
are written BEFORE the card and claims are removed AFTER it, so a crash before the card leaves approvals
marked and silent, a crash after it leaves claims nothing re-enumerates, and neither ordering can produce
a SECOND card.

## Consequence for the refactor

1. A repository trait's error type must let a caller tell "could not, and it does not matter here" from
   "could not, and this must not proceed". A bare `Option` cannot carry that, so typed outcomes are used
   for persistence claims, probe results, delivery results, protocol acceptance and diagnostic checks.
1. The transactional store gets bounded busy timeouts, and the delivery path treats a timeout as
   fail-open rather than as an error to propagate.
1. Crash-recovery and multiprocess-contention tests are written for the store, because this rule is a
   claim about behavior under failure and is worth nothing untested.
1. No use case panics on ordinary external failure. A panic is acceptable only for a compiled-in
   invariant whose violation is a programmer error, and never for anything reachable from operator input
   or runtime conditions.
