# S9 re-land and decomposition design

- Date: 2026-07-15
- Status: approved (design), pending spec review

## Context

S9 (the osquery three-tier alerting slice) was merged to `main` as merge commit `c69baab` (pull request
#52). The merge landed without the pre-merge human review the operator wanted, and the change is too
large to review in one sitting: 52 files and roughly 4,300 lines of real content once the generated
`graphify-out/graph.json` is excluded. A review that large lets bugs hide, which is the exact problem
this work exists to solve.

This repository is a chezmoi dotfiles tree. The live machine (dresden) applies from the
`integration/modernization` branch, not from `main`, until the D1 cutover. So `main` has no effect on the
running machine yet, which means S9 can be reverted off `main` and re-landed with zero live impact.

The broader program (SP2) carries work from `integration/modernization` to `main` in reviewable slices.
S9 was carried prematurely as one oversized pull request. This design re-does that carry as an ordered
series of small, self-supporting pull requests. Along the way it relocates the osquery feature-set into
its own directory, breaks the two oversized files S9 introduced into single-responsibility units, and
uses the per-slice review to improve the feature-set.

## Goal

Re-land the S9 osquery feature-set as a dependency-ordered stack of small, self-supporting pull requests
that can each be reviewed in one sitting, and use that review to make the feature-set better. Each pull
request:

1. is self-supporting: when merged on top of the earlier ones it references nothing that has not shipped
   yet, and its own tests pass;
1. is small enough to review in a single sitting, with no single file over roughly 120 lines; and
1. re-lands its part of the feature-set.

Behavior drift from `c69baab` is expected and welcome. The point of the split is to catch bugs, add
behavior where appropriate, and land better refactors during review. `c69baab` is the starting reference
for the re-land, not a fidelity target: the end state should be at least as correct as `c69baab`, and
better where review improves it. Every intentional divergence is called out in the pull request that
makes it.

## Constraints and standing policy

- Self-supporting is a hard constraint, enforced by the dependency graph, not by file count.
- Each slice gets one Sol (ultra effort) adversarial review pass, then operator review, then merge. No
  auto-merge. This holds for everything after S8 (the standing 2026-07-12 merge gate).
- Feature-branch workflow: pull requests merge to `main` on GitHub with subject
  `Merge pull request #N from webdavis/<branch> (#N)` using `--merge`. Never rebase onto `main`.
- No em-dashes anywhere. Bash style per the repository rules (`set -euo pipefail`, quoted expansions,
  GNU-first tool fallbacks).
- Every pull request description must be run through the `/humanizer` skill.
- Every pull request carries chain-of-thought verification before its review: the implementing agent
  explains its reasoning step by step first, and Sol then explains its reasoning for each finding it
  produces. See the per-slice process.

## Mechanics

### Revert then re-land

`main`'s tip is `c69baab` with nothing stacked after it, so S9 is cleanly removable.

1. Revert: `git revert -m 1 c69baab` on a branch, opened as a mechanical pull request. This is the exact
   inverse of an already-shipped change, so it is not itself a review target. After it merges, `main` no
   longer contains S9.
1. Re-land: ship the slices below in order, each merged only after operator review.

Revert is used rather than a hard reset because resetting `main` would require a force-push to `main`,
which is prohibited.

### Interaction with S10 and S11

Both `feat/s10-macos-defaults-ssh` (pull request #53) and `chore/s11-shell-brew-cache` branched from
`c69baab`, so their trees physically contain S9's files. After the S9 re-land completes on `main`, these
branches are rebased onto the rebuilt `main`. Because the re-land relocates the feature-set and may drift
from `c69baab`, that rebase will surface real conflicts rather than a clean drop-out, and they are
resolved by hand at rebase time. S10 and S11 have no functional overlap with S9, so the conflicts are
path and adjacency noise, not logic collisions.

## Feature-set relocation

Every osquery executable and sourced library moves out of `~/.local/bin/` into its own directory,
`~/.local/libexec/osquery/`. The `libexec` tree is the idiomatic Unix home for programs that other
programs run (here, launchd), not programs a human runs at a prompt, and it mirrors the category-first
layout the rest of `~/.local/` already uses (`bin`, `lib`, `share`, `state`, `log`). The chezmoi source
moves correspondingly, from `dot_local/bin/executable_osquery-*.sh` to
`dot_local/libexec/osquery/executable_*.sh`.

The redundant `osquery-` filename prefix is dropped, because the directory now supplies that namespace:
`osquery-digest.sh` becomes `digest.sh`, `osquery-alert-dispatch.sh` becomes `alert-dispatch.sh`, and so
on. The full path (`~/.local/libexec/osquery/digest.sh`) stays self-documenting, and the plist and
manifest references show the `osquery` directory.

Scope is the whole feature-set, not only the files S9 introduced. Pre-existing members
(`enrich-finding.sh`, and the launchd plists for results-alerter, firewall-gatekeeper, and
uptime-watchdog, which already point at the old `bin/` paths) move too. A partial move would leave the
feature-set half scattered, which defeats the purpose.

Every reference to an old path is repointed in the same change that moves the file it names: launchd
plist `ProgramArguments`, the pipeline-integrity manifest paths, the cross-script `source` lines (each
runtime script sources the dispatch library, and the alerter sources its fragments, by absolute path
under the new home), and any test that names a program path. A repository-wide sweep for the old
`~/.local/bin/osquery-` and `dot_local/bin/executable_osquery-` strings confirms none survive.

## The two decompositions

Sol (gpt-5.6-sol, ultra effort) reviewed the two oversized files S9 introduced and found both
over-scoped. The operator accepted both decompositions. The decomposition doubles as design input for the
SP3 Rust rewrite: the single-responsibility bash module boundaries may become the domain boundaries the
Rust version encodes, but they are a guide, not a rule, with the target rule being SOLID design.

### Test fixture: facade over per-area modules

`test/fixtures/osquery-alerter-lib.bash` (674 lines, 61 functions, loaded by 20 test files) is an omnibus
harness for eight unrelated programs sharing one global namespace. It is replaced by a thin facade of the
same name that sources a new `test/fixtures/osquery-alerter/` module directory. The facade derives the
module directory from `${BASH_SOURCE[0]}`, so existing `.bats` files keep their current `load` line and
no other test file changes. The fixture stays under `test/fixtures/`; it is test tooling, not part of the
runtime feature-set, so it is not relocated to `libexec`.

Modules (each carries only its own program's setup, run, and assertion helpers):

- `core.bash`: the shared temporary HOME, the fixed dispatch-stub path, the alert-capture protocol,
  teardown, and the assertions that interpret the capture log (`assert_mode`, `assert_no_dispatch`).
- `rows.bash`: the osquery row builders (`row`, `file_event_row`).
- `results.bash`: the alerter program constant, the public `setup_harness` wrapper, seed helpers, the
  alerter runner, and the cursor runner and reader.
- `dispatch.bash`: dispatcher setup, polling, spool, and POST helpers.
- `delivery-e2e.bash`: the end-to-end redaction and hard-fail spool harnesses.
- `digest.bash`: digest record, seed and run helpers, digest assertions.
- `poller.bash`, `watchdog.bash`, `heartbeat.bash`, `allowlist.bash`, `tailscale.bash`: one leaf harness
  each (path constant plus setup, run, and assert helpers).

The irreducible shared core is the temporary HOME, the fixed dispatch-stub path, the capture-log
protocol, teardown, and the assertions that read that protocol. Everything else is per-program.

Effect on the stack: the fixture never lands as a single lump. `core.bash` and `rows.bash` plus the
facade land in the test-foundation slice; every other module ships inside the slice that needs it
(roughly 50 to 100 lines per slice).

### Alerter: sourced single-responsibility fragments

`osquery-results-alerter.sh` (495 added lines, 703 total) combines six failure domains with the delivery
checkpoint under one `set -euo pipefail` scope: log snapshot and cursor, input normalization, three
independent trust or persistence mechanisms, finding routing and enrichment, presentation rendering, and
the coupling of delivery success to checkpoint advance. A parser, policy, digest, or renderer edit can
disturb the security-sensitive invariant that the cursor advances only after durable handling.

The script is split into single-responsibility bash files under the feature-set home. The entry script
`~/.local/libexec/osquery/results-alerter.sh` (the file launchd runs) carries the shebang, the
`set -euo pipefail` preamble and environment, the `source` lines, and `main`. Its sourced helpers live in
`~/.local/libexec/osquery/results-alerter/`:

- `normalize.sh`: raw osquery log rows into one normalized JSON object per finding.
- `allowlist-verdict.sh`: the known-good persistence-allowlist judgment.
- `pipeline-verdict.sh`: the integrity-manifest judgment.
- `digest-store.sh`: appending a finding to the daily-digest spool.
- `route.sh`: the routing matrix (page, digest, or log-only), emitting only page candidates and owning
  the digest side effect.
- `render-page.sh`: page candidates into the notification body.

The helpers only define functions and communicate through newline-delimited JSON: `normalize` emits
findings, `route` emits page candidates, `render-page` emits the page count and body. Only the entry
script's `main` may call `send_alert`, `_checkpoint`, or `exit`, so the cursor-advance invariant lives in
one small, readable place that a helper cannot reach. This is the same runtime `source` pattern the
feature-set already uses for the dispatch library, so it introduces no new mechanism.

Because the helpers only define functions and `main` controls the call sequence, source order does not
affect behavior. The alerter still ships as a single pull request, because the entry script sources all
six helpers at startup and cannot run or be tested without them: the irreducible unit is the entry script
plus its helper set. The pull request is granular internally (seven small files, each reviewed on its
own). The alerter's roughly twelve behavior tests peel off into the following slices. The
pipeline-integrity manifest hashes the entry script and all six helpers at their new paths.

## The re-land stack

The revert plus fifteen slices, each a self-supporting pull request, in dependency order. "Brings its
own" names the fixture module and other new files that ship inside that slice. All runtime scripts land
under `~/.local/libexec/osquery/` with the `osquery-` prefix dropped.

| #   | Pull request         | Brings its own                                                                               | Notes                               |
| --- | -------------------- | -------------------------------------------------------------------------------------------- | ----------------------------------- |
| 0   | Revert S9            | (none)                                                                                       | mechanical inverse of `c69baab`     |
| 1   | Relocate feature-set | move pre-existing osquery files to `libexec/osquery/`, drop prefix, repoint refs             | behavior-preserving move only       |
| 2   | Test foundation      | test-guard, guard test, fixture facade, `core` and `rows` modules                            | shared bedrock                      |
| 3   | Dispatch library     | `alert-dispatch.sh`, `dispatch` module, dispatch and spool tests                             | sourced by every runtime script     |
| 4   | Config base          | osquery.conf, three packs, agent-attack-surface, setup script, config tests                  | config queries later slices key on  |
| 5   | Allowlist            | `allowlist.sh`, allowlist-file rename, `allowlist` module, test                              |                                     |
| 6   | Alerter              | `results-alerter.sh` entry, six sourced helpers, `results` module, smoke test                | one pull request, seven small files |
| 7   | Alerter tests A      | gate, baseline, cursor (integration and end-to-end), redaction, `delivery-e2e` module        |                                     |
| 8   | Alerter tests B      | fileevents, posture                                                                          |                                     |
| 9   | Alerter tests C      | agent, persistence, agent-binary-honesty                                                     |                                     |
| 10  | Poller               | firewall-gatekeeper monitor, `poller` module, poller and screenlock and logonly tests        |                                     |
| 11  | Digest tier          | digest script, plist, loader, osquery.yaml, `digest` module, digest tests                    |                                     |
| 12  | Heartbeat tier       | heartbeat script, plist, loader, `heartbeat` module, heartbeat test                          | canary query already in slice 4     |
| 13  | Tailscale tier       | tailscale script, plist, loader, `tailscale` module, tailscale and launchagent-loaders tests |                                     |
| 14  | Watchdog             | watchdog script, `watchdog` module, watchdog test                                            | needs slices 11 to 13 labels        |
| 15  | Pipeline manifest    | the manifest runner                                                                          | last: it hashes every prior script  |

### Ordering rationale

- The relocation (slice 1) runs first so every later slice creates or re-lands its files directly in the
  new home, and no later slice carries move noise. It moves only the pre-existing feature-set files that
  survive the revert; S9's own files are created fresh under the new home in their slices.
- The dispatch library (slice 3) is sourced by all six runtime scripts, so it precedes them.
- The config base (slice 4) introduces the query names the alerter and several config tests key on, and
  its `includeTemplate` requires the agent-attack-surface pack file, so they ship together.
- The allowlist file and reader (slice 5) precede the alerter, which reads the shared allowlist path.
- The alerter (slice 6) depends on slices 3, 4, and 5. Its behavior tests follow in 7 to 9 because tests
  depend on the script, never the reverse.
- The three notification tiers (slices 11 to 13) are siblings; the launchagent-loaders test ships in the
  last of them because it asserts on all three.
- The watchdog (slice 14) references the digest, heartbeat, and tailscale agent labels, so it follows
  their slices.
- The pipeline manifest (slice 15) hashes every pipeline script and plist and pages if one is missing, so
  it ships last, after every file it fingerprints exists at its final path.

## Per-slice process

For each slice, in order:

1. Build the slice on a branch off the current `main`.
1. Run the corresponding tests plus the repository check command.
1. Chain-of-thought verification: before any review, the implementing agent explains its reasoning step
   by step, what it changed, why, how the change preserves or intentionally alters behavior, and how it
   verified correctness. This reasoning is recorded in the pull request.
1. One Sol (ultra effort) adversarial review pass. Sol explains its reasoning for each finding it
   produces, the same way, not just the verdict. Address the findings.
1. Operator review.
1. Merge on approval; move to the next slice.

Where review turns up a bug or a worthwhile improvement, fix it in that slice and call the change out in
the pull request description. The drift from `c69baab` is intended.

## Verification

- Each slice: its own tests and the repository check command pass. A slice that intentionally changes
  behavior states the change and its reason in the pull request.
- Relocation (slice 1): every repointed reference resolves (each plist names an existing program, each
  `source` line names an existing file), the applied or rendered tree is consistent, and the path sweep
  finds no surviving `~/.local/bin/osquery-` or `dot_local/bin/executable_osquery-` reference. The move
  changes no logic.
- Refactored fixture: existing `.bats` files are unchanged and pass; the facade sources the modules
  correctly.
- Refactored alerter: the entry script sources its six helpers and passes the alerter test suite; the
  manifest fingerprints all seven files at their new paths.
- End state: `main` carries the full osquery feature-set under `~/.local/libexec/osquery/`, at least as
  correct as `c69baab` and better where review improved it.

## Risks and open items

- The relocation repoints many references (plists, manifest, cross-script `source` lines, tests). A
  missed reference breaks at apply or run time. Mitigated by repointing each reference in the same change
  that moves its file and by the path sweep in slice 1's verification.
- The alerter ships as one pull request because its entry script cannot run without its helper set. Slice
  6 is therefore the heaviest review even though every file in it is small.
- S10 (#53) and S11 remain unmerged during the re-land and are rebased onto the rebuilt `main` afterward,
  resolving path conflicts by hand. They must not be merged into `main` before the re-land completes, or
  they would reintroduce S9's files at the old paths ahead of the sliced re-land.

## Out of scope

- Subsystems outside the osquery feature-set: S10, S11, S12, the D1 cutover, and SP3, except for the
  rebase handling noted above.
- The SP3 Rust rewrite itself. This design informs it but does not implement it.
