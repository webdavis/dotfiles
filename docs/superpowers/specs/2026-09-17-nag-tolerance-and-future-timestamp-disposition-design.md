# B19 and B25: disposition against current source

Date: 2026-09-17

## What this document is

`docs/remaining-work.md`'s B18 bullet says B19 and B25's nag tolerance and future-timestamp
proposals "still need explicit disposition against current source" and that their "conditional
proposals are not automatic implementation work." This document gives that disposition. It does
not implement either proposal.

## Where the proposals came from

Both items are rows in `~/.claude/pipeline/backlog-consolidated-2026-09-02.md`:

- B19 (row 48): "The nag verdict has no one-second tolerance, so a sibling arm reading the next
  second loses a nudge," against `dot_local/share/pns/src/nag.rs:278-280`, with the condition
  quoted as `is_stale` being `armed > now || now > armed + 2*after`.
- B25 (row 54): "`age_of` clamps an epoch NEWER than the decision clock to age 0 instead of
  unknown," against `dot_local/share/pns/src/engine.rs:395-396`, with the condition quoted as
  `now.saturating_sub(taken_at?)`.

`docs/superpowers/plans/2026-09-05-pns-refactor-plan.md:1005` carries both forward as one line
item: "B19, B25 (nag one-second tolerance; `age_of` clamping a future epoch to zero): PR 5.5 and PR
5.11 as new behavior with tests, if the operator rules; otherwise recorded." No PR 5.5 or PR 5.11
implementing either behavior exists on `main`; the refactor since renamed `dot_local/share/pns` to
the `pns/` Cargo workspace, and the two functions named above moved with it. This document is that
operator ruling, made explicit with evidence rather than left as "otherwise recorded."

## B19: the nag verdict's one-second tolerance

### Disposition: NOT WANTED

The premise is a race between multiple readers of `now` inside a single nag fire. Current source
makes that race structurally impossible, so there is no clock skew for a tolerance to absorb.

**One clock read per fire, shared by every record judged in it.**
`pns/crates/pns/src/command_nag.rs:47-51` reads the clock exactly once, before anything else, and
passes that single value down:

```rust
let Some(now) = now_secs() else {
    eprintln!("pns nag: this machine has no clock to measure a wait against");
    return 0;
};
match (pns_application::RunNag { records: &records, notifier: &NagNotification })
    .run(now, after_secs, |warning| eprintln!("{warning}"))
```

`pns/crates/pns-application/src/nag.rs:36-43` (`RunNag::run`) then loops over every claimed record
in that fire and judges each one against that same `now`:

```rust
pub fn run(&self, now: u64, after_secs: u64, mut warn: impl FnMut(&str)) -> Outcome {
    if !self.records.claim_fire(now) {
        return Outcome::Busy;
    }
    let mut waiting: Vec<(String, Record)> = Vec::new();
    for claimed in self.records.claim_due(now) {
        match nag::fate(claimed.record.as_ref(), claimed.answered, now, after_secs) {
```

There is no second call to the clock anywhere between the fire being claimed and the last record
being judged. Every "sibling" record in one fire is judged against the identical `now`, so no
record can lose a nudge to a clock that ticked over between siblings; there are no siblings with
different clocks to begin with.

**A second process woken in the same window never gets a `now` of its own to disagree with.**
`pns/crates/pns-adapters/src/protocols/nag/claims.rs:92-95` (`claim_fire`) is an exclusive file
create, documented at lines 76-84 as deliberately not a rename precisely because a rename-based
claim let two concurrent fires each win a window and each deliver a card:

> AN EXCLUSIVE CREATE IS THE ARBITRATION, NOT A RENAME, and the difference is measured rather than
> stylistic. … That form delivered TWO cards from four concurrent fires, reproducibly, under load.
> An exclusive create leaves the lock sitting at its name for the whole fire, so every later racer
> is refused by the same atomic operation, whenever it arrives.

`pns/crates/pns/src/command_nag.rs:13-17` documents the same guarantee at the call site: "two
processes woken by two jobs in one tick produce one card between them rather than one card each."
A losing process returns `Outcome::Busy` from `RunNag::run` at line 38 before it ever calls
`claim_due` or `fate`, so its `now` (even if read a second later) never reaches `is_stale`.

**Conclusion.** B19's failure mode requires two things current source both refuse: a second
`now` read inside one fire, and a second process surviving past the fire-claim to use its own
`now`. Neither happens. The one-second tolerance the backlog row asked for has nothing to widen
against; adding one would not fix a bug, because the bug the row describes cannot occur under the
exclusive-create fire lock. `pns/crates/pns-domain/src/nag.rs:177-178`'s `is_stale` (`armed > now
|| now > armed.saturating_add(after_secs.saturating_mul(2))`) is unchanged in substance from the
formula B19 quoted, and its doc comment (lines 156-176) already gives the deliberate reasoning for
both bounds without needing a tolerance term. No task is filed.

## B25: `age_of` clamping a future epoch to age zero

### Disposition: WANTED AND FILED

The successor of `engine.rs`'s `age_of` is `pns/crates/pns-domain/src/decision/reading.rs:48-49`:

```rust
let age_of =
    |taken_at: Option<u64>| now_secs.and_then(|now| Some(now.saturating_sub(taken_at?)));
```

This is the same clamp B25 described: `saturating_sub` on an unsigned integer returns `0` when the
subtrahend exceeds the minuend, so a `taken_at` newer than `now_secs` (a phone `atime`, a marker
`mtime`, from a clock that is skewed, adjusted backward mid-read, or hand-edited) reads as age `0`,
the freshest possible value, rather than as `None` (unknown). The doc comment immediately above it
(lines 45-47) addresses the adjacent case, an unreadable clock, but not this one:

> AGES, never timestamps, and both aged against the SAME clock read: an unreadable clock ages
> nothing, which drops a phone signal out of the arbitration rather than making it infinitely
> fresh.

That sentence states the intended failure direction for one input (`now_secs` is `None`) and gets
it right: unreadable becomes absent. It says nothing about the other input (`taken_at` is in the
future), and the code does not apply the same direction there. There is no other clamp, filter, or
future-timestamp check anywhere in `pns/crates/pns-domain/src/decision/` (confirmed by grep for
`future` and `clamp` in that directory: the only hit is the unrelated comment at line 35 about
`PNS_IDLE_SECS`).

**Why age `0` is the wrong answer, in this function's own terms.** `age_of`'s two callers feed
presence arbitration: `phone_input_age` (line 51) and `marker_age` (line 57) both flow into
`crate::surface::surface(...)` (line 59) and `crate::surface::is_fresh(phone_input_age, ...)`
(line 66), which decide whether the operator reads as present at their desk or on their phone. Age
`0` is indistinguishable from a signal taken at the exact instant of the decision, the strongest
possible evidence of presence. A future `taken_at` is not strong evidence of anything; it is a
clock that cannot be trusted, and the function's own stated policy for that state (line 46, "an
unreadable clock ages nothing") is `None`, not `0`. A `phone_atime` or `marker_mtime` in the future
is reachable without any adversarial input: a filesystem mtime survives a network time
synchronization step backward on the machine, and dresden runs both the phone-attention marker
writer
(`S170`, referenced in `docs/superpowers/plans/2026-09-05-pns-refactor-plan.md`) and `atime`-based
phone reads outside this crate's control.

**What a later task would implement, precise enough not to re-derive this.** In `age_of` at
`pns/crates/pns-domain/src/decision/reading.rs:48-49`, when both `now` and `taken_at` are `Some`
and `taken_at > now`, return `None` instead of `Some(0)`:

```rust
let age_of = |taken_at: Option<u64>| {
    now_secs.and_then(|now| {
        let taken_at = taken_at?;
        (taken_at <= now).then(|| now - taken_at)
    })
};
```

This changes only the future-epoch branch; the unreadable-clock and unreadable-timestamp branches
already return `None` and are untouched. It needs new test coverage for the future-epoch case in
whatever test module covers `surface_reading` today (`pns/crates/pns-domain/src/decision/` has no
`tests.rs` alongside `reading.rs` currently; one would be added), asserting that a `taken_at` one
second ahead of `now_secs` produces `phone_input_age: None` and `marker_age: None`, not
`Some(0)`, and that the resulting `Surface` and `phone_input_fresh` match what a genuinely
unreadable timestamp produces. `now.saturating_sub(taken_at)` is not the only similar pattern in
the crate (`pns/crates/pns-domain/src/lights/held.rs:24`, `pns/crates/pns-domain/src/stale.rs:135`,
and others also subtract a stored timestamp from `now`); this task is scoped to `age_of` only, the
function B25 named, and does not sweep the others without separate evidence that each one is
reachable with a future timestamp and that a future timestamp changes its verdict the way it does
here.

**Cost.** A four-line function change, a small closure rewrite that keeps the same call signature,
plus a new unit test module. No config surface, no migration, no adapter change; `age_of` is
private to `reading.rs` and both call sites (lines 51 and 57) pass their result straight into pure
domain functions that already handle `None`.

## Summary

| Item | Disposition | Why |
| --- | --- | --- |
| B19 | NOT WANTED | The exclusive-create fire lock (`claims.rs:92-95`) and the single `now` read shared by every record in a fire (`command_nag.rs:47-51`, `nag.rs:36-43`) make the described race, two arms judging one record against two different clock reads, structurally impossible. Nothing to add tolerance against. |
| B25 | WANTED AND FILED | `age_of` (`decision/reading.rs:48-49`) still clamps a future `taken_at` to age `0` (maximally fresh) instead of `None` (unknown), contradicting the function's own stated policy for an unreadable clock. Real gap, ~4-line fix plus a test, given above precisely enough to implement without re-deriving it. |
