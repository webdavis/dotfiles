# Condenser deadline load drill, 2026-09-14

The ledger item "Resolve the historical condenser-stall task" (`docs/remaining-work.md`, under "pns
validation follow-ups") asks for one thing: run the load reproduction that the 2026-09 audit skipped,
against the hook test that still injects a 300 ms condenser deadline, and then record closure or fix
whatever is still broken. Todoist task
[6hPCHVmfhXPM9FPM](https://app.todoist.com/app/task/6hPCHVmfhXPM9FPM) is the thing being resolved. Its
text, read with `td task 6hPCHVmfhXPM9FPM` on 2026-09-14:

> flaky test: condenser deadline stall under load
>
> Flagged by the slice-4 fix round 2026-08-28, PRE-EXISTING: tests/hooks.rs
> a_condenser_that_closes_stdout_and_sleeps_is_killed_at_its_deadline stalls (30s vs 2.6s signature)
> under parallel load; reproduces on a pristine archive of fb81ec7e. The repo rule is tests must be fast
> or they go: fix the timing assumption or delete it.

No code was written or changed for this record. The only writes were to `pns/target` (a `--no-run` build)
and to the scratchpad.

## Verdict

**Close 6hPCHVmfhXPM9FPM with this evidence. Adopt no fix, and do not delete the test.** Three reasons,
in the order they matter.

1. **The test has no timing assumption to fix.** Its assertions are satisfied by either arm of the bound
   it exercises, so an early expiry under load produces the same observable as a late one. The reasoning
   is in "The assertion is direction-independent" below, off the current source.
1. **390 executions of the two condenser deadline tests produced zero failures**, across four arms
   including sixteen added busy loops on an eight-core machine and a twenty-way concurrency arm at load
   average 55. The worst sandbox lifetime measured was 1885 ms against a 5000 ms hard ceiling.
1. **The mechanism that could plausibly have cost 30 s is gone, and a recurrence can no longer cost 30 s
   quietly.** A forked grandchild surviving a single-process kill is measured below; the process-group
   guardian that now reaps it landed on 2026-09-08, eleven days after the report. And the sandbox speed
   guard that landed on 2026-09-01 hard-fails any sandbox living past 5 s locally, so a future stall
   shows up as a named failure in five seconds rather than as thirty seconds of silence.

**The drill did find two load-sensitive tests, and neither is the condenser.** Under the added-load arm,
`approval_payload::a_payload_at_the_cap_is_whole_and_is_still_submitted` failed in seven of ten runs of
the full hooks binary, and
`delivery_class::json_class_policy_crosses_the_real_mute_and_focus_edge_without_changing_hermes` failed
in one. Both are fixture deadlines running out, not production deadlines. They belong to the sibling
ledger item that already names ordinary hook fixtures inheriting the five-second payload deadline, and
where they get filed is an open question below, not something this record decided.

## What was checked, and how

**Machine and toolchain.** macOS 26.2 build 25C56, eight cores, 16 GiB, `cargo 1.98.1`, `rustc 1.98.1`.
Repository at `f24aba51` on `main`, working tree clean apart from `docs/remaining-work.md`. Every load
average quoted below is a genuine reading from `uptime` on a machine that already had other agent
sessions running, which is why the ambient numbers start near 20 rather than near zero.

**Source read.** `pns/crates/pns-adapters/src/codex.rs` (the condenser and its deadline),
`pns/crates/pns-adapters/src/process/bounded.rs` (`run_bounded`, `finish_bounded`, `collect`),
`pns/crates/pns-adapters/src/process/wait.rs` (the polled post-stdout wait),
`pns/crates/pns-adapters/src/process/group.rs` and `process/group/watch.rs` (the process-group owner and
its guardian), `pns/crates/pns/src/hook_payload.rs` (the payload deadline),
`pns/crates/pns/tests/hooks.rs` and `tests/hooks/deadlines.rs` (the two tests, `HANG_LIMIT`,
`spawn_hook`, `write_payload`, `finished_within`), and `pns/crates/pns/tests/support/budget.rs` (the
sandbox speed guard).

**History read.** `git show fb81ec7e:dot_local/share/pns/tests/hooks.rs` and
`:dot_local/share/pns/src/system.rs` for the state the report reproduced on, plus `git log -S` for the
date each protection landed.

**Test binary.** Built with

```bash
cargo test --locked --manifest-path pns/Cargo.toml -p pns --features dev-tools --test hooks --no-run
```

which produced `pns/target/debug/deps/hooks-02dc641a492aa4aa` (190 tests). `--report-time` is refused by
this stable compiler, so every duration below was taken outside the binary with `gdate +%s%3N`, and
per-sandbox durations come from the suite's own "test budget" stderr lines.

**The four arms.** Drill scripts are in the scratchpad (`condenser-drill.sh`,
`condenser-parallel-drill.sh`, `condenser-concurrency-drill.sh`), all three `shellcheck` clean.

"Runs" counts executions of the two condenser deadline tests, 390 in total.

| Arm | Shape                                                                         | Runs |
| --- | ----------------------------------------------------------------------------- | ---- |
| 1   | one test per process, one thread, 40 iterations each, ambient load            | 80   |
| 2   | same, with sixteen added busy loops                                           | 80   |
| 3   | the full 190-test binary at default parallelism, ten runs, sixteen busy loops | 20   |
| 3b  | the full binary, five runs, ambient load                                      | 10   |
| 4   | twenty concurrent invocations of both tests, five rounds                      | 200  |

## Findings

### The assertion is direction-independent

`a_condenser_that_closes_stdout_and_sleeps_is_killed_at_its_deadline` stubs `codex` as
`cat >/dev/null; exec 1>&-; sleep 30`, injects `PNS_CONDENSER_DEADLINE_MS=300`, and asserts two things:
the hook exits 0 inside `HANG_LIMIT` (5 s), and the delivered `detail` is `a turn`, the reply itself.

`collect` in `bounded.rs` has exactly two ways to reach a no-answer, and both are on the same deadline.
Either the read arrives and the polled wait then runs out because the child closed stdout and slept, or
the read itself does not arrive inside the window and `recv_timeout` gives up. Both return `None`,
`condense` maps `None` through `fallback()`, and `fallback()` is `("done", preview(reply))`. So a machine
slow enough to blow the 300 ms window before the stub is even scheduled produces the same `detail` the
test asserts. The 300 ms number does not decide the observable; it only decides which arm expires.

The sibling test, `a_condenser_that_never_reads_its_stdin_is_bounded_too`, asserts only the exit code, so
it is direction-independent for the same reason and more simply.

This is also why no condenser test anywhere asserts that a verdict was USED under a tight deadline.
`stub_codex` in `tests/hooks.rs` deliberately injects no deadline at all, so the three tests that do
require the condenser's answer (`turn_reply::a_condenser_line_is_used_...` and the two in
`loop_waits.rs`) run against the production 30 s default. Those are the tests an early expiry would
break, and they have three orders of magnitude of headroom.

### The drill did not reproduce a condenser failure

Durations are whole-process wall clock for one invocation, in milliseconds.

| Arm | Load average during | Test                     | Failures | min | p50 | p95 | max |
| --- | ------------------- | ------------------------ | -------- | --- | --- | --- | --- |
| 1   | 33.07 to 23.85      | closes stdout and sleeps | 0 of 40  | 471 | 528 | 702 | 710 |
| 1   | 33.07 to 23.85      | never reads its stdin    | 0 of 40  | 476 | 593 | 794 | 899 |
| 2   | 23.51 to 37.04      | closes stdout and sleeps | 0 of 40  | 464 | 566 | 761 | 782 |
| 2   | 23.51 to 37.04      | never reads its stdin    | 0 of 40  | 535 | 580 | 681 | 762 |

Arm 4 ran twenty concurrent copies of both tests, five rounds, at load average 46.75 rising to 55.02: 100
invocations, zero failures, slowest single invocation 1906 ms, zero orphaned `sleep 30` processes left
behind.

Arm 3 ran the whole binary ten times under sixteen busy loops. The condenser tests passed every time.
Their sandboxes crossed the suite's 1000 ms review line five times across those ten runs, worst reading
1736 ms; in arm 4 the worst was 1885 ms. The hard ceiling in force locally is 5000 ms, so the worst
measured reading sits at 38 percent of the line that would fail the run.

Arm 3b ran the whole binary five times at ambient load with no added busy loops: five passes, and not one
condenser sandbox even crossed the 1000 ms review line.

### The 30 s mechanism, measured at the shell

`/bin/sh` on this machine forks the trailing command of a script rather than exec'ing it. Measured
directly: a script whose body is `sleep 30`, killed at its shell, left the `sleep` alive as pid 28310
with the shell gone. The suite already carries its own measurement of the same fact, in the
`stub_silent_moshi` comment in `deadlines.rs`: "0.001s to EOF after the kill with `exec`, 9.9s without."

At `fb81ec7e` that mattered, because `run_bounded` there set no process group and its give-up path was
`child.kill()` followed by `child.wait()`, which reaches the direct child only. A forked `sleep 30`
grandchild therefore survived, and for the stub that does not close stdout it also held the stdout pipe's
write end for its full thirty seconds.

On current `main` it does not survive. `finish_bounded` takes a `Group` first, whose forked guardian
`setpgid`s into the owned group, closes every inherited descriptor, hands back a readiness byte, and then
either polls the owner pipe to the deadline or is released early when that pipe closes because the
producer died. On the way out it calls `kill(-getpgrp(), SIGKILL)`, which reaches grandchildren.
`Group`'s own `Drop` repeats the group kill. Measured with `pgrep`: after 390 condenser executions, zero
leftover `sleep` processes.

### Four protections, and which of them post-date the report

| Landed     | Commit     | What it added                                                            |
| ---------- | ---------- | ------------------------------------------------------------------------ |
| 2026-08-12 | `01434fa6` | the polled post-stdout wait, and the stdin write moved inside the window |
| 2026-08-29 | `2a568d26` | `PROBE_READ_MAX`, a byte ceiling on the read                             |
| 2026-09-01 | `bc361d86` | the sandbox speed budget and hard ceiling                                |
| 2026-09-07 | `f0e62082` | `recv_timeout` measured against `expires_at` rather than restarted       |
| 2026-09-08 | `60ea30cb` | `process/group.rs`: the forked guardian and the group kill               |

**The first one is not the fix**, and the ledger's phrase "production now bounds post-stdout waiting"
should be read with that in mind: `01434fa6` predates the 2026-08-28 report by sixteen days, so the flake
reproduced on a build that already had it. Everything else in that table post-dates the report. The one
that changes the stall's cost rather than its cause is `bc361d86`:
`pns/crates/pns/tests/support/budget.rs` sets a 1000 ms advisory review line and a 5000 ms hard ceiling
locally (4x that in CI), and there was no such guard at `fb81ec7e`. A sandbox that lived 30 s would now
fail the run by name at five seconds.

### What the drill did find

Two tests failed, only in arm 3, only inside the full binary, and only with the busy loops running.

`approval_payload::a_payload_at_the_cap_is_whole_and_is_still_submitted` failed seven times in ten runs,
at `approval_payload.rs:50`:

```
assertion `left == right` failed: a payload that arrived whole is the operator's to answer
  left: None
 right: Some(42)
```

`None` is `finished_within` giving up: the hook did not exit inside `HANG_LIMIT`. That test writes a 1 MB
payload and then allows the whole hook, presence probes and moshi round trip included, the same 5 s that
`pns/crates/pns/src/hook_payload.rs:40` allows the payload READ alone by default. The fixture ceiling and
the production deadline it observes are the same number, so under contention the fixture loses first.

`delivery_class::json_class_policy_crosses_the_real_mute_and_focus_edge_without_changing_hermes` failed
once, at `delivery_class.rs:45`, with `Custom { kind: TimedOut, error: "pipe stayed open" }`. Its helper
allows 650 ms for an entire `pns submit --json` process to spawn, answer and close both streams.

Run one per process, both pass 20 of 20 at load average 60 (worst 1827 ms and 2642 ms). So the cause is
the test binary contending with itself across eight libtest threads on cores that had no idle left, not
the assertions and not the machine's load average on its own.

## Assumptions this record made in the operator's place

- **"Representative load" was read two ways and both were measured.** The narrow reading is the machine
  as agents actually leave it (load average 20 to 60 with idle available), which is arms 1 and 3b. The
  wide reading adds sixteen busy loops to remove the idle, which is arms 2, 3 and 4. The alternative is
  for the operator to name one definition; on the narrow one the two other flakes never appear either, so
  the choice only changes whether this record reports them.
- **`fb81ec7e` was not rebuilt.** The original stall was not reproduced on its own archive; its mechanism
  is established from the source diff plus the shell-level orphan measurement. The alternative is a cold
  build of the pre-workspace layout to reproduce the 30 s reading first. Judged not worth it: the reading
  would say something about a tree that no longer exists, and the question on the table is whether `main`
  stalls.
- **Neither "fix the timing assumption" nor "delete it" was taken.** The Todoist task offers those two.
  This record recommends a third, closing on evidence, because the assertion is not a timing assumption
  and the test buys a real bound. The alternative is deleting both tests under the fast-tests rule, which
  would leave the post-stdout wait and the group kill pinned nowhere end to end.
- **The two other flaking tests were left alone.** This item's scope is the condenser. The alternative is
  fixing them here, which would put an unrelated diff in a research task and pre-empt the sibling ledger
  item that already owns that class.
- **Nothing was written to Todoist.** Closing the task on a verdict the operator has not read, citing a
  scratchpad path the document has not yet been landed at, would be deciding for them. The alternative is
  one `td` call, which is the first operator step below.

## What would change the verdict

- **A condenser failure in the arms above, once.** Zero in 390 is the whole basis; one real failure moves
  this from "close it" to "diagnose it".
- **A condenser sandbox past 5000 ms locally, or 20000 ms in CI.** That is the hard ceiling, and crossing
  it is the gate this record is relying on to make any recurrence loud.
- **CI behaving differently from this machine.** Every reading here is local. The GitHub runner has fewer
  and slower cores, which is why `budget.rs` gives CI a 4x ceiling off a single 2026-09-02 measurement. A
  red condenser row on a runner would be new evidence, not a repeat of this one.
- **`stub_codex` gaining an injected deadline.** The three tests that require a condenser verdict are
  safe today because they run on the 30 s production default. Injecting a tight deadline there would
  create the load-fragile condenser test that does not currently exist.
- **The guardian being weakened.** If `Group` stops owning the group, or the group kill stops reaching
  grandchildren, the `fb81ec7e` mechanism is back and the orphan count stops being zero.

## Open questions for the operator

1. **Close 6hPCHVmfhXPM9FPM on this evidence, or hold it for a CI reading?** Recommended: close it. The
   alternative is holding it until the drill has run on a GitHub runner, which costs a throwaway workflow
   run and answers a question no observed failure has raised.
1. **Where do the two other flakes get filed?** Recommended: fold them into the existing ledger item
   "Split and reconcile 6hPJVf2FJc3RHxqM", which already names ordinary hook fixtures inheriting the
   five-second payload deadline and already carries B105's approval-submission exit-code failure under
   load. The alternative is a new Todoist task and a new ledger bullet, which splits one class across two
   items.
1. **Is a fixture ceiling equal to the production deadline it observes acceptable?** `HANG_LIMIT` is 5 s
   and the default payload deadline is 5 s. Answering this decides whether the fix for the
   approval-payload flake is a longer fixture ceiling, a shorter injected production deadline in that
   test, or a smaller payload.
1. **Is the 1000 ms advisory review line worth what it now costs?** The two condenser sandboxes cross it
   under load while behaving correctly, and `allow_slow` cannot quiet them: reading `Drop for Sandbox` in
   `tests/support/sandbox.rs`, the excuse lifts the 5000 ms hard ceiling only, while the warning fires
   off `over_budget` unconditionally. So the options are accepting a recurring "test budget" line from
   tests that are correct, raising the line, or giving the excuse a second effect. Recommended: accept
   it, since a warning nobody has to act on is cheaper than a change to the guard. Across ten
   whole-binary runs under added load the two condenser sandboxes emitted five such lines in total, while
   the rest of the suite emitted between 8 and 38 per run, so these two are not what makes the line
   noisy.

## What was not done

- No production or test source was changed, and no test was deleted.
- `just test-rust` was not run in full. Only the `hooks` binary of the pns workspace was drilled, so
  nothing here says anything about the other pns test binaries or the other four workspaces.
- The drill never ran in CI.
- The audit's claim that no current failure was demonstrated is confirmed for the condenser and
  contradicted for the two tests named above, under the wide reading of load only.
