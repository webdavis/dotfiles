# The test baseline, and how a later step diffs against it

`docs/test-baseline.tsv` is the recorded state of this crate's test suite on 2026-09-02, at `origin/main`
commit `413eb8d0`, before any refactoring work. It is a SET OF NAMES with results, and it is deliberately
not a count.

A count is worthless as a safety net here. It passes when one test is dropped and another added, and it
passes a rename, which is exactly how a behavioral contract goes missing during a refactor without anyone
noticing. This program has already been bitten by that.

## The shape of the file

Three tab separated columns, with a header row, sorted by target and then by test name:

```
target	test	result
src/lib.rs	args::tests::a_bare_flag_is_not_given_a_value	ok
tests/dispatch.rs	racing_present_events_adopt_one_stranded_claim_exactly_once	ignored
```

1,257 rows. 1,256 `ok` and 1 `ignored`.

| Target                         | Tests |
| ------------------------------ | ----- |
| `src/lib.rs`                   | 748   |
| `src/main.rs`                  | 70    |
| `tests/dispatch.rs`            | 212   |
| `tests/hooks.rs`               | 163   |
| `tests/daemon.rs`              | 22    |
| `tests/native.rs`              | 16    |
| `tests/setup.rs`               | 14    |
| `tests/config_render.rs`       | 8     |
| `src/bin/pns-config-render.rs` | 4     |

`src/bin/http-capture.rs` builds a test target that contains no tests, so it contributes no rows.

The one ignored test is `tests/dispatch.rs:racing_present_events_adopt_one_stranded_claim_exactly_once`,
marked `#[ignore = "soak: a probabilistic hunt, roughly one catch in 200 rounds"]`. It stays ignored and
stays in the baseline, because a soak that is deleted rather than skipped is a contract that quietly
stopped existing.

Some names appear under more than one target. The seven `support::guard_tests::*` names are compiled into
every integration target, so the pair (target, test) is the key, not the test name alone.

## How the file was produced

The result column came from a full run, which passed:

```
cargo test --locked --manifest-path dot_local/share/pns/Cargo.toml
```

The NAME column did not come from that run's output, and this matters. A test that prints to standard
output can interleave with the harness's own `test <name> ... ok` lines, and on this suite it did: a
speed-guard warning merged with the following result line and produced a name that no test has. Names are
therefore taken from the deterministic listing:

```
cargo test --locked --manifest-path dot_local/share/pns/Cargo.toml -- --list
```

Regenerate with the same two commands. Take names from `--list` and results from the run, never names
from the run.

## How a later step diffs against it

The comparison is on NAMES, in both directions, and the target column is informational because it will
change: the workspace conversion moves these tests into per-crate targets, and the crate name will appear
in place of `src/lib.rs`.

```
cargo test --locked --workspace -- --list \
  | sed -n 's/: test$//p' | sort -u > after.txt
cut -f2 docs/test-baseline.tsv | tail -n +2 | sort -u > before.txt
diff before.txt after.txt
```

Every line the diff reports is answered in the pull request that caused it, in a table with one row per
name:

| Baseline name | Successor name | Category | Reason |
| ------------- | -------------- | -------- | ------ |

- A test that survives unchanged needs no row.
- A test that is RENAMED gets a row naming its successor. This is the case the count would have missed.
- A test that is SPLIT gets one row per successor.
- A test that is REMOVED gets a row with an empty successor and a reason drawn from the categories below.
  "It tested the old mechanism" is a reason only when the specification it was protecting is named, and
  is shown to be covered elsewhere.

A removal with no row is a regression in the review, not in the code, and is treated as one.

## The other half of the safety net

The name set proves a move dropped no TEST. It cannot prove a move dropped no BEHAVIOR, because a
behavior that no test pins leaves the set unchanged when it breaks.
`docs/specs/unpinned-behaviors.md` is the list of those, and the rule that goes with it: before a later
pull request moves the code behind one of them, it writes the missing test first, against the code in its
current location, and lands it before the move. That list is the known minimum rather than a complete
one, so the per-behavior mutation check stays required whether or not a behavior appears on it.

## Test classification

This is step 3 of the refactoring procedure. It classifies at the level of target and module, with the
individually identified exceptions named. Classifying 1,257 tests one by one would go stale on the first
pull request and would pretend to a precision the refactor has not earned yet, so what follows is the
criterion plus the exceptions found while writing the specifications.

### The criterion

Ask what a test would still be pinning after the mechanism beneath it is replaced.

1. **Permanent behavioral contract.** It pins something observable from outside: an exit code, exact
   operator-facing wording, a fail direction, a threshold and the step either side, an idempotency
   guarantee, a privacy guarantee, a process-cleanup guarantee. It survives the refactor, under its own
   name or under a named successor.
1. **Adapter contract.** It pins one adapter's behavior against controlled infrastructure: a scripted
   transport, a temporary directory, an exact argv, a fixture. It survives, and moves to the crate that
   ends up owning that adapter.
1. **Obsolete implementation-mechanism test.** It pins HOW the current implementation reaches a result,
   where the refactor deliberately replaces the mechanism. It is removed, and its row names the
   specification that still covers the behavior.
1. **Migration test.** It exists to prove a transition: legacy state being swept, a configuration
   migration, or a pin that has to leave this crate.

### Group assignment

| Group                                                                                                                                                                                                                                            | Rows             | Category                      | Note                                                                                                                                                          |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tests/dispatch.rs` root tests                                                                                                                                                                                                                   | 205              | permanent behavioral contract | Black box against the built binary. The program requires this mega-suite be split BY BEHAVIOR, never into `part1` and `part2`                                 |
| `tests/hooks.rs` root tests                                                                                                                                                                                                                      | 156              | permanent behavioral contract | Same, and it carries the hook compatibility surface                                                                                                           |
| `tests/daemon.rs` root tests                                                                                                                                                                                                                     | 15               | permanent behavioral contract | Job, claim and lease behavior across processes                                                                                                                |
| `tests/native.rs` root tests                                                                                                                                                                                                                     | 9                | adapter contract              | The compiled-in destinations, which the dispatch suite deliberately does not reach                                                                            |
| `tests/setup.rs` root tests                                                                                                                                                                                                                      | 7                | permanent behavioral contract | Terminal echo restoration and publication safety are operator-safety contracts                                                                                |
| `tests/config_render.rs`                                                                                                                                                                                                                         | 8                | mixed, see exceptions         | The renderer's own behavior is permanent; the one test pinning the dotfiles template is a migration test                                                      |
| `support::guard_tests::*`                                                                                                                                                                                                                        | 7 names, 34 rows | adapter contract              | They test the speed guard in `tests/support/mod.rs`, which the program explicitly says to keep. They follow the guard wherever shared test support lands      |
| `src/lib.rs` pure-policy modules (`engine`, `surface`, `routing`, `presence`, `pulse`, `quiet`, `safety`, `render`, `decision_log`, `missed_notifications`, `nag`, `args`, `registry`, `lights`, `recap`, `daemon`, `setup`, and the crate root) | 354              | permanent behavioral contract | These are total functions of their arguments. They move into the domain crate largely unchanged, which is the cheapest evidence the refactor preserved policy |
| `src/lib.rs` edge modules (`system`, `home`, `config`, `config_text`, `doctor`, `focus`, `hooks`, `channels::*`)                                                                                                                                 | 394              | adapter contract              | Each is a seam over a real external thing, tested against a stub or a fixture                                                                                 |
| `src/main.rs` unit tests                                                                                                                                                                                                                         | 70               | mixed, see exceptions         | The composition root's private file protocols. This is where the obsolete-mechanism candidates concentrate                                                    |
| `src/bin/pns-config-render.rs`                                                                                                                                                                                                                   | 4                | permanent behavioral contract | The secret-marker refusals, which are a safety contract                                                                                                       |

### Named exceptions

**Migration tests, leaving or proving a transition.**

- The five tests that reach four directories above the crate into the dotfiles checkout, to pin
  `dot_config/pns/private_config.toml.tmpl` against `dot_config/pns/config-values.toml`. Four are unit
  tests in `src/config.rs` reading `include_str!("../../../../dot_config/pns/...")`, and one is
  `tests/config_render.rs:the_binary_over_the_committed_values_file_writes_the_committed_template_exactly`
  building the same paths from `CARGO_MANIFEST_DIR`. It is a dotfiles concern, not a pns concern, and a
  standalone crate cannot carry it. It moves out to a test under `test/` that runs the built renderer,
  and pns keeps its renderer tests against fixtures it owns. See
  `docs/decisions/0011-the-shipped-template-is-pinned-from-outside-the-crate.md`, which records the two
  properties the move must not lose.
- `src/main.rs:tests::the_first_tick_sweeps_the_state_the_old_names_held`, which proves the legacy
  `lights-glow`, `lights-working-since` and `lights-needs` entries are removed. It stays as long as the
  sweep is deployed. See `docs/decisions/0004-the-unread-lamp-and-the-glow-it-replaced.md`.

**Candidates for obsolete implementation-mechanism, decided per pull request, not now.**

The persistence step replaces internal durable multi-record state with a transactional store. Where that
happens, the tests that pin the filesystem protocol beneath it become mechanism tests. Where it does not
happen, because the path, name, mode or existence is itself an external interface, they stay permanent
contracts and their race behavior must still be tested.

The decision is per state family, not per test, and it is made when that family moves. What must NOT be
lost in either case is the behavior these tests actually protect:

- ownership taken by rename or exclusive creation rather than by removal
  (`docs/decisions/0001-ownership-by-rename-not-by-unlink.md`),
- a stale hold being reclaimable,
- a claim being taken at most once under contention,
- a crash between two steps leaving a recoverable state rather than a duplicate delivery.

A pull request that deletes such a test and cannot point at where those four are still pinned has found a
gap, not an obsolete test.

**Not a test, and not to be written.** A file-size check is not a test. Tests here pin the behavior of
tools we wrote (operator ruling, 2026-08-05), and a meta-test about code shape is deleted on sight. The
file-size command is run in the completion report instead.
