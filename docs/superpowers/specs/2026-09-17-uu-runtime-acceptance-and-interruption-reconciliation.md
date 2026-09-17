# uu runtime acceptance and interruption reconciliation

Status: reconciliation, written 2026-09-17 on dresden against local main (`efcc8955`) with the
worktree at `docs/uu-runtime-acceptance-reconciliation`. No Rust file was changed, no config
changed, and no scheduled or manual run was triggered by this pass. Evidence below is either a
file-and-line citation in the checked-out source, a git or `gh` command against real history, a
read-only `ls`/`cat`/`launchctl print` against the live machine, or a `cargo test`/`cargo fmt` run
against the unmodified `uu` workspace.

What this reconciles: ledger task 57a, filed against the interruption defect and duplicate-log
finding from 2026-09-13, read as a set of claims to check rather than as facts already settled.

Finding in one line: **the whole code half of task 57a shipped in one pull request, PR #542, covers
both signals, and passes on this machine tonight; what remains is entirely the operator's clock**,
the Sunday-noon launchd firing that has not happened yet, plus the one new lane whose state
directory waits on that same firing.

## Disposition per claim

| # | Claim | Verdict | Evidence |
|---|-------|---------|----------|
| 1 | SIGINT/SIGTERM cleanup + durable start/interruption records | **shipped** | see below |
| 2 | Scheduled output no longer duplicates uu's own log | **shipped** | see below |
| 3a | No successful scheduled-run marker existed | **operator-owned, still open** | see below |
| 3b | Sunday-noon job had not run | **operator-owned, still open** | see below |
| 3c | Twelve lanes lacked state directories | **shipped, one exception expected** | see below |
| 4 | cua-driver drift resolved | **confirmed** | see below |
| 4 | 21 preserved plugin-history rows | **confirmed** | see below |

### 1. The interruption defect (both signals)

The brief specifically warns that a fix satisfying the SIGTERM test name alone would still leave
the reported SIGINT defect live. That is not what shipped: `install_interruption` in
`uu/crates/uu-adapters/src/interruption.rs:14-24` registers the **same** handler for both
`libc::SIGINT` and `libc::SIGTERM` in one loop, writing into one lock-free atomic
(`REQUESTED`, line 3). Every consumer downstream reads that one atomic through
`crate::interruption()` (`interruption.rs:27-31`) with no branch on which signal arrived:
`uu/crates/uu-adapters/src/watchdog.rs:96` (refuse a not-yet-spawned command),
`watchdog.rs:129-136` (shrink the spawn-wait deadline once interruption is seen), and
`uu/crates/uu-adapters/src/watchdog/wait.rs` (`settle`, `wait_bounded`) which is what actually
walks the child down: SIGTERM to the whole process group, a grace, then SIGKILL to the group
(`wait.rs:70-79`), before the caller's lock is ever released. `uu/crates/uu/src/main.rs:20-26`
installs the handler before dispatch and maps whichever signal fired to the conventional
`128 + signal` exit code on the way out.

The test file carries one shared assertion function, `interrupted(signal, name)`
(`uu/crates/uu/tests/interruption.rs:86-171`), exercised twice:
`sigint_cleans_owned_children_before_unlocking_and_records_interruption` (line 173) and
`sigterm_cleans_owned_children_before_unlocking_and_records_interruption` (line 178). Both assert
the same four things: the child and grandchild are stopped and the lock was still held while they
were (`lock-held` files, line ~150), the log contains `uu: run started` before the signal and
`uu: run interrupted` after (lines ~140, ~163), later lanes and the alert binary never ran
(`late-ran`/`alert-ran` absent), and the run's own lock is free again once the process exits.

```
$ cargo test -p uu --test interruption -- --test-threads=1
running 3 tests
test child_fixture ... ignored
test sigint_cleans_owned_children_before_unlocking_and_records_interruption ... ok
test sigterm_cleans_owned_children_before_unlocking_and_records_interruption ... ok
test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.45s
```

`git log --oneline -- crates/uu-adapters/src/interruption.rs crates/uu/tests/interruption.rs`
shows one commit created both files together: `eca4219f` ("fix(uu): stop owned children on
interruption"), dated 2026-09-13, the same day as the audit. Both signal tests were added in that
same commit, not added later for one signal only. `gh pr view 542` confirms
`state: MERGED`, `mergedAt: 2026-09-13T13:44:41Z`, merge commit `0ec1e22f`, and
`git merge-base --is-ancestor 0ec1e22f... HEAD` returns true against this worktree's `main`.
**Verdict: shipped, both signals, not a SIGTERM-only fix.**

### 2. Duplicate scheduled output

`Library/LaunchAgents/com.webdavis.uu.plist.tmpl` now reads `StandardOutPath` as `/dev/null` and
`StandardErrorPath` as `{{ .chezmoi.homeDir }}/.local/log/uu/launchd-stderr.log`, a path distinct
from uu's own application log at `~/.local/log/uu/uu.log` (confirmed live: that file exists
separately at `~/.local/log/uu/uu.log`, 24673 bytes, and holds the single application record; the
launchd stderr file exists beside it). `git log --oneline` on that plist template shows commit
`004ff186` ("fix(uu): separate launcher errors from run records"), the same day and
the same PR #542 as the interruption fix above, whose message states the fix plainly: "Keep one
writer for the run log, retain startup failures in a separate stderr log, and include that log in
the existing rotation lane." That commit also added `uu/crates/uu/tests/logging.rs` (76 lines) to
exercise the routing. Rendering the template against this checkout (`CI=1 chezmoi --source "$PWD"
execute-template --no-tty` on the plist) is unnecessary evidence here since the deployed copy on
this machine already reflects it and `launchctl print gui/501/com.webdavis.uu` echoes the same two
distinct paths back. **Verdict: shipped.** One application record per run, startup errors routed
elsewhere.

### 3a/3b. Scheduled-run marker and the Sunday-noon job

Both audit findings are **unchanged tonight**, and that is expected rather than a regression:
2026-09-17 is a Thursday, and the plist's `StartCalendarInterval` fires at `Weekday 0` (Sunday),
`Hour 12`, `Minute 0`. Live evidence:

```
$ launchctl print gui/501/com.webdavis.uu | grep -E 'runs =|last exit code'
	runs = 0
	last exit code = (never exited)
```

`runs = 0` is the ground truth that the loaded job has never fired since it was loaded. A
`last-success` marker does now exist on disk (`~/.local/state/uu/last-success` reads
`1789630720 2026-09-17T07:38:40Z`), and `uu doctor` reports "last successful run:
2026-09-17T07:38:40Z (10h 3m ago)", but that timestamp is this morning's manual `uu run`, not a
launchd firing, exactly the substitution the ledger entry warns against accepting. **Verdict:
operator-owned, still open.** No manual run and no notification HTTP 200 stands in for it; the
first real evidence is the next Sunday 12:00 firing, 2026-09-20.

### 3c. Twelve lanes lacking state directories

The live config template (`dot_config/uu/private_config.toml.tmpl`) declares 19 `[lanes.*]` blocks.
`~/.local/state/uu/lanes/` on the machine tonight holds 18 subdirectories, dated 2026-09-13 12:24
(one, `rotate-logs`, updated 2026-09-17 by this morning's manual run). The one lane with no
directory yet is `uv-graphify-skill`, which is exactly the lane ledger task 57c added and which
that same entry already states has not had its first weekly exercise. `uu doctor` lists it as
`on (command)` and resolvable (`/usr/bin/env, found`), so it is enabled and correctly configured; it
has simply never run, the same reason it has no directory. **Verdict: shipped**, in the sense that
the twelve-lane gap from the 2026-09-13 audit is gone; the sole remaining gap is a new lane waiting
on the same Sunday firing as 3b, not a defect.

### 4. cua-driver drift and the 21 plugin-history rows

```
$ ls -la ~/.local/bin/cua-driver
lrwxr-xr-x  1 stephen  staff  53 Jun 25 21:20 /Users/stephen/.local/bin/cua-driver -> \
  /Applications/CuaDriver.app/Contents/MacOS/cua-driver
$ wc -l ~/.local/state/uu/lanes/claude-plugins/snapshot.tsv
      21 /Users/stephen/.local/state/uu/lanes/claude-plugins/snapshot.tsv
```

Both hold. **Verdict: confirmed as RESOLVED**, no correction needed.

## Full-suite evidence (unmodified `uu` workspace)

```
$ cargo test --workspace --manifest-path uu/Cargo.toml 2>&1 | grep -E 'test result|FAILED'
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.20s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.26s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.77s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.13s
test result: ok. 514 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 3.78s
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
(four doc-test targets, 0 tests each, all ok)
$ cargo fmt --all --check --manifest-path uu/Cargo.toml; echo "exit: $?"
exit: 0
```

No Rust file in `uu/` was touched to produce this result; it is the state of `main` as merged.

## What is genuinely left, in order

1. **Operator, on the clock, not forceable.** Let the loaded `com.webdavis.uu` LaunchAgent fire at
   its next Sunday 12:00 slot (2026-09-20). Do not run `uu run` by hand and do not send a test
   notification as a substitute; neither establishes scheduled acceptance per the ledger entry.
2. **Operator, after that firing, capture:**
   - `launchctl print gui/501/com.webdavis.uu | grep -E 'runs =|last exit code'`, expecting
     `runs = 1` (or higher) and `last exit code = 0`; this is what distinguishes a real firing from
     another manual run.
   - `tail -c 4000 ~/.local/log/uu/uu.log`, expecting a `uu: run started <ISO near Sunday noon>`
     line, per-lane verdicts, and a `=== done, N failure(s), D deferred, P pending ===` line with no
     `uu: run interrupted` anywhere after it.
   - `cat ~/.local/state/uu/last-success`, expecting its timestamp to match that same run, not this
     week's manual one.
   - `ls ~/.local/state/uu/lanes/uv-graphify-skill`, expecting it to now exist, closing 3c's gap.
   - `for f in ~/.local/state/uu/lanes/*/streak; do echo "$f: $(cat "$f")"; done`, expecting every
     lane's streak to reflect this run (0 on success, incremented only for a lane that actually
     failed or was pending).
   - Whatever channel `[alerts]`/`[records]` deliver to (the ledger's "notification result") should
     be checked on the receiving side (phone, Discord, banner), not by re-reading uu's own "posted
     HTTP 200" log line, since that line only proves uu's own POST returned 200 and says nothing
     about whether the run behind it was the scheduled one.
   None of this needs code; it is reading the same four places this reconciliation already read,
   after the one event that has not happened yet.
3. **Nothing else.** The interruption fix, the log-routing fix, the cua-driver path, and the
   21-row plugin history are all confirmed live in source and on the machine; there is no remaining
   code change this reconciliation found evidence for. The Todoist task the ledger entry links
   (`https://app.todoist.com/app/task/6hVrcXJrr8FWG4fM`) still names the interruption work; closing
   it is the operator's call once step 2's evidence lands, and this reconciliation did not touch it.

## Assumptions

- The ledger entry's "PR #542 passed required continuous integration and merged" is the same pull
  request `gh pr view 542` reports (title differs slightly in wording from a possible earlier draft,
  but the number, merge date, and file set all line up with the interruption fix read from source).
- The 2026-09-13 audit's "twelve declared lanes lacked state directories" predates that day's apply;
  tonight's 18-of-19 count is compared against the source template's 19 blocks rather than against
  a re-run of the original audit tooling, since no such tooling is named in the ledger entry.
- `~/.config/uu/config.toml` itself could not be read directly under this session's permission
  settings; `uu doctor`'s own lane listing (19 lanes, matching the template's 19 blocks by name) was
  used as the live cross-check instead.
- No chezmoi apply was needed or run for this task: both fixes are already deployed on this
  machine, confirmed by `launchctl print` and the log/state directory contents themselves, not
  merely by reading source.
