# Remaining work

The open task list for the dotfiles modernization, including pns, posture, uu, lights, Neovim, terminal
review tools and the deferred subprojects. Use the resume order below; task numbers are stable
references.

Updated as tasks complete. Last updated 2026-09-13.

## Where things stand

Audited on 2026-09-12 against `76b37ae4`. On 2026-09-13, #530 through #545 merged and local `main`
fast-forwarded to `origin/main` at `3d08b5a5`. The final #530 head passed
[run 34747415685](https://github.com/webdavis/dotfiles/actions/runs/34747415685). The operator reported
that `chezmoi apply` passed on 2026-09-13 after these merges. Live shortcut and harness acceptance checks
remain pending. Task 68c is published; deployment and acceptance remain separate.

Retained design work remains in [#24](https://github.com/webdavis/dotfiles/pull/24) and
[#51](https://github.com/webdavis/dotfiles/pull/51), carrying security and Forzare work explicitly listed
below. Every pull request from the 2026-09-06 Codex handoff has merged; this file replaces that handoff.

The audit compared Claude session `32606e20-aec4-4ed6-ae95-9b9591ead0e4`, the roadmap and subsystem
plans, current source and worktrees, GitHub issues and pull requests, Todoist's modernization, pns and
dotfiles projects, and selected installed paths and launchd metadata. Three independent subsystem reviews
checked pns/lights, posture/uu and Neovim/Scalebar. No new runtime acceptance, hardware drills, updates
or applies ran during this audit. Historical test results below retain their original dates.

The final sweep also checked the consolidated review backlog, all 48 open tasks across the two dotfiles
projects and pns, the four open pull requests and 14 open issues, and the new homelab/vpp decisions.
Recovered follow-ups are recorded below with their source and disposition. This did not run runtime
acceptance or authorize the deferred builds.

### Resume order and completion rules

The Claude session stopped during branch cleanup when its usage limit was reached. It had removed
worktrees and many temporary branches, but had not started the requested pns tap, lights or posture work.
Resume with a fresh inventory, preserving branches and worktrees that contain retained work.

1. Complete the operator acceptance checks for merged #530 and #531 below. The operator reported a
   successful apply on 2026-09-13. Their source changes, independent reviews, required checks, merges and
   local main synchronization are complete, including matching Scalebar #4. Continue independent work
   while operator interaction checks are pending.
1. Preserve the planning work in 68c and finish repository hygiene, tasks 67 and 68. Obtain approval of
   exact removal candidates before further cleanup; pending cleanup must not block independent fixes.
1. Complete pns tap, tasks 71, 71a, 72 and 71b. Investigate 76 before deciding whether to build 74.
   Handle SSH exposure task 75 separately and verify its listener ownership before choosing a mechanism.
1. Finish lights acceptance and cutover, tasks 61 to 63. Task 64's conditional optimization was declined
   by the recorded measurement. Complete the separate uu deployment check and available Neovim acceptance
   work while waiting for device checks.
1. Finish posture implementation and cutovers, tasks 39 to 50, then cleanup and closure, 58 to 60.
1. Recover the remaining design from #24 and reconcile its security integrations. Hermes owns the
   sandboxed investigation workflow that consumes alerts from posture and other security producers.
1. Follow the deferred-project start gates below, including SP5 research before SP4. SP8 is ON HOLD
   (operator 2026-09-14): the current goal is complete when everything else in this ledger is complete,
   SP8 excluded. Forzare (#51) is PART of the current goal (same ruling) and starts after all other
   non-SP8 modernization work is complete, which supersedes the 2026-09-12 "after SP8" ordering.

Record implementation, merge, deployment and operator acceptance separately. A merged change can still
owe live acceptance or deployed-file cleanup. Continue independent work while an operator check is
pending. Reconcile historical checkboxes and Todoist items against source before rebuilding anything.
Stop-point descriptions name the target state after their prerequisites pass; they are not current
completion claims. A research no-go or an explicitly accepted deferral needs a recorded disposition.

### Remaining subprojects at a glance

| Subproject               | Remaining work                                                                       | Start or decision gate                                                     |
| ------------------------ | ------------------------------------------------------------------------------------ | -------------------------------------------------------------------------- |
| SP3, pns                 | Tap, configuration/device acceptance, historical validation and future platform work | Follow the active queue; future platforms retain their own approval gates  |
| SP4, shell improvements  | Bash aliases, bindings and fzf improvements                                          | After Neovim acceptance and the SP5 verdict                                |
| SP5, xonsh evaluation    | Research compatibility, startup and existing shell integrations                      | Evaluate before SP4; adopting xonsh requires a separate decision           |
| SP6, Neovim              | Socket-validation corrections and overhaul acceptance                                | Implementation fixes can proceed; rendered/device checks need the operator |
| SP7, sweep and backlog   | Remaining tools, installer coverage, research and task/issue reconciliation          | After earlier approved work; scope is listed below                         |
| SP8, macOS agent manager | pstack skills and the GUI for harness tooling and configuration                      | ON HOLD (operator 2026-09-14); outside the current goal                    |
| Forzare, #51             | Bob's executive-assistant implementation                                             | In the current goal; after everything else except SP8                      |

Nix packages and per-project Nix flakes remain available by choice. Managing macOS through nix-darwin and
migrating this repository's secrets to sops-nix are out of scope, reaffirmed by the operator on
2026-09-12. The old SP-nix research gate and Todoist migration task are superseded. macOS configuration
stays with chezmoi; this repository's contributor toolchain stays Homebrew and uv.

The Herdr process-toggle plugin and worktree review launcher also remain planned below; neither has an
assigned SP number. Historical plans retain older labels, including SP5 as Thaw, so use this index and
its dated decisions when choosing the next task.

## Review tools and Scalebar deployment

- [x] Merge [#530](https://github.com/webdavis/dotfiles/pull/530), `feat/tuicr-config`, worktree
  `.worktrees/tuicr-config`. Completed 2026-09-13 under explicit operator authorization. Includes tuicr,
  quota preferences and collector preservation, Plannotator hooks and managed updates, full Herdr
  Annotate, all approved bindings and the Worktrunk collection budget. Independent review found no
  blockers. `env -u NO_COLOR -u REPORT_LIB_PLAIN just ship` passed; final continuous integration passed
  at `61164ef4`. All 110 effective Herdr bindings were collision-free. The two installer scripts
  rendered, passed Bash syntax and ShellCheck, and ran successfully. Plannotator 0.27.14 and the
  plugin-owned plannotator-tui 0.8.0 were verified installed. Source merged as `72f745e2` and local main
  matches upstream.
- [x] Merge [#531](https://github.com/webdavis/dotfiles/pull/531) and owner
  [Scalebar #4](https://github.com/webdavis/scalebar/pull/4). The skill directs bodyweight and isometric
  records to `get_workout_trends`; `get_prs` excludes them. Owner and consumer copies match, 41 focused
  Swift tests passed, and independent review found no blockers. Both repositories' local main branches
  match upstream; Scalebar is at `a9d65f7`. Fitness data and Ivy widgets were not changed.
- [x] Operator deployment: `chezmoi apply` passed, reported by the operator on 2026-09-13 after #530 and
  #531 merged. This records deployment; it does not establish the interaction checks below.
- [ ] Operator acceptance: reload Herdr configuration and restart the harnesses. Review changed Codex
  hooks interactively through `/hooks`. Check quota settings/refresh, all Annotate actions, skill
  discovery and plan-feedback delivery across Claude Code, Codex, Gemini and Hermes. Confirm the tuicr
  skill in Hermes nicodemus. Verify that the deployed Plannotator declarations resolve to usable skills.
  Track acceptance in [6hVr2vCjv3vqwHWM](https://app.todoist.com/app/task/6hVr2vCjv3vqwHWM).
- [ ] Test live Worktrunk selection after apply. The reproduction preserved all 208 branch/path pairs and
  reached the intercepted Herdr opening in about 1.6 seconds with `summary = false` and
  `timeout-ms = 1000`; both tab and workspace paths were covered. This was not a live picker interaction.
  The budget can omit slow status details. Tuicr 0.25.0 still ignores `compact_folders` with a warning;
  retain the requested preference and verify it when a supporting upstream release arrives.

Read-only deployment verification on 2026-09-13 passed `herdr config check`; source and live Herdr
configuration match after TOML parsing. Worktrunk returned all 209 current branch/path pairs in 1.624
seconds. All Plannotator links resolve for Hermes default and its four specialist profiles. Interactive
acceptance remains open. The same verification recovered these source fixes:

- [x] Restore Claude delivery of `plannotator`, `plannotator-annotate` and `plannotator-review`. The
  managed lock suppresses them, but installed Plannotator 0.27.14 supplies hooks without commands or
  skills. Correct the consumer declarations and verify deployment separately.
- [x] Fix the shared skills overlay writer in `uu-adapters`: it appends another `policy` mapping when
  upstream metadata already has one. Deployed `plannotator` and `last30days` metadata fail strict YAML
  parsing with `DUPLICATE_KEY`. Update the existing mapping, preserve other metadata and cover the
  repeated-overlay behavior before regenerating through the supported skills lane.
- [x] Fix verification lifecycle and timing failures. The review-tool collector checks passed 71
  assertions but took 17, 21 and 94 seconds. The full planning check also left two temporary
  `pns failures serve` processes running after completion; both were identified by their test paths and
  stopped. Ensure tests reap their processes and satisfy the repository's one-second rule.

Implementation progress, 2026-09-13: [PR #535](https://github.com/webdavis/dotfiles/pull/535) merged the
Claude delivery fix (`bd2b989c`) and parsed, reversible overlays (`c251e6d5`). All 606 uu tests, full
`just ship`, required checks and independent review passed. The release build passed. Operator apply and
live skills acceptance remain open; the autonomous run did not update the live skills store.
[PR #533](https://github.com/webdavis/dotfiles/pull/533) merged the collector fixture reduction
(`6f2e3e43`), owned daemon-job cleanup (`166b4830`) and color-test isolation. Full `just ship` and
required checks passed. The three collector checks passed in 390 ms total, and cleanup regressions passed
against real detached test deliveries. The next full skill check found a separate gate-test pipe race: an
invalid command may exit before the fixture writes stdin. Reuse the existing early-exit payload helper
while retaining exit-code and forwarding assertions. All nine gate tests passed after that change;
independent review approved it. The separate test-only commit `9432c139` merged in #535. The next local
main synchronization reached `b078de0f`, containing #534 through #538. The original 127 untracked paths
remain preserved.

The posture branch's full check then exposed five native delivery fixtures querying the operator's live
idle and mosh sessions. A test-local presence setup retained their wire, receipt, silence and exit-code
assertions. All 25 native checks passed in 440 ms and full `just ship` passed. Independent review passed
the same 25 checks in 430 ms. Separate commit `a7a00c1c` merged in
[PR #538](https://github.com/webdavis/dotfiles/pull/538), whose required checks passed. Live probe
acceptance remains open.

## Red main

- [x] 1. Fix `ledger_awk_spec: reads FIXED-NOTEST as closed as well, so the skip is a prefix match`. Run
  34284143580, merge of PR #462. It looked like an environment difference because it passed here and
  failed there; it is a flake. The case searched the whole output for the two characters `F5`, every
  output line starts with the register's own path, and CI drew the temporary directory
  `nvim.runner/cF5NkO`. The awk program was never wrong. Fixed by matching the id in its own column.
- [x] 2. Fix the e2e case `Two parallel runs deliver a batch exactly once` (`test/e2e`, the osquery
  alerter concurrency test). Run 34270895202, merge of PR #458. `read` returned 142 for SIGALRM: a loaded
  shared runner did not start bash and reach `send_alert` inside the 250 ms bound. All four bounds in
  that case are ceilings on a subject that never signals rather than measurements, so they were widened
  eightfold. A green run still takes about 800 ms.
- [x] 3. Rerun both workflows and confirm `main` is green.

## Apply blockers, merged as PR #463

- [x] 4. Commit, push and merge the apply fixes: the osquery converge configuration check and its tests,
  the deno `--allow-scripts` addition, the gitconfig fsmonitor exclusion for the lazy.nvim checkouts, the
  CLAUDE.md rule about verifying an apply first, the espanso `,,ca` trigger, and the auto-compact window
  setting. Shipped as nine commits with the two test repairs above.

## Untracked files, decide and clear

- [x] 5. Commit `docs/superpowers/specs/2026-07-15-s9-reland-decomposition-design.md`. It is a real
  approved design and every other design spec is tracked in that directory.
- [x] 6. Trash `HANDOFF-CODEX.md`. It is a handoff for a session that has ended and this file replaces
  what it carried. It also sits in the repository root, where no document belongs.
- [x] 7. Trash `nvim.log`. Two Neovim server-start warnings captured by accident.
- [x] 8. Trash `docs/research/moshi-verbose-daemon.log`. A 109 KB daemon capture from the moshi approval
  investigation, which is closed.

## Deployed leftovers

This repository builds no removal mechanisms, so a merged pull request that retires a file leaves the
deployed copy in place until it is trashed by hand. All five below were verified present on 2026-09-08.

Three sit in `~/.config/nvim/lua/custom_api/` with no chezmoi source, left behind by the standalone
Neovim repository that chezmoi replaced. Nothing in the source tree references any of them.

- [x] 9. Trash `~/.config/nvim/lua/custom_api/delegate.lua`. The v3 overhaul design retires it in favour
  of `coder/claudecode.nvim`.
- [x] 10. Trash `~/.config/nvim/lua/custom_api/helpers.lua`. Last touched December 2025, referenced
  nowhere.
- [x] 11. Trash `~/.config/nvim/lua/custom_api/init.lua`. It builds a module table that requires
  `custom_api.delegate`, and nothing requires it in turn.

The other two were retired by merged pull requests and named in their bodies for hand removal.

- [x] 11a. Trash `~/.local/libexec/herdr-jump.sh`. Replaced by the `herdr-workspace-jump` Rust plugin in
  PR #414.

- [x] 11c. Completed 2026-09-09. The skills lane deployed and the retired updater was booted out. With
  the operator's per-invocation approval, its plist, `agent-skills/update-skills.sh` and
  `helpers/log-entries.sh` were trashed together with 11e's remaining consumers. Their absence was
  verified again on 2026-09-12.

- [x] 11e. Completed 2026-09-09. uu's brew and Claude plugin lanes replaced the old weekly updater and
  plugin reporter. The reporter was booted out, then the retired plists, scripts and empty directories
  were trashed. The retired paths and labels are absent on 2026-09-12. The live `agent-skills/` helpers
  `assert-hermes-superpowers-routing.sh` and `live-reconcile.sh` remain.

- [x] 11d. Clear stale `~/.claude/ide/*.lock` files. A lock whose Neovim is gone makes claudecode.nvim
  open a plain HTTP connection to a dead port and warn `Missing or invalid Upgrade header` on every file
  open. Three were found on 2026-09-08, two of them nearly three days old, held by headless Neovim
  processes an agent had leaked. Cleared, and every remaining lock was verified live by its pid. This
  recurs whenever a headless Neovim is killed rather than quit, so it is worth re-checking, not a
  permanent fix.

- [x] 11b. Trash `~/.local/share/herdr/plugins/herdr-last-workspace` and its link. Folded into
  `herdr-workspace-jump` in PR #418. It was still registered in `~/.config/herdr/plugins.json` and still
  running an event hook on every `workspace.focused`, so it needed `herdr plugin unlink` before the
  trash. Deleting the directory alone would have left herdr pointing at a path that no longer exists.

## Before the first stopping point

- [x] 12. Merge PR #458, the auto-commit spec fix and module rename
- [x] 13. Commit and push the failure-reporting spec
- [x] 14. Push `feat/nvim-pns-wiring` (Neovim task 26, pns.nvim), PR, merge
- [x] 15. Push `feat/uu-tooling-e12-e17` (7 commits), PR, merge
- [x] 16. Merge main into PR #448 (herdr), merge
- [x] 17. Pre-apply verification: build pns and posture, headless Neovim start, zero stderr
- [x] 18. Run `chezmoi apply`
- [x] 19. Reload the herdr configuration for the new keybindings

### STOP POINT A

Everything above is additive. posture has not cut over, so the existing osquery pipeline keeps running
untouched. This is the recommended place to stop and apply.

## The monorepo conversion

Operator ruling 2026-09-09, replacing the earlier extraction plan. The tools STAY in this repository for
now, laid out like a monorepo so that lifting one out later is a move rather than a rewrite. They are
products other people install, so nothing in a tool may assume this repository exists.

Extraction into separate repositories is deferred to the tail; see task 68a.

- [x] 20. Convert to the monorepo layout, as ONE change because half-moved paths are the failure mode:
  move `dot_local/share/{pns,uu,posture,lights}` to `{pns,uu,posture,lights}` at the repository root;
  rename the CLI packages `pns-cli` to `pns`, `uu-cli` to `uu`, `posture-cli` to `posture`, so
  `cargo install --git https://github.com/webdavis/dotfiles pns` reads naturally; install the binaries to
  `~/.cargo/bin` and retire the `~/.local/libexec` rule for these four only, the bash scripts keep it.
  Every caller reads ONE declared value rather than a literal path: the pns and uu LaunchAgents, the
  Claude Code hook table in `modify_settings.json`, the Codex hook installer, `dot_bashrc.tmpl`, and the
  herdr and aerospace keybindings. launchd is the exception that needs the absolute path, because it has
  no PATH. Also update `.chezmoiignore`, the four builder scripts, the justfile, `treefmt.toml`,
  `scripts/treefmt/rust-file-size.sh` and the tests that name the old paths. Verified by experiment on
  2026-09-08: `cargo install --git` finds a package in a nested workspace with NO root `Cargo.toml`, so
  no root workspace manifest is needed and none should be added.

  Shipped as three commits on `refactor/monorepo-layout`. Four things the task did not anticipate:
  `lights-cli` was renamed with the other three, because leaving one command crate on the old suffix
  would have been the tree's only inconsistency. The declared value is `.chezmoidata/rust_tools.yaml`,
  and the bashrc reads it as `"$HOME/{{ .rust_tools.install_dir }}/pns"` rather than an absolute render,
  because a shell rc should expand `$HOME` at runtime. pns's two development binaries went behind
  `required-features = ["dev-tools"]`, since `cargo install` installs every binary a package declares and
  the rename would otherwise have put a bare `http-capture` in an installing user's `~/.cargo/bin`. And
  posture's tracked-path allowlist matches `~/.cargo/bin/posture` EXACTLY rather than by prefix, because
  that directory is shared with every other cargo-installed program on the machine. The aerospace keys
  needed no change: they still call `control-hue-lights.sh`, which is task 62's job to retire.

- [x] 21. ALL FOUR CONFIRMED on 2026-09-09, after the apply. `pns doctor` and `uu doctor` both answer and
  exit 0, and `launchctl list` shows both agents loaded. The fourth needed the operator, because an
  agent's tool shell is not interactive: bash-preexec never loads there, so the shell hook never fires,
  which a `sleep 35` from an agent shell proved by leaving no trace in the decision log. The operator's
  own `sleep 35` was silent at first for the RIGHT reason, and the surface rule is the one to remember:
  the banner belongs to the desk and fires only when the pane that raised it is not the pane on screen
  (`pns-domain/src/surface.rs`, `banner: surface == Surface::Desk && !watching`). Watching the pane it
  ran in suppresses it by design. Switching away before the sleep finished raised
  `shell / done / dotfiles, sleep (36s)`. The mobile push refusal found that day was subsequently fixed
  in PR #503: pane-less cards carried an invalid data object. PR #505 consolidated the token source. The
  Claude session records `pns doctor` reporting `mobile: sent` and `4 sent, 0 failed` on 2026-09-10. That
  establishes endpoint acceptance; it does not record the operator seeing that card on the phone.
  Original text: apply, then confirm every caller still resolves: `pns doctor`, `uu doctor`, a
  `launchctl list` showing both agents loaded, and one real long-running command raising its notification
  through the shell hook. The old binaries under `~/.local/libexec/{pns,uu,posture}/` and
  `~/.local/libexec/lights` are NOT removed by the apply and want trashing once this is confirmed;
  `~/.local/libexec/pns/hooks/` stays, because the Codex hook installer still lives there.

## pns closure and the rescued lanes

- [x] 22. Rework `fix/pns-retry-backoff` against the current crate layout, PR, merge
- [x] 23. Review and push `feat/posture-producer-commands` (heartbeat), PR, merge
- [x] 24. Review and push `feat/posture-converge-staging`, PR, merge. Shipped as
  `feat/posture-converge-foundation`: five of its six commits. The sixth deletes the bash converge and
  routes through the native command, which is the cutover task 50 owns, and the native path still calls
  `osqueryctl config-check`, the bug PR #463 fixed. It is parked until task 50 ports that fix.
- [x] 25. pns 18.1a: the `cargo doc` gate with `RUSTDOCFLAGS="-D warnings"`
- [x] 26. pns 8.4: the Codex and Claude hook-table verification record
- [x] 27. pns: backfill decision record 0012 (SQLite two fail directions)
- [x] 28. pns 18.1b: the completion report and line counts

### STOP POINT B

The pns refactor plan is closed and nothing is stranded.

## Delivery failure reporting

Designed in `docs/superpowers/specs/2026-09-08-pns-delivery-failure-reporting-design.md`, built from
`docs/superpowers/plans/2026-09-09-pns-delivery-failure-reporting-plan.md`.

- [x] 29. Plan the build, the pull request breakdown from the spec. Seven pull requests, one per task
  below, in that order. Three facts from the survey changed the plan: the backoff already landed, so task
  30 removes its jitter rather than adding the type; an `http_status` column already exists with a CHECK
  admitting only four codes, so task 31 has to recreate it rather than add one; and the banner's click
  slot is already free, returning the no-op `:` for exactly the pane-less events every delivery failure
  is.
- [x] 30. Permanent versus temporary classification in `pns-domain`. `DeliveryOutcome`, `FailureClass`
  and `RetryLimits::verdict`, with `DeadletterReason::Permanent`. Permanence outranks both counters, so a
  refused request dead-letters on attempt one. The jitter went with it: `retry_at` is now a pure function
  of the clock and the attempt count, `RetryBackoff::random_secs` is gone, and `retry_random_secs` is
  refused by name rather than ignored, because a key that parses and changes nothing reads as configured
  behavior.
- [x] 31. The ledger columns and the persisted failure record. Migration step 8 widens the CHECK on both
  `http_status` columns to any real status and adds `permanent` to `deadletter_reason`, and the terminal
  decision moves out of the hermes channel and behind `pns_domain::retry`'s permanent class, so a 404 now
  stops at its first queued retry. The attempt keeps whatever status it got, so a retryable 503 no longer
  leaves its code nowhere. The three denormalized leg columns the plan listed were not built: the attempt
  row already carries them, `route` was already on the leg, and a per-delivery write measurably slowed
  the suite. The read path the plan put here lands with its caller in task 33 rather than as an uncalled
  method.
- [x] 32. The message, both render forms, the per-destination meaning tables. `pns_domain::failure`
  rather than `pns-protocol`, whose own contract is that it holds no view of the domain model. The
  notification form enforces its 256-character budget itself, in the order the reader can least afford to
  lose, because a route name is as long as the producer made it and the surface would otherwise cut the
  fix line off the end. `Surface::Terminal` is a separate type from the notification surfaces, since the
  terminal repair names the route inside the fix line and can exceed the whole budget by itself. Two
  design gaps closed while building: the terminal `fix` needed a repair per code, which the design gave
  only as one worked example, and the 413 and 422 rows named no concrete subject despite the design's own
  rule that every meaning must.
- [x] 33. `pns failures` and the `pns doctor` routing to it. The read path, the listing capped at twenty
  newest first, `pns failures <id>` through the full renderer, and the doctor's ledger line naming the
  command when there is something to look at. NO PICKER, so the design's interaction model is not built:
  a printed listing plus an id needs no prompt, and a prompt is the only thing that model exists to
  guard. The `failed command` is reconstructed from the routing facts rather than stored, since a second
  copy of the flags could disagree with the routing it describes. The design's rung 5, a non-zero exit
  for a synchronous producer, is NOT here: it changes what every producer sees and belongs in its own
  change, filed against task 34's PR.
- [x] 34. The `pns doctor` route check. THE PROBE IS AN UNSIGNED POST, measured against the live gateway
  on 2026-09-09: it answers 401 for a route that exists and 404 for one that does not, while GET and HEAD
  answer 405 for every path and distinguish nothing. The signature is what authorizes a delivery, so a
  request without one cannot become a page. The ledger's distinct routes are the roster, because a
  producer names a route at call time and nothing else records it. A missing route reports loudly but
  does not move the exit code: the roster is derived from history, so a route retired on the gateway
  would fail the doctor forever with nothing an operator could do to clear it.
- [x] 34a. `posture-adapters` flake, PR #486, and only one of the two was a flake. The lifecycle case was
  a REAL DEFECT the suite happened to stand on: the command runner decided whether it had a terminal to
  hand over from the errno of `tcgetpgrp`, and Darwin answers a SOCKET at descriptor 0 with `EOPNOTSUPP`
  rather than `ENOTTY`, so an inherited socket failed every interactive command outright, which a
  socket-activated launchd job would hit. It now asks whether descriptor 0 is a terminal, which covers a
  pipe, `/dev/null` and a socket at once. The lock case was fixture shape: a single nonblocking `flock`
  asked once, in a binary where any concurrent spawn holds a copy of every open descriptor between its
  fork and its exec. Three more of the same class in pns followed, PRs #487 and #488.
- [x] 35. The banner click: `pns click`, and its three configured types
- [x] 35a. Raise the failure BANNER, which no task in this plan built: the design gives the notification
  its own 256-character form and its own `fix` line, and `pns_domain::failure::notification` renders it,
  but nothing called it, so a delivery failure was silent everywhere except `pns failures` and
  `pns doctor`. A leg speaks twice at most, on its first failure and on its dead-letter, because a banner
  per attempt teaches the operator to dismiss the one banner the design exists to raise. It carries the
  half of task 35 that had no producer: the banner's click is now `<absolute pns> click <id>`.
- [x] 35b. The failure PHONE CARD, under the existing presence rules (anything but at the desk, the same
  reading `forward_to_moshi` takes). It carries the rule the banner never needed, that a failure is never
  reported through the destination that failed: a mobile refusal pushes no card about itself, and a
  hermes refusal says Discord is empty rather than pointing at it.
- [x] 35c. A non-zero exit code for a synchronous caller whose page did not land, rung 5 of the design's
  "where a failure surfaces". Shipped OPT-IN, behind `--require-delivery`, because the design's
  unconditional form contradicts accepted decision 0010 (a notification never fails the work it reports
  on) and 126 tests pin that contract: every harness hook, the shell notifier and the daemon call the
  event path while real work is in flight. A caller that asked for the answer is one that can take it.
- [x] 36. The local page for moshi's browser preview
- [ ] 36a. Structured progress recaps, in chat and on `#pns`. Operator request 2026-09-14: every
  end-of-work summary uses the fixed Recap layout (Git block, stack graph, file list, Summary,
  In-Progress with a Blocked-on line, Upcoming Agent Tasks, User Tasks; the layout and the approved
  readability tweaks are in the agent memory `end-of-turn-recap-format`), and the same recap is posted to
  the `#pns` Discord channel through hermes. Triggers are agent-initiated, never a hook: a PR opened and
  waiting on a human review or auto-merged, each milestone while a `/goal` runs, after overnight work, an
  end-of-day summary, and on demand through `/pns:work-recap`. The command is a feature of pns, so it
  lives in the pns Claude Code plugin as
  `private_dot_claude/pns-marketplace/plugins/pns/skills/work-recap/SKILL.md`, beside `loop`, and is
  never a bare `/work-recap` (operator 2026-09-14; a bare `/recap` also collides with Claude Code's
  built-in). Three PRs, in order: (1) docs only, the layout in `.chezmoitemplates/global-agent-rules.md`
  plus the plugin skill, no agent review; (2) `pns recap agent --stdin`, which reads the markdown the
  agent wrote, fits it to Discord's 2000-character cap through the existing recap budget, sanitizes it
  and delivers it on the Discord route the overnight recap already uses
  (`pns/crates/pns/src/recap_delivery_runtime.rs`), Rust test-first, Opus, one review after the fix; (3)
  `pns recap git`, which generates the Git block, stack graph and file list from git, worktrunk and
  gh-axi. Brainstorm (2) before building. PR (1) DONE:
  [PR #566](https://github.com/webdavis/dotfiles/pull/566) (`docs/work-recap-command`) merged 2026-09-14
  (`2f3e6294`): the "Work recaps" section in `.chezmoitemplates/global-agent-rules.md` and the
  `work-recap` skill in the pns plugin. The 2026-09-13 20:15 apply converged the marketplace directory,
  but Claude Code runs the INSTALLED copy and `claude plugin update pns@pns` re-copies it only on a
  version change ("already at the latest version (0.1.0)", cache still `loop` only), so a follow-up,
  [PR #568](https://github.com/webdavis/dotfiles/pull/568) (`chore/pns-plugin-version-bump`, merged
  2026-09-14, `3edf50dc`), bumps `plugin.json` to 0.2.0 and records the rule in
  `docs/runbooks/claude-code-settings.md`. Operator steps left: `chezmoi apply`,
  `claude plugin update pns@pns`, restart Claude Code; then `/pns:work-recap` exists. (2) and (3) remain.
  Schedule (1) with the next Workflow round and (2) and (3) after the posture queue (#552, #549, #548,
  #553) clears, and before gnhf's first unattended night.

### STOP POINT C

A delivery failure is now loud, specific, and quick to act on, so posture can be trusted to page.

## pns command output

`pns doctor` prints twenty lines, twelve of them opening with the same `pns doctor:` prefix, in one
undifferentiated run. Every fact an operator needs is there and nothing says which lines belong together
or which one is the thing to act on. The operator's ruling on 2026-09-09: sections, color, a way to turn
color off, and the gum look rather than the plainer `hermes doctor` one, because output that reads well
is what makes a tool feel finished.

- [x] 69. The house style module and the doctor's report. `pns/crates/pns/src/style.rs` is the only place
  in pns that emits an escape sequence: gum's palette (the pink this repository already picked for
  `.chezmoitemplates/cli-print-style-lib.sh.tmpl`), a rounded frame, a section heading and a set of
  marks. `pns-domain::doctor::report` carries the report's SHAPE with no opinion about presentation, so
  the same report renders painted for an operator and plain down a pipe without either being a second
  copy. `--no-color` is the flag; `NO_COLOR`, `REPORT_LIB_PLAIN=1` and a destination that is not a
  terminal each turn it off on their own, and the flag beats all of them. Glyphs survive plain mode and
  only the color is dropped, because a mark is the row's meaning rather than its decoration. The doctor's
  twenty lines become titled sections, each with one line saying what its rows are for, and the report
  closes with a numbered list of what to act on. NOTE: the doctor currently refuses any argument at all,
  with a comment saying so; that comment changes with the flag.

- [x] 70. Every other pns command that prints more than a sentence adopts the same vocabulary. Its scope
  is decided by reading what each command prints today, not by a list written here in advance. Read on
  2026-09-09, the commands that qualify are `pns failures` (a column table, and `listing()` is served by
  the failure page as well, so it takes a `Paint` and the page passes the plain one), `pns home` (a
  multi-line diagnostic), `pns setup` (the wizard's walk) and `pns lights` (a list of lines). Everything
  else prints one sentence or a usage string.

  DONE 2026-09-09, all four. `pns failures` gained a header that discloses its own cap, because a listing
  that silently stops at twenty reads as "twenty things are failing"; its `listing` takes a `Paint`
  because the failure page serves the same table into a browser's `<pre>`, where an escape sequence is
  line noise. `pns home` became a verdict section and an evidence section, with unknown marked as a
  WARNING rather than a verdict (the router did not answer, so nothing was established either way) and no
  evidence heading when no keys are configured, since a heading over nothing reads as a section that
  failed to load. `pns lights quiet` gained a header naming its own scope, the thing most often got wrong
  about it. `pns setup` became a labelled opening plus six titled sections, so an operator part-way
  through can tell which feature the question in front of them arms.

  THE STYLE MODULE MOVED to `pns-adapters`, which is what the wizard needed: its questions go through the
  `Terminal` port, and with the vocabulary in the binary crate the walk could not reach it and would have
  grown a second look. Presentation on a terminal is a concrete destination, so adapters is its right
  home; the CLI already depends on that crate. The port gained `open` and `section`, which take SHAPES
  rather than styled strings, so `pns-application` still decides nothing about how a terminal looks.

  Two domain sentences lost their `pns ...:` prefix, each having one caller that now renders it under a
  heading supplying that context. The stderr warnings beside them KEEP theirs: those arrive alone, with
  no heading above.

## Framed headers carry labels, never floating sentences

- [x] 77. NOTHING IN A FRAME FLOATS (operator ruling 2026-09-09). Task 69 shipped the doctor's frame as a
  command name with a bare sentence under it, `pns doctor` over `every suppression gate is bypassed`, and
  a reader has no way to tell whether that sentence is a description, a status or an error. It reads like
  something went wrong. Every line after the command name takes a LABEL naming its role: `Note` for a
  caveat about the report being read, `About` for what a feature is, `Steps` heading a contents list. The
  label is what supplies the context the reader was otherwise left to guess at. THE FRAME ALSO SHOWS THE
  WHOLE INVOCATION, `pns tap --info` rather than `pns tap`, so the reader can tell which flag produced
  the output in front of them. This changes `pns doctor` (shipped) as well as the tap guide, and every
  command task 70 converts.

  DONE 2026-09-09, and the rule is now enforced by the argument type rather than asked for in a comment.
  `header` takes `&[HeaderLine]`, and a `HeaderLine` cannot be built without a label, so a floating
  sentence is unrepresentable rather than merely discouraged. Labels line up in one column, because
  ragged ones read as unrelated lines instead of as facts about the same report. The header also OPENS
  WITH A BLANK LINE (operator ruling 2026-09-09), so a report does not begin flush against the prompt
  just typed. The whole-invocation rule is carried by the parameter's name and its tests; `pns doctor`
  has no flags, so it is already whole, and the tap guide inherits the shape when it is built.

## The Back Tap marker

Verified on 2026-09-09, and it is the reason the two tasks below exist. The marker is written by a FORCED
COMMAND on an SSH key: `~/.ssh/authorized_keys` line 9 reads
`command="/usr/bin/touch /Users/stephen/.local/state/pns/phone-attention.marker",restrict` on the key
labelled `Shortcuts on mister`. sshd runs that command instead of whatever the client asks for, so the
iOS Shortcut never names the file and could not: a public key has no room for a path. pns has no writer
for it either, which its own spec records as an open question (`persistence-and-process-lifecycle.md`:
"no writer of that path exists anywhere in `src/`"). So the path is written down TWICE, in two systems,
one of which is not chezmoi-managed, and nothing checks that the two agree. A mismatch is silent: pns
sees a file that never updates, reads the tap as stale, and phone cards simply stop.

Mac implementation merged in [PR #537](https://github.com/webdavis/dotfiles/pull/537), with ten
fail-first behavior checks, seventeen focused checks, independent review, release build, full local
checks and required checks passing. Tasks 71 and 72's command, shared marker resolution and doctor row
are implemented. Task 71a's command failure, private directory creation, default configuration, typed
`pns.tap/1` output and manual setup/undo guidance are implemented. Operator deployment, Remote Login,
sleep/wake behavior and the actual phone artifact remain under 71a and 71b. The guide reports the missing
verified Shortcut URL; it does not supply an invented download or edit SSH trust.

- [x] 71. `pns tap` takes over the write. A new subcommand touches the marker at the SAME configured path
  the presence probe reads, so the path exists once. The forced command becomes
  `command="<cargo bin>/pns tap",restrict` and names no path at all, which is what makes task 72's knob
  safe to turn: the operator edits `authorized_keys` once, here, and never again. `restrict` still holds,
  so the key can run this and nothing else. The cost, stated rather than hidden: `/usr/bin/touch` is
  always present and a built binary is not, so a broken build takes the tap with it. That is why the
  doctor row below is part of this task rather than a follow-up: `pns doctor` gains a row under Pairing
  reporting the marker's freshness, so a broken tap chain is visible without inspecting SSH trust. Its
  flags, and what each one is for. Bare `pns tap` touches the marker and prints the surface that results,
  because the forced command's stdout travels back over SSH and the Shortcut can show it: a tap that says
  nothing is a tap you cannot tell from a broken one. `--info` explains the feature and reports its live
  configuration: the marker path AND WHICH SOURCE SUPPLIED IT (the shipped default, the config file, or
  the environment), whether the file exists, how old it is, and the surface that age implies. Naming the
  source is the point of it: an operator who set the config value and still sees the default is looking
  at an override they forgot, and no other output on the machine would tell them. `--install` PRINTS the
  `authorized_keys` line for this machine, with the binary path resolved, and says where to paste it. It
  says that a line already wired for this should be replaced rather than added beside. THERE IS NO
  `--write`, AND PNS NEVER READS OR WRITES `~/.ssh/authorized_keys`. This task's own history went back
  and forth on it, so the reasoning is recorded rather than the conclusion alone. Against writing: the
  flag would gate INTENT, never CAPABILITY. The write code sits in the binary on every run, and that
  binary runs unattended as a daemon, from every harness hook, and on every shell prompt. Any bug, config
  injection or compromised dependency that reaches it escalates to granting SSH access to the machine,
  which is not a notification tool's blast radius. What it buys against that is one paste, once per
  machine, ever. pns is also a tool other people `cargo install`, and "this notifier can edit your
  authorized_keys" is a line that should stop an auditor cold. Against reading: `--info` PRINTS what it
  reads, into a terminal whose contents get pasted into chats and issues, and the file is the operator's
  whole SSH trust list. And therefore no `--backup`: it only ever existed to make the write safe, and it
  carried its own hazard, since a copy of a trust file re-grants a key that was later revoked if it is
  restored unread. WHAT REPLACES THE READ IS A BETTER CHECK. `--info` and the doctor row report the
  MARKER'S OWN FRESHNESS: "last tap 3 hours ago", or "never tapped". Freshness establishes that the
  marker changed and needs no access to `~/.ssh`. A local command can also update it; task 71b's real
  phone test verifies the complete tap chain. `--install` IS A GUIDE, not a dump. It uses task 69's house
  style, so setup and `pns doctor` read as one tool: the framed title, `◆` numbered step headings with a
  faint blurb on the rule, `·` rows for the parts of a line that need explaining, and a closing rule
  pointing at `pns tap --info` to check the work. THE FRAME CARRIES A NUMBERED CONTENTS LIST, not a
  sentence and not a count of parts (operator ruling 2026-09-09, after "two halves" and then "set up this
  Mac, then set up your phone" were both rejected as too vague). It lists the steps by the same numbers
  their headings use, each with a short gloss: `1. This Mac / the authorized_keys line`,
  `2. Your phone / the PNS Tap shortcut`, `3. Trigger methods / Back Tap, Action Button, others`, NOT "A
  trigger": the section lists ways to fire the Shortcut, so it names the category rather than one
  instance of it. The reader sees the whole job before starting one, finds their place again after
  stepping away, and learns what a step involves without scrolling to it. Three steps, in the order they
  are performed: step 1 the `authorized_keys` line, with `command=`, `restrict` and the key placeholder
  each explained on their own row; step 2 the Shortcut, as labelled fields (Host, User, Auth, Script)
  rather than prose, with a note that the script text is cosmetic since step 1 overrides it; step 3 the
  triggers, listed with the Settings path beside each. Host and user come from the machine, never
  hardcoded. EVERY WORD PNS PRINTS GOES THROUGH THE `humanizer` SKILL BEFORE IT SHIPS (operator ruling
  2026-09-09, standing, and it covers every pns command rather than this guide alone). Terminal output is
  prose the operator reads under pressure, and the tells that skill catches are the ones that make a tool
  feel generated. The first pass over this guide caught four. A subjectless "Nothing is written for you"
  tacked on as a negation becomes "pns does not edit this file". A run of fragments closing on the
  manufactured punchline "This key does one thing" keeps the fact list and loses the punchline. "The
  script text is cosmetic" becomes "sshd ignores this script text", which is shorter and more accurate.
  And "Found 3 issues to address:" carries filler ahead of a numbered list, so the doctor's closing line
  becomes "3 issues to fix:". The doctor's seven section blurbs passed unchanged. The operator's
  2026-09-09 record in `pns/docs/pns-tap-apple-shortcut.md` says the Shortcut is already public. Verify
  its install link and use it in step 2, retaining the required machine-specific setup fields. COVERS THE
  PHONE SIDE TOO, because the wiring has two halves and an operator holding only one of them has nothing
  working. After the `authorized_keys` line it prints the Shortcut recipe (Run Script Over SSH, with the
  host, the user and which key to select) and the triggers that Shortcut can be attached to: Back Tap,
  the Action Button, a Lock Screen widget, Control Center, Siri. The command text typed into the Shortcut
  is cosmetic, since sshd runs the forced command instead, but it is spelled `pns tap` anyway so the
  Shortcut reads as what it does. THE SETUP PROSE STAYS OFF `--info`: that flag is read when something is
  already wrong, and burying a status report under a wall of instructions is how a diagnostic stops being
  read. `--info` closes with one line pointing at `pns tap --install`. THE PRINTED INSTRUCTIONS CARRY THE
  iOS VERSION THEY WERE VERIFIED AGAINST, as a line the reader sees ("Settings paths verified on iOS
  <version>"). The exact paths to Back Tap and the Action Button move between releases, and instructions
  that do not date themselves are worse than none: a reader on a later iOS cannot tell a path that moved
  from a step they got wrong. Verify them against the operator's own iOS at build time rather than
  writing them from memory here, and record the version in the same change that writes the text.
  `--delete-marker` IS NOT BUILT. Verified against `pns/docs/specs/presence-and-visibility.md` on
  2026-09-09, which settles it: "Mobile and Away both mean the phone card", and "Away always cards while
  Mobile lets [the viewed pane suppress it]". So deleting the marker while away from the desk moves the
  operator Mobile to AWAY, which cards MORE aggressively because Away never suppresses, the opposite of
  what a flag called clear or delete would promise. At the desk it is redundant, since typing already
  cancels a stray tap under newest-signal-wins. Both cases fail, so the flag does not ship. The earlier
  names weighed for it (`--clear`, `--at-desk`) are moot. `--json` emits the same answers
  machine-readably, so the Shortcut renders them rather than dumping a sentence. `--no-color` is NOT one
  of these flags; it is tool-wide, task 73. DELIBERATELY NOT `--set-marker`, a flag that writes the
  config: `~/.config/pns/config.toml` is a chezmoi-rendered target on this machine, so a write there is
  erased by the next apply and the operator would watch their change disappear. `--info` names the file
  that really holds the value instead. DELIBERATELY NOT `--for <duration>`, a tap that expires on its
  own: the probe reads the marker's mtime and never its contents (`symlink_metadata`, so a dangling
  symlink still answers), so an expiry is a reader redesign rather than a flag, and it is scoped
  separately if it is ever wanted.

- [ ] 74. THE HTTP TAP, an opt-in ALTERNATIVE to the SSH one, never a replacement that arrives on its
  own. Operator ruling 2026-09-09: ship the SSH shape first, offer this as an upgrade the operator
  chooses. pns serves a small endpoint the Shortcut posts to ("Get Contents of URL" rather than "Run
  Script Over SSH") and records the tap itself. THE POINT IS THAT PNS OWNS BOTH ENDS: no
  `authorized_keys` line, no forced command, no second system holding a copy of a path, so the decoupling
  tasks 71 and 72 work around stops existing rather than being managed. The machinery is mostly here
  already: the daemon runs, and `pns failures serve` is a listener. IT ASSUMES NOTHING ABOUT THE
  OPERATOR'S NETWORK (operator ruling 2026-09-09, correcting an earlier draft of this task that said "on
  the tailnet"). pns is a tool other people install and it has no idea what anyone's topology looks like:
  no Tailscale, no VPN, no LAN shape, nothing detected and nothing guessed. The listener is OFF unless
  configured, and its `bind` address is written by the operator with NO DEFAULT, because there is no safe
  one to pick: loopback is safe and unreachable from a phone, and every other address is a guess about
  somebody's network. Authentication is a config secret, the way the hermes webhook already is. ITS ONE
  REAL COST, which is why it is opt-in rather than the default: the SSH tap works with pns's daemon dead,
  because sshd and the command it forces carry it end to end, and an HTTP tap does not. An operator whose
  daemon is wedged still wants their phone to say so. Also a listening port where there was none, and a
  secret that needs a rotation story.

- [ ] 76. Apple Shortcuts research completed on 2026-09-13; device acceptance remains open. The proposed
  iCloud route is a no-go: Apple's
  [synchronization guide](https://support.apple.com/guide/shortcuts-mac/apdb3a4240b0/mac) documents
  shared shortcut definitions, not dispatching execution to a selected Mac and returning its result. No
  supported remote-dispatch interface was found. Retain the settled SSH (Secure Shell) route to
  `pns tap`. [Mac command-line Shortcuts](https://support.apple.com/guide/shortcuts-mac/apd455c82f02/mac)
  can run through SSH but add a dependency without replacing that transport. This finding alone does not
  authorize task 74. With task 71b, measure trigger-to-confirmation and trigger-to-failure on the actual
  phone and Mac, including locked, sleeping, unavailable and remote-network cases. Network wake is
  conditional; do not promise that a request wakes the Mac. The acceptable latency and failure-feedback
  deadline still require operator acceptance. No device state was changed in this investigation.

## Tool-wide output flags

- [x] 73. DONE 2026-09-09, shipped with task 69 rather than after it, because a flag whose scope is wrong
  is a contract, and the narrow form would have been the shipped one for as long as it took to widen.
  `--no-color` is a PNS-WIDE flag, accepted in every position: `pns --no-color doctor` and
  `pns doctor --no-color` mean the same thing, because an operator who has decided about color has
  decided about the whole command rather than about one subcommand's report. Task 69 shipped it as a
  `pns doctor`-only argument, which is the narrower reading and wrong; this widens it. The dispatcher
  takes the flag out of argv wherever it appears, remembers it once, and every command that prints reads
  that one answer with no plumbing of its own. THE EVENT PATH IS EXEMPT and keeps its argv untouched: it
  prints nothing but an exit code, and a position-blind filter would eat a producer's
  `--detail "--no-color"` as a flag, which is the same value-position bug the argv grammar already guards
  against elsewhere.

- [ ] 71a. FIVE THINGS THE TAP DESIGN LEFT OUT, found by re-reading it whole on 2026-09-09. Each is a
  silent failure, which is why they are recorded rather than left to be noticed later. EXIT CODE:
  `pns tap` exits non-zero when the touch fails, so the Shortcut can show a failure. A tap that fails
  silently is worse than no tap, because the operator stops checking. THE STATE DIRECTORY:
  `~/.local/state/pns/` may not exist on a fresh machine and `pns tap` may be the first thing to reach
  for it, so it creates the directory rather than failing on it. REMOTE LOGIN is a prerequisite in the
  Mac setup step. The whole feature needs sshd accepting connections (System Settings, General, Sharing,
  Remote Login). Without it every other step is wired correctly and nothing happens, which is the worst
  kind of wrong. Verify sleep and wake behavior on the operator's devices and explain what the Shortcut
  reports when the Mac cannot answer. `--info` should identify this troubleshooting path. NO CONFIG
  REQUIRED: `pns tap` must work with no `~/.config/pns/config.toml` at all, falling back to the default
  marker path, because requiring one would fail on exactly the fresh machine `--install` is walking
  somebody through. Define the `--json` schema and manual undo instructions before building. DONE
  2026-09-14 in [PR #569](https://github.com/webdavis/dotfiles/pull/569) (`feat/pns-tap-fresh-machine`,
  merged `f85a6cfd`), each claim verified against the code first: the non-zero exit, the single stderr
  line and the OS error were already true, the line now names the marker path and prints the whole error
  (errno included); the 0700 state directory was already created recursively (pinned by existing tests);
  Remote Login already led the Mac steps but named no settings path, now it does, and `--info` says what
  the phone shows when the Mac cannot answer (the SSH action fails, so the phone shows the SSH error,
  never the success notification); a missing config already fell back to the default marker path (a parse
  error still fails); the shipped `pns.tap/1` JSON object was kept as is (its `marker` is a table, not a
  bare path) and gained `touched_at` in RFC 3339 UTC, with the whole field table, the null cases under
  `--install` and on early failures, and the undo (delete the marker; the 0700 state directories stay)
  written into `pns/docs/pns-tap-apple-shortcut.md`. Left open, operator-device work: verify sleep and
  wake behavior of the Mac against the Shortcut.

- [x] 72. `[phone] marker_file` makes the path configurable, defaulting to today's
  `$HOME/.local/state/pns/phone-attention.marker`, with `PNS_PHONE_MARKER_FILE` still winning over it so
  the tests and sandboxes are untouched. NOT `[presence]`: `[plugins.presence]` already exists and is the
  Hue room sensor, and two tables a word apart meaning different things is the confusion this avoids.
  Ordered after 71 deliberately: a knob shipped while the path is still duplicated is a knob that breaks
  the tap when it is turned.

- [ ] 71b. Finish the phone-side artifact after the Mac command exists. The shipped Shortcut currently
  points at the missing `pns tap --install`. Correct its three-global-variables/four-fields instructions
  and feed success or failure from the command into an accurate confirmation of the resulting surface.
  Update the actual Shortcut first, then its verbatim record in `pns/docs/pns-tap-apple-shortcut.md`.
  Verify setup and a real tap on the operator's devices, including an unavailable Mac and a write
  failure.

## SSH exposure (not a pns task)

- [ ] 75. Restrict this Mac's SSH exposure to the tailnet using a supported mechanism. This belongs to
  dotfiles; pns remains network-independent. The earlier `ListenAddress` proposal did not account for
  launchd owning Remote Login's listening socket, documented in `executable_ssh-hardening.sh` and the
  installed `/System/Library/LaunchDaemons/ssh.plist`. Investigate that ownership and available controls
  before choosing the change. Preserve recovery access, review the exact activation and rollback with the
  operator, then verify allowed and disallowed reachability over IPv4 and IPv6, the listener state, a
  real SSH login and a real phone tap. Do not infer network isolation from `sshd -T` alone.

## posture foundation

- [x] 37. posture 2.4: page, domain digest, protocol codec

- [x] 38. posture 2.9: `drift.rs` and `converge_policy.rs`. ALREADY DONE when this was checked on
  2026-09-09, shipped by the converge-foundation work in PR #470 rather than by a task of its own. Both
  modules are implemented, exported and tested (20 drift cases, 9 converge-policy cases), and neither
  carries a deferral note. Verified by running them rather than by reading the plan.

- [x] 39. posture 2.10 domain work: cursor and triage policy are implemented. Task 40 supplies the
  recorded/on-disk hash and upgrade facts; task 45b retains caller arming and acceptance.

- [x] 40. Complete posture 3.1 adapter behavior. `PollMarkers` and the initial adapters landed, but
  enumerating trait implementations did not establish acceptance. The triage/upgrade-record producer used
  by 45b now matches the actual producer's example from the port plan.
  [PR #538](https://github.com/webdavis/dotfiles/pull/538) merged actual alert composition, display-only
  recorded/on-disk hashes and a bounded upgrade-record reader. The missing-detail regression failed
  before implementation and passed afterward; five pure correlation regressions also failed before their
  implementation. The producer now matches a captured Bash example containing quotes and an empty
  added-version field. Package tests, checks, Clippy and documentation passed. Independent review found
  and verified fixes for eager triage on ignored events and parallel scratch-directory collisions. Only a
  domain-approved integrity page now requests display facts; missing facts still preserve the page. The
  full local checks and required checks passed. Deployment and task 45b's arming/acceptance remain
  separate.

- [x] 41. Complete posture 3.2 health-adapter review and publication with task 46. Commit `03b66c0b`
  supplies the `launchctl print` and gateway health readers, plus independent pns integrity, daemon and
  ledger checks. Published and reviewed with task 46: it is on `main` through
  [PR #547](https://github.com/webdavis/dotfiles/pull/547) (verified 2026-09-14 with
  `git merge-base --is-ancestor`). Gateway health stays separate from notification delivery. The
  operator's health/recovery acceptance after deployment is tracked under 46.

- [x] 42. posture 3.3: the converge read half, staging, privileged. Already done, and verified the same
  way: `ConvergeStaging` in `staging.rs`, `DesiredTree` in `staging/owned.rs`, `LiveTree` in
  `live_tree.rs`, `PrivilegedInstall` in `converge/install.rs`, plus `OsqueryControl`, `ProcessTable` and
  `RestartClock` under `converge/`.

### STOP POINT D

Finish the foundation and adapter acceptance before starting the dependent cutovers. Task 40 is merged;
task 41 still needs review and publication. Heartbeat and digest have already cut over independently.

## posture cutovers

The pns-keyed gateway route prerequisite was verified on 2026-09-09, as recorded below. Do not ask the
operator to create it again. The remaining adapter, delivery and live cutover checks still apply.

- [x] 43. posture 6.1: heartbeat cutover. The plist now runs `posture heartbeat` instead of
  `bash heartbeat.sh`; the bash script and the integration test that pinned it are deleted.
  `canary-freshness.sh` STAYS, because the watchdog still sources it and its own cutover is task 46. WHAT
  THE OPERATOR STILL DOES, per the plan's step 6: apply, run `posture heartbeat` by hand once, watch for
  the silent Discord line on the pns-keyed route and the silent desk banner, confirm the pns ledger
  recorded it, and only then trash the deployed `~/.local/libexec/osquery/heartbeat.sh`. Deleting a
  chezmoi source never deletes its target, which is why the deployed copy outlives this change.
- [x] 44. posture 6.2: digest cutover. The plist now runs `posture digest` instead of `bash digest.sh`;
  the bash script and the integration test that pinned it are deleted. The port splits one `main` into
  three seams that test apart: the application use case owning the claim, keep and restore decisions, the
  adapter owning the file moves, and the composition root. Two behaviors the shell could not express are
  now pinned: a batch whose every line is unreadable is KEPT for forensics rather than sent with a count
  and an empty body or retried forever against bytes that render empty again, and a clock that cannot
  answer leaves the batch untouched rather than claiming one this run could not finish naming. The digest
  spool's WRITE side stays bash until task 45b; both ends still agree because they are built from one
  `posture-protocol` record. THE ALLOWLIST TUPLE MOVED WITH THE PLIST: the alerter matches a
  `persistence_launchd` finding against (label, path, program), so repointing without it pages on the
  next launchd scan. Task 50a tracks the filled-spool delivery and `.last` rotation acceptance still
  needing evidence. The deployed `~/.local/libexec/osquery/digest.sh` is already absent on 2026-09-12.
- [x] 45a. posture 6.3, first half: the alerter's read-to-checkpoint transaction. SPLIT FROM TASK 45 on
  2026-09-09 because the port plan calls 6.3 "the largest cutover" and a single pull request for it would
  be the huge diff the small-PR rule exists to prevent. This half is policy and ordering only, with no
  adapter and no cutover, so the pipeline it replaces keeps running untouched while it lands.
  `posture-domain/src/records.rs` splits a snapshot at its last newline, because osquery writes a row
  before its newline and the trailing bytes are not a record yet. Retaining the torn line is the obvious
  half; the expensive half is that COMPLETE JSON WITHOUT ITS NEWLINE IS ALSO TORN, since processing it
  now and again once the newline lands pages one finding twice over two overlapping byte ranges. The
  count is in BYTES, not characters, because the cursor is a byte offset and osquery rows carry paths.
  `posture-application/src/judge_results.rs` owns the transaction: take the lock or no-op, read the log
  once, ask the cursor where to start, replay and page loudly on a lost cursor, judge only complete
  records, deliver, and checkpoint LAST. The judge itself is a port (`JudgeFindings`), because judging a
  row reaches the allowlist file, the known-good manifest, the deployed state, the enricher's spawned
  inspections and the digest spool, and keeping all of that behind one boundary is what lets the ordering
  be tested against doubles that touch nothing. A digest row is delivered the moment the judge spools it,
  so only a page has a delivery this run can fail. 21 tests green, clippy clean.
- [ ] 45b. posture 6.3, second half. The main transaction shipped in PR #506 (81 tests), and triage facts
  merged in #538. Arming and live acceptance remain. Shipped: the results-log reader with its single
  reading and bounded span, the cursor published by rename, the non-blocking single-instance lock
  (`O_CLOEXEC` replacing the shell's by-hand `9>&-` on every spawn), the row decoder, the column
  projection, the allowlist reader, the known-good manifest reader, the digest spool's append side, the
  `JudgeFindings` implementer, and `posture alert`. The enricher runs IN PROCESS rather than through a
  spawn, because `posture enrich` was already a use case in the same crate. WHY THE ENRICHER WAS NEVER
  OPTIONAL, recorded because it was twice reasoned about wrongly on 2026-09-09 before being measured: an
  untrusted signing verdict PROMOTES a Notice finding to Critical in the gate, so a cutover without it
  would send a finding the shell paged about to the next day's digest. That is a missed page, not extra
  noise. Both directions are now pinned by tests. The triage producer supplies recorded and on-disk
  hashes and upgrade correlation. These are display facts, and the shell tolerated missing facts whenever
  its optional helper was undeployed, so a page fires carrying less rather than not firing. FOUR
  DERIVATIONS WERE WRONG until the binary was run against a real sandbox, and the unit tests agreed with
  all four because they came from the same misreading of the shell's jq: the action was taken from a
  column rather than from the row, the identity column order dropped `identifier`, a listening port lost
  its address and port, and the timestamp carried the date without the time. Real-run verification is
  what caught them. The producer is now verified; arm the command by repointing the plist to
  `posture alert`, move the allowlist tuple for `com.webdavis.osquery-results-alerter` with it (the
  alerter matches a `persistence_launchd` finding against label, path AND program, so repointing without
  it pages on the next launchd scan), and delete `executable_results-alerter.sh` plus six private files
  under `results-alerter/`, keeping `pipeline-verdict.sh` deployed because bash `pipeline-audit.sh` still
  sources it and would otherwise refuse BOTH manifest scans as unavailable (it retires in task 46), and
  retire the old tests by their current consumers. The canonical plan names six suites; reconcile that
  inventory against current source before deletion. Run the sandbox composition checks and the plan's
  live page/digest, checkpoint and retry acceptance after the operator applies.
- [ ] 46. posture 6.4: finish watchdog publication and cutover. Source on `feat/posture-watchdog-health`
  composes state publication, delivery ordering, legacy growth history, independent binary integrity,
  daemon and ledger checks. Independent review passed 944 posture tests and six additional regressions.
  The direct alarm precedes pns submission, and failed alarms retain unresolved state even when pns
  reports acceptance. The authorized pns build record and manifest publication passed 30 private checks
  with 99 assertions. Poll and watchdog are integrated at `a57d9346`; the final repeated `just ship`
  passed with four Rust workers and unchanged deadlines, and the exact release build produced 3,692,720
  bytes. A separate fail-first installer regression verifies that posture records the compiler selected
  by the build directory; all 15 installer tests and 70 assertions pass. On 2026-09-13 `just ship` on
  `a57d9346` passed again (exit 0, 3m16s) and [PR #547](https://github.com/webdavis/dotfiles/pull/547)
  was opened against `main`; it is reviewed once, unmerged, and merges on green continuous integration.
  Independent review returned six findings, all fixed and pushed. SEV-0: pns retained dead-lettered legs
  forever (no delete in the `retain_deadletters` migration), so a `deadletters > 0` check paged every
  tick forever; fixed at `12373fd5` to page only on growth. SEV-1: the 8 MiB binary cap in the pns
  builder, the manifest script and `watchdog_audit.rs` would have refused every apply after one
  dependency bump, since pns is already 6,966,304 bytes; fixed at `a448e635` with per-tool artifact
  ceilings in `.chezmoidata/rust_tools.yaml` (pns 14,680,064 bytes, posture 2,097,152 bytes) across the
  four sites that needed one, the posture builder being the fourth. The remaining findings are fixed at
  `ccb44a64`, `c2d84559`, `cf180556` and `3fdc83cd`, the last replacing clock windows with control arms;
  the `gateway_health` `WouldBlock` flake it fixes reproduced 2/20 at load 20 and 0/50 after. Continuous
  integration is pending on the pushed fixes. Follow `posture/docs/acceptance/watchdog.md` before
  retiring `pipeline-audit.sh` and its remaining `pipeline-verdict.sh` dependency. Deployment and real
  alarm acceptance remain open. On 2026-09-14 `main` was merged into the branch with no conflicts and
  [PR #547](https://github.com/webdavis/dotfiles/pull/547) merged at `1f934c7b`.
- [ ] 47. posture 6.5: finish poll composition and cut over its plist. The application transaction and
  command merged in [PR #544](https://github.com/webdavis/dotfiles/pull/544), and local main contains it.
  Independent review passed 909 workspace tests and 15 private Bash/native command comparisons, including
  exact alerts, baseline bytes, markers and submission order. The full repository gate and required
  continuous integration passed, with four local Rust test workers and unchanged deadlines. Preserve the
  existing baseline and verify exposure and recovery across two live ticks before removing the Bash
  producer. Security-page sound parity is supplied by merged
  [PR #540](https://github.com/webdavis/dotfiles/pull/540), with independent review, full checks and
  required continuous integration passed. Operator deployment and audible acceptance remain separate.
- [ ] 48. posture 6.6: publish the implemented funnel command on `feat/posture-funnel`, then cut over.
  Independent review approved the bounded security omission notice and finite timeout parser fixes. The
  notice never acknowledges the original oversized finding. All 45 command fixtures, 24 producer checks
  and 144 additional private submission cases passed, along with the integrated repository gate. On
  2026-09-13 the branch (`adb23b54`, which contains the watchdog branch and current `main`) passed
  `just ship` (exit 0, 3m44s) and [PR #551](https://github.com/webdavis/dotfiles/pull/551) was opened
  with base `feat/posture-watchdog-health`, so it shows only the funnel commits and retargets to `main`
  when #547 merges; it is unreviewed and unmerged. Independent review returned five findings; the fix is
  on the branch and its own fix review is in progress. SEV-1: exposure pages with 37 or more keys
  exceeded the 8,000-character wire cap and were refused forever, bounded at `FUNNEL_EXPOSURE_KEY_LIMIT`
  (32 keys plus a summary line, commit `0037bc33`). SEV-3: stderr named retired tools, fixed at
  `b1a8b777`. SEV-3: the inline executable check was replaced by the shared `is_executable`, fixed at
  `4ea6e8b9`. Two findings are deferred to a follow-up: SEV-3, the duration parser maps `0`, `inf` and
  `1e100` to `Status(125)` and pages a false gap; SEV-3, the 44-line unsafe FFI hex-float parser could be
  `trim` plus `parse::<f64>`. Two more fix commits, `4cb11f21` and `c50f95d6`, are not yet pushed: a
  sorted-before-cut test, fixture cleanup, a root skip, and doc numbers now measured by test at 7,160;
  the timeout parser's `0`/`inf`/oversize inputs now saturate to a 24-hour ceiling; `strtod` is kept
  because the capture `timeout_hex` passes `0x1p-1`. `just ship` on `c50f95d6` failed only on the
  `gateway_health` flake that #547 fixes; re-ship once #547 merges into it. Preserve the baseline and
  verify real-input behavior before retiring Bash. On 2026-09-14, after merging main in, fix commits
  `0037bc33`, `b1a8b777`, `4ea6e8b9`, `4cb11f21` and `c50f95d6` were pushed, the PR body was re-posted,
  continuous integration passed and [PR #551](https://github.com/webdavis/dotfiles/pull/551) merged at
  `0efb2119`.
- [ ] 49. posture 6.7: retire the drainer only after every producer has migrated, all three queue tables
  are empty and the operator has reviewed dead-letter disposition. Remove its loaded job, monitored
  label, legacy queue reader and growth state together. The drainer is still loaded at audit time.
  Preserve an export of reviewed dead letters, obtain fresh approval for exact-row removal and reread all
  three counts. Unresolved rows retain the route, key, drainer and queue. Once empty and no other
  consumer needs them, retire the old `priority` route/key and propose cleanup of the queue's three
  files, `osquery-spool/`, `osquery-tailscale-funnel` and `~/.config/osquery/webhook-secret` as required
  by the port plan. Secret values stay out of logs and review artifacts.
- [x] 50. posture 7.1: publish converge integration from `fix/posture-converge-validation`.
  Private-database validation now precedes daemon probing or repair; the apply caller uses slot 59 after
  its build, and uu supplies the configuration argument. Independent review, ten executable-discovery
  cases, focused validation regressions and the integrated repository gate passed. On 2026-09-13 the
  funnel branch was merged in (`eef8ea6f`), `just ship` passed (exit 0, 4m27s) and
  [PR #552](https://github.com/webdavis/dotfiles/pull/552) was opened with base `feat/posture-funnel`, so
  it shows only the converge commits; it is unreviewed and unmerged. Independent review returned six
  findings and the fix is pending. SEV-1: the private config check runs
  `sudo -n osqueryi --database_path <private>/db`, osquery creates that directory root-owned 0700, the
  unprivileged cleanup fails and turns a passing check into Unavailable, so every real repair reports
  failure and aborts the apply at slot 59. SEV-2: the test double models the wrong privilege boundary.
  Four further SEV-3 findings. Fixed 2026-09-14, root cause reproduced against the real `osqueryi` (it
  creates the `db` directory as the invoking identity, root under `sudo -n`, mode 0700): the caller now
  creates `<private>/db` itself before the privileged call, so root only writes flat files into a
  directory the caller owns, and removal is the private directory's Drop alone, never part of the verdict
  (`672d83f9`, two tests, both red before the fix); the test double now stands in for root (mode 0o000
  directory, the same EACCES) rather than for a same-user cleanup; `resolve_osqueryd` renamed to
  `resolve_osqueryi` (`5346d815`); the osqueryctl fallback no longer creates the private directory first
  (`be4ea091`); the double cleanup and the placeholder test names are gone (`8f10d720`). The Fable-tier
  review after the fix returned three more findings, fixed at `2dc16ce`, `1614dd39` and `785ebdba`;
  `just ship` passed (exit 0) and the PR body was re-posted. GitHub started no CI run for the pushed head
  `a726101e`; main (with #548, #549, #566, #567) was merged in by hand (`73d70f7e`, three additive
  conflicts in the export blocks and `Cargo.toml`, resolved as unions, 1,056 posture tests green) and
  pushed; CI still did not start because the PR's base was the merged `feat/posture-funnel` branch and
  the lint workflow runs only against main, so the base was retargeted and the PR closed and reopened, CI
  passed, and the PR merged 2026-09-14 (`6fa992b0`); the worktree is removed. Left for the live drill,
  tracked as acceptance rather than code: same-user fixtures do not prove privileged cleanup, so verify
  silent no-drift behavior and the operator's approved permission-repair/restart drill before retiring
  Bash.
- [ ] 50a. Close outstanding acceptance from already-merged heartbeat and digest cutovers, tasks 43 and
  44\. Installed plists invoke Rust, but that does not prove delivery. Record the silent pns-route
  message, banner and ledger evidence, and a filled-spool digest with `.last` rotation. Inventory retired
  helpers and their remaining consumers before proposing removal: deployed `heartbeat.sh`, `allowlist.sh`
  and `enrich-finding.sh` remain; `digest.sh` is already absent. See port-plan steps 6.1 to 7.1 for each
  cutover's full acceptance and rollback requirements. Also reconcile earlier enrichment and allowlist
  acceptance from steps 4.1 and 4.2: signed/unsigned enrichment exits and facts, deployed-list parity,
  and own-agent tuple refresh/publication. Record previously evidenced checks as complete. Update stale
  install paths and by-name apply instructions to the current operator-run full-apply rule when recording
  acceptance.

### STOP POINT E

Every posture producer is Rust and the old pipeline is off.

## uu

- [x] 51. uu B1: `rust-toolchain.toml`, needs the stable toolchain certified.

  DONE 2026-09-09. Stable on this machine is 1.98.1, which carries the `file_lock` `pns-adapters` depends
  on, so `channel = "stable"` is a pin the tree can actually hold. Certified by running the whole
  `just test-rust` under `RUSTUP_TOOLCHAIN=stable` BEFORE the pins were written, rather than writing them
  and hoping.

  EIGHT PINS, not the plan's five. The plan predates `posture`, `lights` and `tailnet-pin`, so the crate
  roots are seven (`pns`, `uu`, `posture`, `lights`, `tailnet-pin` and the two herdr plugins) plus one at
  the repository root. The root pin is what makes `just test-rust` run on stable: it invokes cargo from
  the root with `--manifest-path`, and rustup resolves a toolchain by walking up from the CURRENT
  directory, never from the manifest's.

  THAT SAME RULE IS WHY THREE BUILDERS CHANGED. `run_onchange_after_58-build-pns-engine`, `59-build-uu`
  and `58-build-posture` each built with `--manifest-path`, and a chezmoiscript's working directory is
  `$HOME`, where no toolchain file lives, so the crate pin would have been read by nothing. Each now runs
  `(cd "$crate_dir" && cargo build ...)`. The lights, tailnet-pin and herdr builders already did this and
  were left alone. Each pin joins its builder's hashed inputs, because moving the channel changes the
  binary.

  `.chezmoiignore` excludes `rust-toolchain.toml` by bare name, which chezmoi matches at the TARGET ROOT
  alone. Verified with `chezmoi managed`: the two herdr plugin pins deploy, the root pin and the five
  workspace pins do not (those five sit inside directories the file already ignores by name).

- [x] 52. uu D1, D2, D3: the cargo lane and `RustupLane`.

  DONE 2026-09-09. Crates and toolchains were the last tool families nothing checked, so a crate
  installed a year ago sat at that version until somebody noticed. Running the finished cargo lane on
  this machine found three: `fd` 8.4.0 against 10.5.0, `nu` 0.44.0 against 0.115.1, and `selene` 0.26.1
  against 0.31.0.

  THE CARGO LANE REPORTS AND DOES NOT COMPILE by default. A `cargo install` builds from source, minutes
  per crate on an unattended weekly run, so a crate that is behind comes back PENDING with the exact
  command to paste. `compile = true` turns building on, and one crate that will not build does not stop
  the next.

  TWO PARSING RULES ARE LOAD-BEARING, both found by reading real output rather than guessing at a format.
  A search is matched BY CRATE NAME, never by position: `cargo search ripgrep` answers with `gist-search`
  and `cgx-core` below it, whose descriptions merely mention ripgrep. And a git-installed crate is named
  and skipped rather than searched, because crates.io holds no version of it; an origin the parser cannot
  read drops the whole header rather than passing as a registry crate, which would search for a version
  that install never had.

  THE RUSTUP LANE READS THE SUMMARY, not just the exit code. rustup exits 0 whether it moved a toolchain
  or found nothing to do, so a record saying only "ok" could not answer the question the lane exists for.
  A summary it cannot read is recorded as exactly that rather than as everything being current.

  `schema::boolean` replaced the one hand-rolled copy of the same check in the nvim lane. The roster
  guard that walks every declared key probes with `true`, which a boolean key legitimately accepts, so
  `compile` joins `auto_commit` in the short list that gets an integer probe instead.

  Both lanes were proved against the real tools, not only against doubles. The rustup lane read this
  machine's two toolchains as current, and its failure path was exercised by accident and reported
  rustup's own stderr verbatim.

- [x] 53. uu E12, E13, E14: skills hermes and forks

- [x] 54. uu E15a, E15: the skills orchestrator

- [x] 55. uu E16: retire `update-skills.sh` and its LaunchAgent

- [x] 56. uu E17: retire `log-entries.sh`

- [x] 57. uu E19: the log rotation lane, needs the hourly log writer stopped.

  DONE 2026-09-09. The lane's code shipped in E18 but was never REGISTERED, so `uu` did not list
  `rotate-logs` among its lane types and the config block sat commented out with a note saying it was
  waiting on the cutover. One line in `uu/crates/uu/src/registrations.rs` is what turned it on.

  The bash side is deleted: `executable_compress-and-truncate-local-logs.sh`, its plist, its loader at
  `run_onchange_after_67`, its `.chezmoiignore` Linux-block line, and four unit tests. `CLAUDE.md` lost
  its LaunchAgent row and stopped using the script as its flat-leaf and verb-first examples, which now
  name `control-hue-lights.sh` and `brew-shellenv-cache-refresh.sh`.

  THE LIST GAINED A FOURTEENTH LOG, `~/.local/log/scalebar/scalebar.log`, because a managed LaunchAgent
  whose log is missing from this list grows without bound and nothing says so. A path that does not exist
  yet is `Skipped`, not a failure, so naming it before the Scalebar work applies costs nothing.

  Proved by running it, not by reading it: an 11 MB log was compressed to `big.log.1.gz` and truncated in
  place to zero bytes, a small one was left alone, and an absent one was skipped.

  Deployment cleanup verified 2026-09-12: the retired rotation script, plist and loaded label are absent.
  Rotation is weekly from here rather than hourly, so a log can sit above the threshold until the next
  weekly run.

### STOP POINT F

The planned Rust lanes are implemented. The following deployment check remains.

- [ ] 57a. Finish uu runtime acceptance and the interruption fixes found on 2026-09-13. The old
  cua-driver drift is resolved: installed config uses `~/.local/bin/cua-driver`, non-secret config and
  the managed job match rendered source, and `uu doctor` exits 0 with styled terminal output. The
  imported Claude-plugin history preserves all 21 rows. Two harmless private runs each logged once and
  released their lock. A separate interruption fixture exposed a defect: SIGINT releases uu's lock while
  its child continues writing, and the interrupted run leaves no log entry. Add bounded cleanup of owned
  children before lock release for SIGINT and SIGTERM, plus durable start/interruption records. Scheduled
  output also duplicates uu's own log because both job streams target that file. Give startup errors a
  separate destination while keeping one application record per run. Track these fixes in
  [the interruption task](https://app.todoist.com/app/task/6hVrcXJrr8FWG4fM).
  [PR #542](https://github.com/webdavis/dotfiles/pull/542) passed required continuous integration and
  merged; local main contains it. After operator deployment, verify an authorized real skills run and
  visual output. The audit found no successful scheduled-run marker; the loaded Sunday-noon job had not
  run, and twelve declared lanes lacked state directories. Record the first scheduled lane verdicts,
  notification result, success marker and streaks separately. A successful manual run or notification
  HTTP 200 does not establish scheduled acceptance. Do not repeat task 11c's retired-job cleanup.

- [ ] 57b. Reconcile B2's approved Herdr plugin-pinning requirement with the requested weekly upgrades.
  Current uu reinstalls plugin source tip and rejects a `pin` setting. Installed Herdr's
  `plugin install --help` exposes `--ref <REF>` (verified 2026-09-12). Record the desired pin/update
  policy, then implement it through that supported interface in uu's configuration and plugin lane. Do
  not silently freeze plugin updates or claim that source-tip reinstalls honor a configured revision
  across weekly updates. Source:
  `~/.claude/projects/-Users-stephen-workspaces-Ivy-webdavis-dotfiles/memory/goal-2026-09-01.md`.

- [ ] 57c. Refresh graphify's existing Claude skill alongside package upgrades. The source adds the
  `uv-graphify-skill` command lane, an app-owned Claude symlink and a first-install seed with
  preservation and partial-destination guards. All 18 private installer checks with 96 assertions, 15
  extra adoption cases, fan-out checks, private uu composition and full `just ship` passed.
  [PR #545](https://github.com/webdavis/dotfiles/pull/545) merged and local main contains it. Preserve
  the existing real skill directory before operator adoption; live installation, fresh Claude discovery
  and scheduled refresh acceptance remain open. No live install or skills run was performed.

- [x] 57d. Acceptance for 57c on dresden: `~/.claude/skills/graphify` is still a real directory dated
  2026-07-05 (observed 2026-09-13), not the link into `~/.local/share/graphify/claude/skills/graphify`
  that `docs/runbooks/agent-skills-store.md` describes, so graphify's weekly self-update never reaches
  Claude Code. After the next `chezmoi apply`, confirm the path is a symlink to that target and that the
  July copy was preserved as 57c requires; if the apply leaves the directory in place, the seed's
  partial-destination guard needs a look. Checked after the 2026-09-13 20:15 apply: the path is the link
  and the target holds the 0.9.53 bundle (`SKILL.md`, `.graphify_version`, `references/`). The July copy
  was NOT preserved: the seed only guards its own `~/.local/share/graphify` destination ("the old Claude
  directory is preserved" in its comment means it never touches `~/.claude/skills/graphify`), and the
  `symlink_graphify` declaration replaced the real directory when chezmoi applied it. No copy exists
  under `~/.claude/skills`, `~/.local/share/graphify` or `~/workspaces/backups`. Loss judged nil: the
  July directory was an older upstream graphify skill, superseded by the bundle now linked.

- [ ] 57e. Retire the `deleteValueAtPath "skillOverrides.<name>"` lines in
  `private_dot_claude/modify_settings.json`. The branch `docs/clean-code-rust-test-first` added nine
  (clean-code, clean-code-rust, clean-code-swift, defuddle, obsidian-bases, obsidian-cli,
  obsidian-markdown, owasp-security, tuicr) so one apply scrubs the stale `user-invocable-only` key a
  promotion to core leaves in the live file. They are tombstones: once every machine that carried those
  keys has applied (check `jq .skillOverrides ~/.claude/settings.json` shows none of the nine), delete
  the lines. Longer term, derive the whole block from the lock's `tiers` table with `include` and
  `fromJson` so a promotion needs one edit and no tombstone; requested in the operator's Plannotator
  review on 2026-09-13.

- [x] 57f. [PR #562](https://github.com/webdavis/dotfiles/pull/562) (`docs/clean-code-rust-test-first`)
  merged (`066fd762`): twelve skills promoted from on-demand to core (clean-code, clean-code-rust,
  clean-code-swift, git-guardrails-claude-code, defuddle, handoff, obsidian-bases, obsidian-cli,
  obsidian-markdown, owasp-security, tuicr, resolving-merge-conflicts), the herdr skill replaced with
  upstream verbatim, a Git worktrees rule (`herdr worktree create`) added to the shared agent rules, and
  this ledger's earlier uncommitted edits committed on that branch. Nothing is applied yet. After the
  operator's next `chezmoi apply`, confirm `jq .skillOverrides ~/.claude/settings.json` no longer lists
  the nine promoted skills that had keys, and that `~/.claude/skills/plannotator-review` and its siblings
  are links. The operator's apply on 2026-09-14 stopped at `58-build-posture`: the posture binary is
  3,692,736 bytes since the watchdog, funnel and digest ports merged, over the 2 MiB ceiling set on
  2026-09-13, so every script after slot 58 was skipped (uu build, LaunchAgent loaders, skills
  first-install, Neovim bootstrap). [PR #563](https://github.com/webdavis/dotfiles/pull/563)
  (`fix/posture-artifact-ceiling`) raised the ceiling to 8 MiB in the data file, the manifests script and
  the two unit fixtures, and merged on 2026-09-14. Proof run the same day from the main checkout after
  57h cleared the nested worktrees: the rendered `58-build-posture` (with `CHEZMOI_SOURCE_DIR` set, as an
  apply does) built and installed posture (3,692,736 bytes) and refreshed the manifests, exit 0; the
  rendered `59-build-uu` installed uu, exit 0. The operator applied at 20:15 local on 2026-09-13 and it
  passed. Acceptance: none of the nine promoted skills has a `skillOverrides` key any more, the four
  plannotator entries under `~/.claude/skills` are links into the store, the rendered
  `~/.claude/CLAUDE.md` carries the worktrees and work-recaps sections, posture and uu are the new
  builds. (An acceptance check that ran while the apply was still writing files read stale copies of two
  targets; re-run after the state file was written, both were current.)

- [ ] 57g. Allowlist the scalebar LaunchAgent. `com.webdavis.scalebar` is loaded by
  `run_onchange_after_*` on every apply but had no tuple in
  `dot_config/osquery/private_page-launchd-allowlist.txt`, so its persistence row pages as an unknown
  agent. [PR #564](https://github.com/webdavis/dotfiles/pull/564) (`fix/scalebar-launchd-allowlist`) adds
  the tuple (plist path, program `~/.local/libexec/scalebar/Scalebar`, no sha256 pin, like the other
  host-owned agents); merged 2026-09-14 (`0a52800a`) and deployed by the 2026-09-13 20:15 apply
  (`~/.config/osquery/page-launchd-allowlist.txt` carries the line). Remaining acceptance: the next
  persistence_launchd finding for that label digests instead of paging.

- [ ] 57h. Nested worktrees leak their `.chezmoidata` into every apply. Measured 2026-09-14 on dresden:
  with the source data file at 8 MiB, chezmoi still rendered `max_artifact_bytes=2097152`, because
  chezmoi reads `.chezmoidata` directories RECURSIVELY and the 78 worktrees under `.worktrees/` (66
  merged, 12 unmerged, 115 GB) each carry their own copy; a nested value wins over the root.
  `.chezmoiignore` does not help (proven in an isolated source: a nested `.worktrees/x/.chezmoidata`
  overrides the root value with or without `.worktrees` in the ignore file), so the CLAUDE.md sentence
  "`.worktrees/` is deliberately NOT in `.chezmoiignore`" is not the protection it reads as, and this is
  also why every `chezmoi execute-template` in the main checkout takes ~40 s (the walk includes every
  worktree's Rust `target/`). Fix: no worktree may live inside the source tree. Move the live ones to
  `~/.herdr/worktrees/dotfiles/<branch>` with `git worktree move` (the herdr rule already puts new ones
  there), remove the merged ones, amend the CLAUDE.md paragraph, and re-verify the render reads 8388608
  before the apply that 57f waits on. Progress 2026-09-14 (operator approved the move-then-delete): 69
  worktrees left `.worktrees/`, the merged clean ones removed and the unmerged or dirty ones kept under
  `~/.herdr/worktrees/dotfiles/`; every one of the 12 unmerged branches still has a worktree. Nine remain
  in place: three under live Workflows (`posture-converge-validation`, `posture-ssh`,
  `b74-render-context`) and six that a process still sits in. Only `posture-converge-validation` and
  `posture-ssh` still carry 2097152, so the render flips to 8388608 once #552 and #549 ship and those two
  move out. Later on 2026-09-14: #548 and #549 merged and their worktrees were removed,
  `posture-converge-validation` merged main in, and the render from the main checkout now reads 8388608.
  The six worktrees held only by stale test processes (leftover `pns` debug daemons, a `trash-cli`
  python, `prettierd`) were moved out as well once their data was diffed (each carried an older
  `rust_tools.yaml`, three an older `system_packages_autoinstall.yaml`, so they would have fed the apply
  stale package lists too), and `chezmoi data` now matches the root files (ceiling 8388608, 131 formulae,
  55 casks). Only `posture-converge-validation` remains nested, with zero data drift, until #552 ships.
  The CLAUDE.md paragraph merged in [PR #567](https://github.com/webdavis/dotfiles/pull/567)
  (`docs/worktrees-outside-source-tree`, 2026-09-14). The last nested worktree left with #552's merge;
  `.worktrees/` is empty. Still open: the worktree registrations git holds outside the repo (187 before
  the cleanup, most under `~/.herdr/worktrees/dotfiles/` and temp paths), which need an audit of their
  own.

- [x] 57j. Espanso `,,ee` for `echo $?` (operator request 2026-09-14):
  [PR #565](https://github.com/webdavis/dotfiles/pull/565) (`feat/espanso-echo-exit-status`) adds the
  match to the Commands section of `snippets.yml`; merged 2026-09-14 (`1fdfb288`) and deployed by the
  2026-09-13 20:15 apply (the deployed file carries the trigger).

- [ ] 57l. clean-code as a Claude Code plugin (operator 2026-09-14: a plugin for Claude specifically,
  skills for the other harnesses, no duplication). Facts that shape it: plugin skills are always
  namespaced (docs: "`/my-first-plugin:hello` … to prevent conflicts"), so the bare `/clean-code` cannot
  come from a plugin; Claude Code runs an installed copy refreshed only on a version bump (57j's lesson);
  Codex scans the store natively and hermes symlinks into it. Design: a directory marketplace
  `private_dot_claude/clean-code-marketplace` beside `strategy-marketplace`, plugin `clean-code`, skills
  `rust`, `swift` and `base`, each a THIN SKILL.md that loads the store copy
  (`~/.agents/skills/clean-code-rust` and siblings stay canonical, so the wrappers never change when the
  content does and no version bump follows a content edit). `base` reads `$ARGUMENTS` as the target
  language, asks (rust, swift, other) when none is given, and for other languages applies the base method
  with model knowledge and whatever LSP is attached. Claude drops its three store symlinks so the picker
  shows only `/clean-code:rust`, `/clean-code:swift`, `/clean-code:base`; the lock's Claude delivery
  rows, `modify_settings.json` (marketplace declaration and `clean-code@clean-code` roster entry,
  darwin-only like pns and strategy), `.chezmoiignore` (darwin-only directory) and the skills runbook
  follow. Built and merged 2026-09-14 in [PR #573](https://github.com/webdavis/dotfiles/pull/573)
  (`feat/clean-code-plugin`, `d09ab399`). Decisions made on the way: the lock's Claude delivery value
  stays `"none"` (uu's roster reader rejects any other value; the plugin is named in the lock comment and
  the runbook instead); the wrapper descriptions keep the store skills' semantic triggers so the
  core-tier auto-load still fires on Rust or Swift work nobody named the command for (review finding,
  fixed); the wrappers name no companion files, only the general "every relative link resolves in the
  store" assurance; marketplace counts in the settings template and runbook corrected to nine (babysitter
  had been omitted). Claude's three old store links were trashed by hand on 2026-09-14 (chezmoi never
  deletes an undeclared target). Operator steps after the next apply: `claude plugin install clean-code`
  (bare form on 2.1.257), restart Claude Code, then `/clean-code:rust`, `/clean-code:swift`,
  `/clean-code:base` exist.

- [ ] 57m. zoetrope (operator request 2026-09-14): `brew install furkankly/tap/zoetrope` (0.2.0, `zoe`)
  and `herdr plugin install furkankly/zoetrope/herdr-plugin` (`furkankly.zoetrope`, enabled) done by hand
  on dresden; the tap, trusted tap, formula and herdr plugin roster entry merged in
  [PR #571](https://github.com/webdavis/dotfiles/pull/571) (`feat/zoetrope`, 2026-09-14, `6da70bfb`). The
  operator ran `herdr plugin action invoke setup-keys --plugin furkankly.zoetrope` (succeeded): it
  appended a 25-line managed block to `~/.config/herdr/config.toml` (`[[keys.command]]` binding
  `prefix+shift+z` to `furkankly.zoetrope.open`, plus two commented placements), the same
  plugin-writes-into-config drift class as 57k, so the block was copied byte for byte into
  `dot_config/herdr/config.toml` in [PR #572](https://github.com/webdavis/dotfiles/pull/572)
  (`fix/herdr-zoetrope-keys`, merged 2026-09-14, `6c571969`). Acceptance: the next apply neither asks
  about the file nor drops the `prefix+shift+z` binding.

- [x] 57k. Every apply asked `.config/herdr/config.toml has changed since chezmoi last wrote it?` (seen
  on the operator's 2026-09-13 20:31 apply). Cause: the source carried the herdr-agent-quota sidebar row
  pretty-printed by taplo while the plugin's `configure` action, which `run_after_53` invokes on every
  apply, rewrites the same row on one line, so chezmoi's write and the plugin's never matched (TOML
  content equal, bytes not). Fixed in [PR #570](https://github.com/webdavis/dotfiles/pull/570)
  (`fix/herdr-config-quota-row-bytes`, merged 2026-09-14, `2ca552b7`): the source carries the plugin's
  own line, verified byte-identical to the live file, and `dot_config/herdr/config.toml` joins
  `dot_aerospace.toml` in taplo's exclude list. Accepted: the operator's 2026-09-13 20:50 apply ran
  without the prompt and the live file has zero drift from the source afterwards. A plugin update that
  changes the row brings the prompt back once; copy the new line into the source then.

- [ ] 57i. The post-commit graphify hook races the pre-push lint gate. Seen twice on 2026-09-14 (scalebar
  and espanso pushes made right after their commit): `chezmoi execute-template` in
  `shellcheck-rendered-template` aborts with
  `lstat .../graphify-out/cache/ast/<hash>.tmp: no such file or directory` because the hook is still
  rewriting its cache inside the source tree while chezmoi walks it, so the gate reports "lint drift"
  with 0 files changed and the push is refused. A second push a minute later passes. Fix candidates,
  robust first: move graphify's cache out of the source tree (its output directory setting, or a symlink
  like the `minutes` one), or have the pre-push hook wait for a running graphify rebuild before the gate;
  never a retry loop in the gate.

## posture cleanup

- [ ] 58. posture 8.1 to 8.3: implement the SSH hardening port in its three planned stages. The command
  is implemented on `feat/posture-ssh` at source `539ecbb0`, with all three stages committed. Full
  `just ship` passed, including 991 posture tests; all 86 new tests passed individually within one
  second. Eight mutation controls and sixteen private Bash/native comparisons passed. Exit status, final
  bytes/modes and recorded restart effects agree; error text is not universally byte-identical.
  Independent review is checking the additional Match-scan bounds and restoration when a rename takes
  effect but reports failure. On 2026-09-13 `main` was merged in at `150fe38c`, resolving one dispatch
  conflict in `posture/crates/posture/src/lib.rs` against poll (#544); the posture Rust gate and
  `just ship` (exit 0, 4m45s) passed on the merge, and
  [PR #549](https://github.com/webdavis/dotfiles/pull/549) was opened against `main`; it is unreviewed
  and unmerged. Its first CI run failed the 600 ms bound in
  `every_sshd_reader_is_bounded_and_a_later_reader_gets_its_own_deadline` (three 80 ms reads, three
  process spawns on the loaded runner); `c74ebf8a` widens the bound to 3 s with a control arm (a deadline
  mutated to 2000 ms still fails it in 6.08 s, the real reader passes in 0.32 s) and CI was rerun on the
  push. Independent review returned six findings and the fix is pending. SEV-1: `arm()` installs
  INT/TERM/HUP handlers without checking the inherited disposition, so under `nohup` a dropped session
  rolls back a valid install and re-raises to kill the process; the fix skips signals already set to
  `SIG_IGN`. SEV-2: several sub-second timing bounds that a loaded runner can exceed. SEV-2: a naming
  collision (`ports` shadowed). Three further SEV-3 findings. Fixed 2026-09-14: `arm()` now skips any
  signal already at `SIG_IGN` (`ddc0226b`, pinned by a test that sets `SIG_IGN` first); the wall-clock
  upper bounds became behavioral evidence (`fdf15bf6`, `e836fc63`), keeping one lower bound and a 10 s
  watchdog; the `ports` shadow was judged not a bug (the later use is the only read and is the intended
  binding). Main was merged in (`4ab0feb1`, three additive conflicts in the crate `lib.rs` export blocks,
  resolved as unions), the second review passed, and
  [PR #549](https://github.com/webdavis/dotfiles/pull/549) merged 2026-09-14 (`235891e4`); the worktree
  is removed. A pre-existing grace-test flake (30 ms grace under load) was fixed in the same round. Live
  configuration/output acceptance remains before Bash retirement. Evidence:
  `/private/tmp/dotfiles-modernization/task58/HANDOFF.md`.
- [ ] 59. posture 9.1: relocate posture controls and desired state out of the legacy `osquery/` tree, add
  coverage for relocated data and update its consumers, then retire the old managed scripts and approved
  deployed leftovers. Remove the old `osquery/*` tracking only after the deployed directory is empty.
  Coordinate that removal across watch paths, manifests and Rust manifest selection. Keep osqueryd
  installed as the query producer and perform the plan's operator-run restart after changing its watched
  paths. Controls source `4b3d59f4` passed independent review, including ten integrity checks, eleven
  poll checks and four Bash checks. Missing new controls cannot be hidden by stale legacy data.
  Desired-state commit `916319c5` also passed separate review: six files moved with identical bytes, and
  73 Rust plus 53 Bash consumer tests passed privately. Integration with task50 retains its validation
  guards and the new data default. On 2026-09-13 the converge branch was merged in (`5055279e`),
  `just ship` passed (exit 0, 4m21s) and [PR #553](https://github.com/webdavis/dotfiles/pull/553) was
  opened with base `fix/posture-converge-validation`, so it shows only the two relocation commits.
  Independent review returned two findings, fixed at `0edbb5ef` and pushed, with the PR body re-posted:
  stale old paths in `CLAUDE.md`, and the controls file's consumer note claiming `posture poll` reads it
  today. It also found no defect in the move and one operational fact the diff had not recorded: the
  whole `~/.local/libexec/osquery/` tree is under the pipeline-integrity watch and every DELETED event on
  a tracked path pages CRIT, so the hand cleanup after the apply pages nine times (seven stale files:
  `posture-controls.json`, `osquery-converge/desired/osquery.conf`, `osquery.flags` and the four
  `packs/*.conf`, plus the `desired/packs` and `desired` directories). Remove them in one `trash` pass so
  the pages arrive together; `osquery-converge/drift-verdict.sh` stays managed. On 2026-09-14 the PR was
  retargeted onto main after #552 merged, main merged in cleanly (`9c99a880`), CI passed after one rerun
  (the first run died on the runner's DNS, not the branch), and it merged (`6478254c`); the worktree is
  removed. Deployed 2026-09-13 20:50: the apply rebuilt pns, posture and uu, the relocated data is at
  `~/.local/libexec/posture/` (`controls.json`, `converge/`), and the operator ran the one `trash` pass
  (`posture-controls.json` and `osquery-converge/desired/` are gone). Restart acceptance (the plan's
  operator-run osqueryd restart after the watched paths changed) remains open.
- [ ] 60. posture 9.2: finish the completion report, original 187-test successor/disposition mapping,
  before/after table and decision index. `posture/docs/test-baseline.tsv` is only the original result
  inventory. Preparatory mapping on `docs/posture-test-mapping` at `d95c39f3` preserves all original
  columns and maps all 187 leaves to exact assertions or explicit gaps. It records merged, unpublished
  and deployed status separately. On 2026-09-13 `main` was merged in (`93f3f288`), `just ship` passed
  (exit 0, 4m31s) and [PR #554](https://github.com/webdavis/dotfiles/pull/554) was opened against `main`;
  it merged into `main`. Independent review found the README's Task58 row and the B041 bullet needed to
  name the real constants; fixed at `068e34e1` before merge. The final post-port size comparison and
  decision index are still required. The Rust size gate already covers posture; do not add it again.
- [ ] 60a. Resolve the behavior gaps found by the original-test mapping before final posture closure. The
  private B020/B027 reproducer loses valid digest records when one invalid UTF-8 byte makes a claimed
  batch unreadable; a focused preservation fix is in progress on `fix/posture-digest-read-failure`, which
  merged `main` in (tip `74166d25`); `just ship` passed (exit 0, 5m15s) and
  [PR #558](https://github.com/webdavis/dotfiles/pull/558) was opened against `main`. Independent review
  returned four findings; three are fixed and pushed: torn lines are now dropped and counted, fixed at
  `d2e18d3c`; an unclaimable spool exits 1 with a stderr line, and the LaunchAgent has no `KeepAlive` so
  the uptime watchdog pages at a streak of two, fixed at `45a64bc4`. The fourth finding belongs to #559.
  B041/S290's four `DIGEST_MAX_*` scalar overrides are restored on `fix/posture-digest-limits`
  (`7030da0a`, six command-level cases in `posture/crates/posture/src/digest/tests/limits.rs`; posture
  Rust gate and `just ship` exit 0 after merging `main` at `cee02e20`);
  [PR #550](https://github.com/webdavis/dotfiles/pull/550) was opened on 2026-09-13 and merged into
  `main`. Independent review returned three SEV-3 findings (test string coverage, doc comments and
  `FIELD_LIMIT` export, and `mod limits;` placement); fixed at `e5b5f877` before merge. B142/S122's
  named-spool append-failure diagnostic is still missing from native code. The
  `fix/posture-spool-diagnostic` branch merged `main` in; its first `just ship` failed lights'
  `missing_config_exits_five` test with "unexpected request", unrelated to the branch: the lights test
  fixture `home()` builds `temp_dir()/lights-<pid>-<counter>` and never removes it, so a reused process
  id finds a stale `config.toml` (1,760 leaked directories measured, 844 holding a `config.toml`). The
  fix merged as [PR #561](https://github.com/webdavis/dotfiles/pull/561): `Home::fresh` clears its
  directory on create and removes it on drop, with three fixtures routed through it (commits `72434bbd`,
  `a60cf1cf`, `d224062c`). With the flake understood, `fix/posture-spool-diagnostic`'s tip `dc77f4cf`
  (after merging `main`) re-ran `just ship` (exit 0, 3m58s) and
  [PR #560](https://github.com/webdavis/dotfiles/pull/560) was opened against `main`. Review returned
  three findings, fix in progress: SEV-2, the append diagnostic writes to process stderr instead of the
  injected sink at `digest_appender.rs:41`; SEV-3, a pid-keyed temp root in `alert/tests/spool.rs`;
  SEV-3, a 650 ms wall-clock bound. B039 is reproduced and fixed on `fix/posture-digest-fold-append`
  (stacked on the read-failure branch): `fold` did read-then-rewrite despite its "append rather than
  rename" comment, and the new `digest_spool/tests/concurrent_fold.rs` loses 135 to 258 of 1000
  concurrent appends against it and none against the `O_APPEND` fold. At tip `7db593c2`, `just ship`
  passed (exit 0, 3m06s) and [PR #559](https://github.com/webdavis/dotfiles/pull/559) was opened stacked
  on #558. Fable review returned one SEV-low finding, the fourth deferred from #558's review: the spool
  is opened twice in `fold`; a fix to a single read-and-append handle is in progress. The old fold lost
  148 of 1,000 racing lines; the new one lost none in 25 runs. The same two-thread reproducer driven
  through `claim` and `restore` still loses 1 to 2 of 1000: an append through a handle the alerter opened
  before the claim rename lands in the claim after the digest read it. That window predates the port (the
  Bash `>>` had it) and is recorded here as B039b: the proposed fix is for the appender to re-check the
  spool's inode after its write and re-append to the fresh spool when the file was renamed under it,
  accepting a possible duplicate line in one digest. B001/B002 retain narrower detached-child lock and
  two-process single-notification coverage gaps. Reconcile the recorded empty-bundle-path and quoted-zero
  normalization decisions against current assertions. Five former jq/pipe fault-injection dispositions
  remain proposals, and thirteen legacy queue leaves retain their Bash owner until task 49's acceptance.
  On 2026-09-14 [PR #558](https://github.com/webdavis/dotfiles/pull/558) merged (`d2e18d3c`..`45a64bc4`),
  [PR #560](https://github.com/webdavis/dotfiles/pull/560) merged (`79f28454`), and
  [PR #559](https://github.com/webdavis/dotfiles/pull/559), retargeted from #558's branch onto `main`
  before merging, merged (`45277874`). Evidence:
  `/private/tmp/dotfiles-modernization/task60-mapping/HANDOFF.md`. Unrelated to posture:
  [PR #557](https://github.com/webdavis/dotfiles/pull/557) (`fix/pns-private-process-budget`, the pns
  fixture process budget) merged into `main`, reviewed NO_ISSUE, after continuous integration passed.

### STOP POINT G

posture owns the migrated pipeline and the legacy Bash tools are retired. osqueryd remains the query
producer.

## The tail

- [x] 61. lights argument-surface comparison is merged in
  [PR #541](https://github.com/webdavis/dotfiles/pull/541), and local main contains it. Independent
  review verified 169 cases against the frozen owned Bash reference and rejected changed-exit and
  changed-power-write controls. All 95 Rust checks, the release build, full `just ship` and required
  continuous integration passed. Each comparison completed within one second. Tasks 62 and 63 retain
  their hardware and manifest-policy gates.

- [ ] 62. lights PR 12: move all seven aerospace keys F4 to F10 to `~/.cargo/bin/lights`. Five still call
  Bash and two call OpenHue directly. Complete the three remaining command/hardware drills, then verify
  actual key presses and held-key behavior after apply. Retire the script and propose manual cleanup of
  its deployed copy and obsolete logs after acceptance.

- [ ] 63. lights: decide manifest coverage for `~/.cargo/bin/lights`, its current install target. The
  existing generated-binary exception covers posture only. Update the stale target in the lights plan and
  spec when recording the decision.

- [x] 64. lights PR 11a is unnecessary under the recorded bulk-read decision. Bulk measured 210 ms,
  versus 267 ms and 455 ms for the targeted alternatives. Keep bulk and record the accepted deviation
  from the 150 ms design target. The other three hardware drills gate 62.

- [ ] 65. Neovim task 63: finish the acceptance record required by PR #385. Capture five silent starts,
  full-plugin health output, quiescent startup comparison, rendered which-key groups, both agent loops,
  Swift/custom-plugin behavior, a clean-home apply and quiet repeat apply, and the inventory-to-merged-PR
  mapping. Synthetic/headless runs do not establish rendered acceptance. Reconcile the stale expected
  `X = xcode` and `d = do` groups with current `x = xcode` and `d = docker` before the operator checks.
  Reconcile `dot_config/nvim/docs/todo.md`; bootstrap, neotest, annotation extraction and autosave/format
  coordination already exist in source. Keep deferred formatter/linter and agent-protocol evaluations
  separate from this acceptance task. On 2026-09-13 the documentation reconciliation commit from the
  superseded ledger branch was cherry-picked onto `docs/nvim-acceptance` (`7e81209c`, three files: the
  acceptance record's inventory section, the nvim todo status list, the nvim `CLAUDE.md` loading model);
  `just ship` passed (exit 0, 3m39s) and [PR #555](https://github.com/webdavis/dotfiles/pull/555) was
  opened against `main`; it merged into `main` on 2026-09-13. The ledger branch's
  `docs/remaining-work.md` edits were older versions of this file's current text and were dropped. Write
  `docs/research/2026-09-nvim-overhaul-acceptance.md`. Run `Lazy! load all` before health capture. Retain
  the plan's synthetic warm-start pass condition, `after < baseline - 10`, and separately record a
  rendered Herdr start with every `VeryLazy` plugin loaded. Keep cold and rendered-start timing as
  recorded measurements, as the plan specifies. The
  [acceptance record](research/2026-09-nvim-overhaul-acceptance.md) now distinguishes completed private
  checks from remaining rendered, device, agent-session and deployment checks. Source inventory accounts
  for all 90 entries through 59 verified merged-PR receipts. Source documentation reflects existing lazy
  loading, autosave, formatting and test integration; rendered acceptance and the listed language
  decisions remain open.

- [x] 66. tailnet-pin: the Rust crate replacing `reconcile-hosts-pin.sh`. Two limits of the shell went
  with the port. A line carrying a NUL byte is now copied through whole, where `read` dropped the NUL and
  joined the two halves because no shell variable can hold one; and the temporary file is removed by
  `Drop` rather than by six signal traps, so there is no signal list to keep in step with a test.

- [x] 66b. tailnet-pin cutover: the builder at `run_onchange_after_40`, the `run_onchange_after_41` call
  site, and the source script and its bash suite deleted. The caller's SHA256 pin of a source file became
  a SOURCE FINGERPRINT the builder records after installing, because a built binary's bytes do not exist
  at render time; the runner refuses every pin unless that record matches what this apply rendered, which
  is what stops a deferred build from aiming a stale binary at `/etc/hosts` as root.

- [x] 66c. DONE 2026-09-09. Trashed the deployed `~/.local/libexec/tailscale/reconcile-hosts-pin.sh` and
  its now-empty directory, after an apply has installed `~/.cargo/bin/tailnet-pin`. Chezmoi does not
  delete a target whose source entry is gone, and this repository builds no removal mechanisms, so it is
  one operator command.

- [x] 66a. herdr: the clean-code pass on `dot_local/share/herdr/plugins/herdr-smart-nav`. The direction
  became an enum, which closed a pair that could disagree: the word and the chord travelled side by side
  as two strings, so a call passing `"left"` with `ctrl+l` compiled and sent Neovim the wrong way. Every
  public item gained the documentation the house voice asks for, and the parse of herdr's answer states
  why every unreadable shape means the same thing.

- [ ] 68a. Extract each tool into its own public repository with `git subtree split`, once the operator
  has hand-rewritten it and is ready to tag a v1. Deferred from tasks 20 and 21; the monorepo layout
  exists so this is a move. Nothing is published to crates.io while a tool is pre-v1.

## Repository hygiene

- [ ] 67. Reconcile local branches before further cleanup. On 2026-09-13 there are 577, of which 518 are
  ancestors of `origin/main`. No `wf_*`, `worktree-agent-*` or `agent-*` branches remain. The four
  `backup/*` branches contain unmerged work and were deliberately retained by the Claude session.
  Classify the other throwaway candidates by reachability, attached worktree, dirty state and owner.
  These counts do not authorize deletion. Obtain approval for the exact proposed groups.

- [ ] 68. Finish the worktree inventory and approved cleanup. There are 238 registrations: 28 under
  `~/.herdr/worktrees`, 75 in this checkout's `.worktrees`, 108 under `~/workspaces/dotfiles-worktrees`,
  19 under `~/workspaces/dotfiles-agent-worktrees`, and eight elsewhere. One Claude scratch worktree
  registration points at a missing directory. The old count of 195 under Herdr is obsolete. Preserve
  active pull requests, retained commits and dirty work; recheck candidates immediately before approved
  `git worktree remove` or registration pruning. Do not treat an old cleanup inventory as current
  consent. In particular, compare and preserve the uncommitted source in `pns-refactor-6-5`,
  `.worktrees/pns-executable-deadline`, `.worktrees/herdr-smart-nav-clean-code` and
  `.worktrees/lights-implementation`. They contain source edits beyond generated graph drift; their
  presence does not by itself prove missing implementation. Continuation handoff, 2026-09-13: these 11
  worktrees were newer than `feat/tuicr-config` and were not tied to a merged pull request. Inspect them
  before starting duplicate work:

  - `.worktrees/nvim-acceptance-ledger` (`docs/nvim-acceptance-ledger`, `5001c94f`)
  - `.worktrees/nvim-mcp-boundary` (`fix/nvim-mcp-boundary`, `295b84e3`, PR #546 open)
  - `.worktrees/pns-daemon-fixture-children` (`fix/pns-daemon-fixture-children`, `c6b99b96`)
  - `.worktrees/posture-controls` (`refactor/posture-controls`, `a4907309`)
  - `.worktrees/posture-converge-validation` (`fix/posture-converge-validation`, `2409bca7`)
  - `.worktrees/posture-digest-read-failure` (`fix/posture-digest-read-failure`, `50dbaea9`)
  - `.worktrees/posture-funnel` (`feat/posture-funnel`, `adb23b54`)
  - `.worktrees/posture-spool-diagnostic` (`fix/posture-spool-diagnostic`, `f6cf9e8d`)
  - `.worktrees/posture-ssh` (`feat/posture-ssh`, `c92887a4`)
  - `.worktrees/posture-watchdog-health` (`feat/posture-watchdog-health`, `a57d9346`)
  - `.worktrees/task60-test-mapping` (`docs/posture-test-mapping`, `d95c39f3`)

  This is the continuation set from the interrupted modernization run. Branches already contained in
  `origin/main`, including completed worktrees that remain registered, are intentionally excluded.

  The following additional worktrees are excluded only because their branch tips are already contained in
  `origin/main`; their dirty files still require inspection before cleanup:

  - `.worktrees/b74-render-context` (`fix/b74-render-context`, `b1adf180`): staged renderer-context
    changes for the B74 concurrent render race, including a new formatter library and tests. Preserve the
    source changes and compare them with `/private/tmp/dotfiles-modernization/b74-hiiey5u7/RESULTS.md`.
  - `.worktrees/posture-digest-limits` (`fix/posture-digest-limits`, `b1adf180`): dirty native digest
    limit implementation and tests for task 60a. Inspect before starting another B041/S290 fix.
  - `.worktrees/herdr-process-plan` (`feat/herdr-process`, `4014ff49`): untracked Herdr process-plugin
    source and fixtures for the approved configurable floating-process-window plan. Do not rebuild this
    feature without reviewing the existing source.
  - `.worktrees/fix-review-skill-delivery` (`fix/review-skill-delivery`, `9432c139`),
    `.worktrees/posture-digest-read-failure` (`fix/posture-digest-read-failure`, `50dbaea9`) and
    `.worktrees/posture-spool-diagnostic` (`fix/posture-spool-diagnostic`, `f6cf9e8d`): current dirty
    changes are generated `graphify-out/graph.json` only. Inspect status before discarding anything.

  Start from local `main` at `3d08b5a5`. Pull requests #530 through #545 are merged; #546 is open with
  lint passed. Do not repeat those implementations. The watchdog, funnel, converge, controls, SSH and
  task 60 branches above have passed their recorded private reviews but still need the publication or
  acceptance steps stated in tasks 46, 48, 50, 58, 59 and 60. The operator deleted the 126 untracked
  Graphify merge-driver artifacts and `nvim.log` after their scope was reviewed on 2026-09-13. Do not
  restore them. The current `docs/remaining-work.md` edit is also uncommitted because this environment
  cannot create the Git index lock. Do not restore it. Agents do not run `chezmoi apply`; the operator
  does that after publication.

  Operating rules from the operator, 2026-09-13, second round, replacing the earlier allocation note:
  work continues in a fresh session from this ledger plus the memory file `resume-2026-09-13-handoff`.
  Each pull request runs as one Workflow script (review, fix, re-check, gates) reporting once. One review
  per pull request, run after the fix; none for docs-only or test-only pull requests. Agents are not put
  on a reading diet. Fable orchestrates, Opus implements, Sonnet does mechanical work.

- [x] 68b. Review and remove the untracked `.merge_file_*` artifacts in this checkout. They were Graphify
  merge-driver residue, not source. The operator deleted all 126 matching artifacts and `nvim.log` on
  2026-09-13 after reviewing their scope. Verify the current inventory before any further cleanup; do not
  recreate or restore them.

- [x] 68c. Published the approved planning edits on 2026-09-13:
  [homelab #38](https://github.com/webdavis/Homelab/pull/38) and
  [dotfiles #532](https://github.com/webdavis/dotfiles/pull/532) are merged. Dotfiles local main was
  fast-forwarded to `fa26dd6e`. The original two planning edits remain in a scoped stash named
  `preserve task68c original planning edits before reviewed main sync`; all 127 untracked paths and their
  modes were preserved. Homelab local main contains upstream while retaining its 44 previously
  unpublished commits and unrelated work. Do not push that local history or discard the retained edits.
  Publication did not deploy either plan.

- [ ] 21a. Finish the deployed binary cleanup named in task 21. The old
  `~/.local/libexec/{pns/pns,uu/uu,posture/posture,lights}` binaries remain. Verify current callers,
  preserve the live `pns/hooks/` installer directory, and obtain approval for the exact obsolete files
  before trashing them. The September 13 caller audit found two Codex hooks still invoking the old pns
  binary alongside current handlers. Commit `1b0cca44` on `fix/codex-pns-hook-migration` migrates
  precisely owned legacy commands while preserving unrelated handlers and metadata. Follow-up `257cb3e1`
  fixes four ShellCheck findings in its test. Ten focused cases, synthetic installer checks, full
  `just ship` and independent review passed. [PR #534](https://github.com/webdavis/dotfiles/pull/534)
  merged and local main contains it. Operator deployment and hook-trust review remain separate from
  exact-file cleanup approval.

## Waiting on the operator

Each of these gates work that cannot start without it.

- [x] Add the pns-keyed gateway route, gated tasks 43 to 50. CLEARED 2026-09-09, and it turned out to
  have been done already. The open question below could not confirm it because the hermes config is
  age-encrypted; decrypting the SOURCE and counting route keys alone shows both `priority` and `pns`, so
  the route is durable rather than a hand edit of the deployed copy that the next apply would erase.
  `bypass_silence_classes` is in the pns config schema, which is the plan's step 0.5.
- [x] Certify the stable Rust toolchain, gated tasks 51 and 52. CLEARED 2026-09-09, and certifying it is
  what found the real problem. The machine's stable was 1.88.0 from June 2025 while CI's is 1.98, so the
  two were never the same toolchain; `pns-adapters` uses `file_lock`, stabilized in 1.89, so PINNING the
  old stable would not have compiled at all. After `rustup update stable` to 1.98.1, `just test-rust`
  exits 0 across all six workspaces, 102 green test binaries, and no crate uses `#![feature]`. The
  default toolchain stays nightly; task 51 is what pins stable per directory.
- [x] Stop and retire the hourly log writer, task 57. Source retirement landed and the deployed script,
  plist and loaded label are absent on 2026-09-12. No further bootout or removal remains.
- [ ] The clean-home apply from PR #385, gates task 65
- [ ] The lamp drills, gates task 62. ONE OF FOUR DONE 2026-09-09: `bulk_read_latency` is measured and
  answered. Seven samples each against the operator's own bridge: the shipped bulk read of
  `/clip/v2/resource` runs a 210 ms median (127 min, 261 max), which is over the design's 150 ms bound,
  but the targeted strategy the plan named as its alternative measures WORSE, at a 267 ms median for the
  two-call room-plus-grouped form and 455 ms for the three-call form that adds scenes. Verdict: keep the
  bulk read, do not adopt targeted. The other three (`seven_commands_preserve_key_intent`,
  `brightness_floor_and_power`, `held_steps_match_isolated_steps`) still need the operator's eyes, and
  must run against the Kitchen or MBedroom rather than the Studio: `[lights.lamp.*]` routes loop, blocked
  and unread to four lamps including `3F - Studio - HCL3`, so pns animates the Studio while an agent is
  working and every brightness reading taken there is mid-animation.
- [x] Archive `webdavis/neovim-config` and remove `~/.config/nvim/.git`. Both verified complete on
  2026-09-12.
- [ ] Approve the branch and worktree deletions, gates tasks 67 and 68
- [x] Deploy the uu skills lane, task 11c. Live configuration, binary usage and launchd metadata confirm
  it. The later config drift is task 57a.
- [x] Retire the deployed tailnet pin script, task 66c. The script is absent and
  `~/.cargo/bin/tailnet-pin` exists on 2026-09-12.
- [x] Restart Claude Code after removing `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE`. The operator reported
  restarting in the 2026-09-10 handoff.

## Additional work recovered from the backlog

These items were absent from this file but remain in the recorded modernization scope. The source
references identify where to resume; an old unchecked plan or Todoist task alone is not proof that code
is missing.

### pns validation follow-ups

- [ ] Reconcile Part 2's remaining configuration and device acceptance against
  `~/.claude/projects/-Users-stephen-workspaces-Ivy-webdavis-dotfiles/memory/pns-part2-scope.md` and
  current source. That record clears the UniFi credential gate on 2026-08-20 and records implementation
  complete on 2026-08-31, with apply/configuration/drills still pending then. Some original proposals
  were explicitly declined or reshaped; do not revive the whole A to H proposal as unapproved new work.
  Verify which drills subsequently passed, including actual phone display where needed. This audit did
  not test present credential validity or run device drills. Explicitly reconcile drills 28/29 for Codex
  and pi/Hermes, NotHome acceptance (Home was recorded passed), and the post-apply blocked/loop/daylight
  visual comparison. Standalone lights acceptance does not close these pns checks. Include the recorded
  total-runtime performance pass and Part 2 intent review; neither has established closure in this sweep.
  Configuration generation and opt-in setup already exist; reconcile their acceptance records rather than
  reimplementing them.
- [ ] Evaluate native macOS probes for pns, approved 2026-09-13. Benchmark the current `ioreg` idle-time
  and screen-lock probes and the `pgrep`/`ps` process queries used for phone-session activity. Compare
  probe latency and total pns runtime under representative load with small Rust adapters using maintained
  IOKit bindings and the existing `libc` dependency where suitable. Reuse bindings to Apple's system
  interfaces; keep the adapter inside pns and limited to the calls it needs. Adopt a replacement only
  when measurements show a worthwhile benefit and behavior checks pass. Preserve unknown readings,
  lock/idle routing, process and terminal matching, bounded execution and handling of processes that exit
  during a query. Record the measurements and retain the current commands if the replacement is not an
  improvement. Include this in the existing
  [pns performance task](https://app.todoist.com/app/task/6hPxWVHM8pG4qgwp). Keep `terminal-notifier`
  unless a demonstrated feature gap justifies taking over notification permissions, app identity and
  click handling. Keep `rusqlite`/SQLite and supported external-tool interfaces. Focus detection is
  already Rust; changing languages does not remove its dependence on undocumented Apple files. A general
  translation framework or rewrite of third-party implementations is outside this task. This approval
  schedules the investigation and conditional replacements. September 13's private prototype measured the
  parallel desk/phone probe stage at 216.7 ms median with current commands versus 18.8 ms through bounded
  native helpers; added-load medians were 276.8 ms and 27.4 ms. Sixty live parity comparisons passed for
  observed conditions, but this does not cover locked-state transitions or total pns runtime. The
  direct-call variant loses interruptible deadlines. The native phone candidate also misses an `argv[0]`
  match accepted by current `pgrep`; do not adopt it as equivalent. Resolve that selection mismatch,
  compare a hybrid retaining `pgrep` if useful, and measure total runtime before adoption. Keep
  production probes unchanged until those checks and required device acceptance pass. The bounded
  prototype uses maintained `objc2-io-kit` and Core Foundation bindings with the existing `libc` version.
  Raw activity readings remain private in the local investigation, not in this repository. The private
  hybrid follow-up retained actual `pgrep -x` selection and fixed the argument-zero witness mismatch.
  Bounded phone medians were 207.8 to 39.6 ms ambient and 238.2 to 43.2 ms under added load. A complete
  pns process with private destination stubs measured 239.9 to 83.1 ms and 270.8 to 88.0 ms respectively.
  All 320 whole-process runs completed and 50 bracketed comparisons agreed. These measurements exclude
  real delivery, daemon and hook latency. The candidate combines bounded native desk probes with hybrid
  phone selection. Its five-second total phone deadline is tighter than the existing chain's three
  separate budgets and needs an explicit adoption decision. Actual device transitions, unreadable
  devices, multi-user behavior and stalled native calls remain acceptance gates. All 33 original
  investigation hashes were preserved; production is unchanged.
- [ ] Finish P4's recorded loop rule: a live loop lease for the pane prevents a condenser-generated
  `asking` guess from arming the blocked marker; actual hook-driven waits still do. The current submit
  path updates that marker without checking the lease. Read the instrument evidence before implementing
  the companion permission-mode filter for false blocked alerts; its cause was never established in the
  reviewed records. Do not suppress all subagent approvals. The condenser prompt correction already
  shipped. Resume from `~/.claude/pipeline/slices/brief-pns-one-moment.md` and the September 1 decision
  in
  `~/.claude/projects/-Users-stephen-workspaces-Ivy-webdavis-dotfiles/memory/pns-lights-lock-sheet.md`.
  Local implementation `4136fd42` on `fix/pns-loop-rule` passed eight new regressions, three mutation
  checks and package gates. Independent review passed 52 focused checks. The permission-mode filter
  remains excluded. [PR #536](https://github.com/webdavis/dotfiles/pull/536) combines this change with
  B18 while preserving separate commits. Combined `just ship` and the installer release build passed;
  required checks passed and the PR merged. Operator deployment and visual acceptance remain open.
- [ ] Resolve the historical condenser-stall task
  [6hPCHVmfhXPM9FPM](https://app.todoist.com/app/task/6hPCHVmfhXPM9FPM). The named hook test still has a
  300 ms condenser deadline; production now bounds post-stdout waiting and cleans up process groups.
  Reproduce under representative load and record closure or fix the remaining cause. The audit found no
  demonstrated current failure and did not rerun the load drill.
- [ ] Split and reconcile [6hPJVf2FJc3RHxqM](https://app.todoist.com/app/task/6hPJVf2FJc3RHxqM). Ordinary
  hook fixtures still inherit the five-second payload deadline and need bounded fixture inputs. The
  Hermes redirect fixture already consumes the complete request and keeps its socket until disconnect;
  commit `f3b5a21b` records that repair. Verify historical closure without rebuilding it. Keep production
  deadlines distinct from fixture ceilings and follow the repository's test-runtime policy. Explicitly
  include B105's approval-submission exit-code failure, historically `0` instead of `42` under load. The
  September 7 disposition leaves it unresolved after #383 and #441; #378 closed unmerged. A bounded
  fixture is not proof of closure, and this audit did not establish a current reproduction.
- [ ] Retain Moshi image recap cards as blocked on transport, not ready to build. The recorded reopening
  conditions are a homelab HTTPS image host, an upstream upload interface, or a documented data-URL path.
  An operator-approved single-card probe must establish actual image display before treating data URLs as
  supported. Revisit usefulness before adding a renderer; the proposed recap duplicates Discord. Source:
  `~/.claude/pipeline/slices/design-moshi-image-cards.md`.
- [ ] Preserve the pns refactor plan's explicitly carried-forward behavior work (section 7). B1 needs a
  reviewed Hue bridge certificate/identity-pinning design; `pns/crates/pns-adapters/src/hue/bridge.rs`
  still disables certificate verification. Define enrollment, changed-certificate handling and recovery
  before changing that behavior.
- [ ] Resolve the related B6/B20/B39 hook design: the answered-wait race, when `AskUserQuestion` should
  arm a waiting indicator and what its notification contains, and alerts for sandbox network approval
  requests. The `AskUserQuestion`-specific `asked` wiring runs after the tool completes, and network
  permission waits remain explicitly uncovered. Inspect current harness events and agree behavior before
  changing hooks; a prompt-only guess does not establish an actual permission wait.
- [ ] Implement B18's decided behavior (2026-09-12): pause persistent agent-status lighting during
  `pns quiet` and macOS Focus. Pause the status effects, not ordinary room lighting. Preserve the settled
  security-banner and phone-alert mute bypass. Verify quiet/Focus transitions, including an effect
  already active when muting begins. B19/B25's nag tolerance and future-timestamp handling still need
  explicit disposition against current source; their conditional proposals are not automatic
  implementation work. Local implementation `cf7d4866` on `feat/pns-status-quiet` passed 16 focused
  checks, including eleven new cases, three mutation checks and package gates. Independent review passed
  17 checks. PR #536 contains this change and P4; combined full checks and the release build passed.
  Required checks passed and #536 merged. Operator deployment and actual lamp/Focus acceptance remain
  open.
- [x] Isolate pns color-selection tests from the invoking shell's environment. On 2026-09-13, `just ship`
  failed `a_terminal_with_nothing_asking_otherwise_is_painted` with `NO_COLOR=1` inherited from the agent
  session. The exact test passed after unsetting `NO_COLOR` and `REPORT_LIB_PLAIN`. Production correctly
  honors these variables; make the test's assumed environment explicit without changing that behavior.
  The #530 verification uses a clean environment; this test repair is separate. Track it in
  [6hVqqxHGq35Fq5Hv](https://app.todoist.com/app/task/6hVqqxHGq35Fq5Hv). Implemented locally in
  `186c7ee5` on `fix/pns-test-isolation`; independent review approved it. All 16 style tests passed with
  both disabling variables inherited. Seven private terminal comparisons against the previous binary
  produced identical output and exit status. Merged in #533; full checks passed and local main contains
  it.
- [x] Reconcile B74's concurrent Cargo/lint failure against current source: reproduce the disappearing
  `rmeta` error, identify the failing stage, then close or fix it. The historical extra `target/`
  exclusion was measured ineffective and must not be proposed again without new evidence. The 2026-09-13
  private investigation confirmed that chezmoi scans build output while collecting template data, before
  evaluating the selected template. Current treefmt failed three amplified rename races, and one
  concurrent private `cargo doc` run failed on a disappearing search-index path. Idle and outside-source
  build controls passed. The owned-renderer fix is committed as `80cfa0bf` on `fix/b74-render-context`:
  `scripts/treefmt/lib-render-context.sh` renders every formatter through a shallow symlink view of the
  checkout, so chezmoi's pre-template walk never enters a build directory, while real source data,
  partials, build hashes and render-error propagation are pinned by the five cases in
  `test/unit/formatter-render-context.test.sh` (5 passed, 20 assertions, 921 ms). After merging `main`
  (`8f50e926`), `just ship` passed (exit 0, 2m27s) and
  [PR #548](https://github.com/webdavis/dotfiles/pull/548) was opened on 2026-09-13. Independent review
  returned four findings, with the render-context mechanism verified against real chezmoi 2.72.1. SEV-2:
  the espanso formatter test has no hostile fixture, so reverting its command leaves every test green.
  Three SEV-3 findings: the throwaway `HOME` is never removed, three redundant chezmoi flags, and a dead
  `home` fixture with no `tear_down`. All four fixed test-first with mutation checks (`f8811c3b`,
  `8a243f4d`, `25c719a3`, and the tear_down commit; the refusal classifier lives in
  `shellcheck-rendered-template.sh`, not the espanso formatter, and `--config` had to stay because it
  injects the real `sourceDir`), and the PR merged 2026-09-14 (`152dfdf7`); its worktree is removed.
  Evidence: `/private/tmp/dotfiles-modernization/b74-hiiey5u7/RESULTS.md`. B75's Rustdoc link fixes
  already shipped in `20a0c245`; keep them closed.
- [x] Correct the owning `webdavis/pns.nvim` repository's provisional minimum-version documentation and
  default after checking its actual requirements. Commit `e77799f` corrects the default and docs to
  `0.1.0`; its new default-health check failed before the correction, then all 38 checks passed.
  Independent review repeated the suite, and formatting and Lua lint passed. Owning-repository PR #1 is
  merged at `ed51fd5`, with its clean local main fast-forwarded. GitHub has no configured checks for that
  repository. The existing consumer override already supplies this minimum, so no pin or live
  configuration change is needed. Task 14 remains complete.

### Neovim review follow-up

Read-only acceptance audit, 2026-09-13: Neovim 0.12.5 and all 93 source, deployed and installed plugin
pins agree. All 380 Lua checks passed with private socket fixtures. Source and installed copies each
passed five silent starts, and seven language servers attached to private projects, including Swift and
clangd. Full plugin health captured 40 sections, with documented exceptions and fixture artifacts kept
separate from defects. Synthetic warm startup measured 94.37 ms against a historical 164.18 ms, but the
quiescent and cold conditions were not met. This is advisory evidence. Rendered keys, real agent loops,
complete Swift/custom-plugin interaction and fresh-home/repeat applies remain operator acceptance. The
deployed Overseer template has a different filename with identical contents; reconcile that during
operator deployment. No source correction was warranted by this audit.

- [ ] Finish [6hR57XgFJxrgVFVM](https://app.todoist.com/app/task/6hR57XgFJxrgVFVM), the remaining
  nvim-mcp review. `pane_socket.lua` and `executable_nvim-mcp-connect.sh` validate the final runtime
  directory but leave replaceable ancestors unchecked. The resolver's `answers()` follows socket
  symlinks, and newline-containing runtime paths are accepted by the listener but split inconsistently
  during discovery. Source `bffa979a` and `e8bd14ac` fix replaceable ancestors, shared listener/resolver
  path validation, socket symlinks and newline pins. Independent review approved 23 listener cases, 51
  resolver cases, eight private socket drills and eight additional native path checks. Full `just ship`
  passed after integrating current main, and [PR #546](https://github.com/webdavis/dotfiles/pull/546) is
  open with its lint check passed. Deploy both files together, verify a fresh harness connection, and
  recheck the outstanding quiescent timing claim. Private checks do not establish live editor,
  second-account or access-control-list acceptance.
- [ ] Resolve B103's same-workspace pane-move routing bug. The current integration validates workspace
  identity, while the agent resolver still uses the old `HERDR_TAB_ID`; the isolated review reproduction
  selected the old tab's agent. The cross-workspace refusal in `4c06b8ca` does not fix this case. Use
  supported Herdr interfaces and owned integration code; do not patch the third-party plugin. Commit
  `dfe28fd3` passed independent review with 94 private checks; full `just ship` and required continuous
  integration passed. [PR #543](https://github.com/webdavis/dotfiles/pull/543) merged and local main
  contains it. Operator deployment and live pane-move acceptance remain.
- [ ] Resolve B97's Zig tooling decision: supply a working, compatible Zig/ZLS pair or remove the unused
  ZLS configuration after that decision. At audit time Zig reported `0.12.0-dev.3158+1e67f5021`, Mason
  ZLS reported `0.15.1`, and `zig env` failed to locate its installation. The Zig neotest adapter is also
  absent; decide whether that language workflow is wanted before adding it.
- [ ] Preserve B107's deferred JavaScript test-discovery responsiveness work. Caching shipped, but cold
  parsing remains synchronous; the recorded 7.4-second UI stall was not remeasured in this audit. Source
  for B97/B103/B107: `~/.claude/pipeline/backlog-consolidated-2026-09-02.md`. B92's canonical-hour,
  rainbow and X11 colour cycles were deliberately excluded, not missed implementation.
- [ ] Preserve B95's Rust neotest discovery and duplicate-client follow-up. Rust, Java and Elixir
  adapters are absent from the configured adapter list; verify Rust's intended workflow before calling
  language coverage complete. Record the Java/Elixir disposition against plan task 46b, step 2, which
  permits withholding adapters that fail their verification. Their absence alone is not an instruction to
  add them. Existing neotest infrastructure and other language adapters remain implemented.
- [ ] Reconcile B96's first-use parser readiness. Go is omitted from the preinstalled parser list,
  missing-parser installation is asynchronous, and the Go adapter returns without discovery when its
  parser is absent. Verify the first test request in that state and provide a working first-use path
  through supported integration. The historical failure was not reproduced during this audit.
- [ ] Correct B100's stale pane-selection contract in the canonical Neovim spec/plan: the owned helper
  uses `agent_pane(on_pane)`, not a synchronous returned pane identifier. Preserve cancellation/refusal
  behavior. This is documentation reconciliation, not a missing helper implementation. On 2026-09-13
  `3d92ca3e` on `docs/nvim-agent-pane-contract` rewrote the spec's 7.2 lookup paragraph, its 7.4
  interface bullet and the plan's PR 11 interface line against `M.agent_pane` in
  `dot_config/nvim/lua/custom_api/herdr.lua`; `just ship` passed on the second run (exit 0, 3m12s; the
  first run hit the pns `security_sound` fixture's 900 ms child guard under load, fixed separately) and
  [PR #556](https://github.com/webdavis/dotfiles/pull/556) merged into `main` on 2026-09-13.

### Recover the remaining design from PR #24

- [x] Review #24 before deciding its disposition. Recovered on 2026-09-13 from head `2202dcbf`; four
  historical documents and thirteen upstream snapshots passed all 17 recorded hash checks. Retain its
  independent alert, restricted evidence and advisory-only requirements in the section below. Supersede
  the digest trigger and obsolete recipes in a smaller reviewed plan after the security decisions are
  settled. #24 remains open; do not merge its old instructions unchanged.
- [ ] Reconcile the approval interface separately: Butters tap-to-approve scoped to pending findings and
  the `/osquery allow|deny|list` Hermes skill. Verify the current posture command and trust contracts;
  investigation must not grant the analyst approval authority.
- [ ] Reconcile the June hardening-plan remainder and explicitly deferred FleetDM, beaconing and Wazuh
  research. Record accepted scope before implementation. Issue #18's FileVault fix and dead snapshot
  handling are already present in the current query, Bash and Rust paths; reconcile/close its stale issue
  rather than reopen that implementation. PR #19 is closed and was not merged.
- [ ] Give the June hardening requirements explicit dispositions: signature-chain verification,
  interpreter-payload assessment and per-run grouping of repeated findings about the same subject.
  Current Rust code reads signature metadata, leaves interpreter payloads unverified and retains each
  finding in a batch. Porting the existing behavior did not implement these proposed changes.
- [ ] Preserve deferred install-state kernel-extension monitoring and off-host machine-death detection.
  The former needs reconciliation with July's decision to alert on untrusted loaded extensions; do not
  restore June's obsolete delivery block. The latter needs an external host and remains homelab scope: a
  watchdog on the monitored Mac cannot detect that Mac disappearing from outside it. Keep intentional
  log-only `es_launchd_writes` handling and accepted residual risks out of the implementation queue.

### Hermes security investigation, recovered from #24

Operator clarification, 2026-09-12: the sandboxed investigator belongs to Hermes. Posture produces
security findings; Hermes owns the downstream investigation and advisory reply. This workflow should
accept security alerts from other producers through the same supported alert contract. It is configured
through dotfiles, without embedding agent orchestration in posture or modifying third-party Hermes code.

Trigger decision, 2026-09-12: investigate Critical alerts only. Deliver the original alert immediately,
then publish the investigation as a separate advisory. Daily digests do not trigger this workflow.

Supported-interface review, 2026-09-13: installed Hermes `a4091e49` and upstream `b6b53c69` differ.
Docker contains tool execution, not the host controller or every plugin. Container reuse, automatic
mounts and credential forwarding need explicit restrictions. Proxy environment variables alone do not
enforce an outbound-network allowlist. Upstream has a separate `kanban attach` command; the old
`kanban create --attach` recipe is invalid, and ordinary attachment reads do not replace a bounded,
no-follow evidence collector. A named route selects one destination rather than broadcasting to two.
Transport deduplication needs distinct delivery-attempt identifiers under one alert correlation key.

Implementation remains blocked on the execution/network boundary, permitted evidence and model-provider
disclosure, credentials, and enforceable advisory limits. Prefer networkless evidence execution and a
separate trusted publisher; the operator must settle that security boundary. A constrained result
validator can enforce allowed fields and actions, while a prompt cannot guarantee semantic limits on
unrestricted advice. Verify reply correlation before wiring delivery. Preserve the original message and
failure notice independently. No investigator, raw completion publisher or evidence upload was enabled.

Current references:
[Hermes configuration at the reviewed revision](https://github.com/NousResearch/hermes-agent/blob/b6b53c69a6ed49cb099cf1bfe76b5e6edd718e5a/website/docs/user-guide/configuration.md),
[worker launcher](https://github.com/NousResearch/hermes-agent/blob/b6b53c69a6ed49cb099cf1bfe76b5e6edd718e5a/hermes_cli/kanban_db_dispatch.py),
and
[completion delivery](https://github.com/NousResearch/hermes-agent/blob/b6b53c69a6ed49cb099cf1bfe76b5e6edd718e5a/gateway/kanban_watchers.py).

The original documents are on #24's `docs/osquery-design` branch, not in current main:

- [Original analysis-agent design](https://github.com/webdavis/dotfiles/blob/docs/osquery-design/docs/superpowers/specs/2026-06-03-osquery-analysis-agent-design.md),
  especially sections 2, 4 to 8 and 11.

- [Original implementation plan](https://github.com/webdavis/dotfiles/blob/docs/osquery-design/docs/superpowers/plans/2026-06-03-osquery-analysis-agent.md),
  including the recorded immediate-alert/separate-advisory decision and failure notification.

- [Later decision addendum, D-V2-12](https://github.com/webdavis/dotfiles/blob/docs/osquery-design/docs/superpowers/decisions/2026-06-10-osquery-alerting-v2-decision-addendum.md),
  which instead proposes a read-only advisory over the deterministic daily digest. The v2 master spec's
  section 13 repeats that narrower digest scope.

- [ ] Reconcile the original documents with the Critical-alert-only decision before implementation.
  Supersede their daily-digest investigation scope. Keep this as a Hermes-owned workflow, with
  source-specific facts supplied by security producers and immediate original alert delivery.

- [ ] Preserve immediate deterministic alert delivery. Hermes then starts a dedicated, ephemeral
  investigator over a bounded copy of the supplied evidence and publishes a separate advisory associated
  with the alert. Failure or timeout must be distinguishable from an all-clear. The investigator cannot
  suppress, delay or rewrite the original alert, approve trust, or perform remediation. Verify the
  current correlation and evidence-transfer contract before wiring it. The recorded advisory contract
  permits adding concern or explanation but forbids clearing the original finding, and limits suggested
  responses to a fixed vocabulary. Verify those limits outside the prompt before promising enforcement.

- [ ] Define the required alert/evidence metadata through the existing delivery path. The current pns
  webhook forwards `agent`, `state`, `project`, `detail` and `request_id`; it does not forward the
  producer's security class or a structured artifact reference. Verify classification and evidence
  references before attaching an investigator, rather than inferring them from rendered prose. Also
  reconcile route names: native posture names `posture`, while the tracked route checker covers `pns` and
  `unattended-upgrades` as delivery-only. The encrypted route configuration was not inspected in this
  review. Any producer/transport changes should carry data; Hermes owns the investigation.

- [ ] Revalidate the old Docker/profile, trigger, network and artifact-copy assumptions against supported
  Hermes interfaces. Preserve restricted host access and outbound connectivity, no host secrets, and
  untrusted evidence handling. The old plan includes unverified flags and prompt-based output checks;
  those do not establish sandbox isolation or enforce the promised output limits. Review the actual
  controls and their acceptance checks before building. If supported integration cannot provide them,
  report that gap instead of patching Hermes. Use the existing research at
  `~/Documents/Sandboxed_Agent_Prompt_Injection_Research_20260603/report.md` as a review input; it is not
  a missing research assignment.

The posture port plan's section 8 summary still says its fourth decision is waiting on the operator, but
decision 4 itself records the security mute bypass as settled on 2026-09-06. Correct that stale summary
during plan reconciliation; do not reopen the settled security-page behavior.

## Start gates and retained deferrals

On 2026-09-13, the operator authorized the active modernization goal. After task 68c, proceed through the
resume order, including the Herdr process-toggle plugin and review launcher, SP5 research, SP4 Bash
improvements, SP8 and Forzare/#51 when their prerequisites pass. Those projects need no further start
instruction. Preserve Neovim acceptance and Herdr feasibility dependencies, SP5 before SP4, SP8 last in
modernization and Forzare after everything else. Separate future-platform/presence and extraction gates,
other explicit deferrals, unresolved product/security decisions and operator-only actions remain in
force.

- [ ] Build a persistent process-toggle plugin for Herdr in Rust, following `$clean-code-rust` and its
  prerequisite `$clean-code`. Consult `$frontend-design:frontend-design` for terminal interface design
  and review. Behavior agreed 2026-09-12; the 2026-09-13 goal authorizes implementation in the resume
  order. Support any number of named window configurations and running sessions, with no hardcoded cap.
  Each configuration supplies a program, arguments, working directory, floating dimensions, and
  independent shortcuts for Split right, Split below, and Toggle float. tuicr, reviewr, btop, and a
  scratch shell are example configurations; the plugin stays independent of the program. Split right
  opens side by side with a vertical divider; Split below stacks panes with a horizontal divider. Either
  split action starts, docks, or repositions the same session, or focuses it when already placed
  correctly. Toggle float starts a floating session, pops out an existing split, or hides/restores an
  existing float. Pop out/dock preserves the running program, terminal screen, position, comments, and
  unfinished input. Popping out releases the old split's space; hiding a float keeps it hidden until
  restored or explicitly docked. The plugin owns background session lifetime and forwards input and
  resize events through the attached view. Shortcuts work from Neovim and shell panes and while a float
  is focused. Show one floating window at a time: selecting another hides the previous float without
  stopping either session. Docked sessions remain visible. Stacking floating windows is outside the
  agreed scope. Center floats over the whole Herdr window, spanning underlying panes, with configurable
  percentage dimensions that resize and recenter when the terminal window changes size. A hidden session
  receives the current dimensions when restored. Keep toggling and docking fast. Per window, let users
  configure whether Ctrl+C in the popup terminates its process or only hides the popup and keeps the
  background session alive. The hide action must not forward Ctrl+C to the running program. Also expose a
  separately configurable Herdr kill binding for each named session, closing its view and terminating its
  owned processes whether floating, docked, or hidden. Hiding preserves the session; quitting or killing
  ends it. Process exit closes its view and clears its session, including exits while hidden. The next
  launch starts a fresh instance. Verify attachment, redraw, resizing, focus, configurable Ctrl+C
  behavior, explicit termination, and process-exit cleanup through supported Herdr interfaces before
  building the review launcher below. The 2026-09-13 feasibility audit found a supported implementation
  path in Herdr 0.9.0: percentage popups center and resize over the shared pane surface, spanning its
  panes while excluding sidebar and tab-bar chrome. The owned Rust attachment must handle configured
  shortcuts while focused because popup input bypasses native Herdr binding dispatch. Read the same
  configured prefix and plugin actions, preserve unmatched input and paste, and reject ambiguous
  encodings. Keep the process in an owned pseudoterminal and replace its views. Hide by ending the owned
  attachment, never by blindly closing whichever popup is active. Prove view identity, redraw, transition
  rollback and process cleanup with fixtures before runtime acceptance. These are implementation
  requirements; no mandatory upstream change was found. Operator note 2026-09-14: herdr's documented
  `[[keys.command]]` popups are NOT this (a popup lives only until its command exits; no toggle, hide or
  float exists in the docs, the keybinding actions or the CLI, which offers zoom, split, move, swap and
  close). When this is built, dig into herdr's source for a true hide before settling for parking the
  pane in another tab, and check herdr's preview channel (its nightly, more or less) for a hide or float
  primitive that the stable release lacks.
- [ ] Add a deterministic worktree picker and reviewr launcher. Consult
  `$frontend-design:frontend-design` for the picker's interface design and review. Implementation is
  authorized by the 2026-09-13 goal after the process-toggle feasibility checks pass. From the current
  repository, list existing worktrees with the most recently updated first, including commits and
  uncommitted file edits while excluding ignored files. Search and select a worktree without changing the
  agent's working directory or branch. Agents may remain on main while orchestrating multiple worktrees;
  selection needs no language-model call or agent-to-worktree registry. Keep worktree selection separate
  from the generic process-toggle plugin; the picker can be an ordinary command rather than another
  required plugin package. Use the shared session behavior above for Split right, Split below, and Toggle
  float. With no selected target, open the picker first; subsequent toggles resume that review without
  repeating discovery. Preserve the originating workspace context so reviewr can send comments to the
  intended agent, including its agent picker when several agents are present. Opening the picker and
  resuming a review must feel immediate with hundreds of worktrees. Measure first-load, repeat-load, and
  review-start latency separately and agree a budget before implementation. Evaluate cached activity
  ordering refreshed separately from display, keeping the selection stable during refresh; resolve
  deletion timestamps, stale paths, and cache freshness before claiming accurate recency. Reuse the
  process-toggle session handling after its feasibility checks pass; do not patch reviewr or duplicate
  that handling here. The requested upstream proposal already exists as
  [reviewr issue #99](https://github.com/persiyanov/herdr-reviewr/issues/99); check it when revisiting
  supported placement actions.
- [ ] SP5, evaluate xonsh before SP4's shell work, after Neovim acceptance. The 2026-09-13 goal supplies
  the start authorization. The 2026-08-28 Todoist decision supersedes the old roadmap's SP5 Thaw label.
  Research cold/warm startup against Bash, atuin, the shell hooks, starship, direnv, zoxide, carapace,
  Herdr attachment and the existing key chords. Produce a go/no-go and reviewed spec. A migration has not
  been approved, and scripts remain Bash.
- [ ] SP4, improve the interactive shell after the SP5 verdict. Start from the existing Bash plan: alias
  consolidation and one binding table that drives both key bindings and an fzf menu to view, select and
  run them. Revisit the existing candidates for directory, stash, process, worktree and Herdr workspace
  selection without treating every candidate as adopted. Reconcile the old Charm-tools evaluation with
  the now-adopted gum output style, and obsolete declaration-testing requirements with today's
  behavior-only test policy. Preserve the ratified Nushell no-go; do not assume xonsh adoption from its
  evaluation task.
- [ ] Future pns platforms and presence: Linux/homelab
  [#192](https://github.com/webdavis/dotfiles/issues/192), iOS companion
  [#193](https://github.com/webdavis/dotfiles/issues/193), Android
  [#194](https://github.com/webdavis/dotfiles/issues/194), and the Todoist millimeter-wave Home Assistant
  presence backend after the existing Hue presence work. Retain their prerequisite and design gates. The
  Linux move also requires the approved age identity, recipient and credential-source design from the
  roadmap, plus its recovery/rotation runbook. Extraction issue
  [#195](https://github.com/webdavis/dotfiles/issues/195) is covered by 68a's hand-rewrite/v1 gate.
- [ ] SP7, sweep and backlog reconciliation, detailed below. Finish the earlier approved work first.
- [ ] Revisit Neovim's explicitly deferred conform.nvim/nvim-lint migration and Agent Client Protocol
  option only after a separate decision, using the v4 design. Replacing noice with private
  `vim._core.ui2` was excluded; it is not an approved follow-up.
- [ ] Revisit the babysitter execute-bit issue draft in `~/.claude/pipeline/babysitter-issues/` when the
  operator releases its 2026-09-06 deferral.
- [ ] Reconcile babysitter's separate Stop-decision defect against a released artifact and rerun the
  recorded A/B reproducer before lifting that hold. B112 records upstream issue #1761 and merged fix
  #1777; do not file a duplicate or treat a merged source fix as an installed release. Source pins the
  SDK (software development kit) to `6.0.3` because the npm `latest` tag is behind it. Preserve the
  separate execute-bit hold too: the current Claude and Codex templates require both defects resolved
  before normal enablement. Reconcile that condition with the operator before changing either template.

### SP7 scope recovered from the roadmap and Todoist

- [ ] Reconcile the package-manager audit (#11), npm/uv cleanup drift (#20), Homebrew rollback (#12),
  remaining macOS settings (#17), and optional shell command generation (#91). Review the existing
  installers and uu producers before proposing additions. Keep package removal and rollback decisions
  explicit; installation tracking does not authorize removal of all undeclared packages.
- [ ] Reconcile existing Nix installer maintenance (#10) only as needed to preserve optional Nix package
  and project-flake use. This is separate from the rejected nix-darwin macOS-management transition.
- [ ] Revisit the roadmap's bandwhich/doggo/ouch evaluation, remaining shell quick wins and optional Tart
  clean-machine environment. `MANPAGER` and Git's `autocorrect = prompt` already exist. VM creation
  remains operator-gated. Re-rule the old documentation/archive tasks S1/S2/S4 against current files.
- [ ] Finish the recorded pi harness setup/evaluation and its configuration, skills and hook integration.
  Reconcile babysitter's evaluation with its existing declaration and holds, and Understand-Anything with
  its current adoption decision. OpenSpec and credential-access work have explicit entries below.
- [ ] Finish OpenSpec configuration, explicitly requested 2026-09-12. Its npm package is declared and
  version `1.12.0` is installed. The live global configuration exists with profile `core` and delivery
  `both`, but no OpenSpec configuration or generated integrations are tracked here, and this checkout has
  no `openspec/` root. Choose the intended global settings and project scope, then configure the
  supported harness integrations without overwriting existing instruction files. Track reusable
  configuration in dotfiles and project specifications in their owning repositories. The weekly uu npm
  lane upgrades the CLI; define when `openspec update` refreshes generated project instructions
  separately. Continue [6hPCF8hrCrjjWv2M](https://app.todoist.com/app/task/6hPCF8hrCrjjWv2M), promoting
  its old evaluation-only scope to the requested setup work.
- [ ] Configure [YNAB (You Need a Budget)](https://github.com/oliverames/ynab-mcp-server) through its MCP
  (Model Context Protocol) server, added by the operator on 2026-09-12. Track the upstream npm package
  `@oliverames/mcp-server-for-ynab` in the existing fnm package declaration and use its local stdio
  binary from managed harness configuration. The existing weekly uu npm lane covers global package
  upgrades; no new producer is needed. Select the intended harnesses and Hermes profiles during setup.
  Obtain the personal access token through the existing KeePassXC-backed secret configuration, never a
  committed value. Preserve the direct server's read-only default unless writes are explicitly requested;
  upstream plugin manifests enable writes, so do not copy those defaults blindly. Verify tool discovery
  and an authenticated read after setup. Track it in
  [6hVpJxQgR7f73xmM](https://app.todoist.com/app/task/6hVpJxQgR7f73xmM). This entry schedules
  configuration; nothing was installed in the audit.
- [ ] Reconcile [Backpass](https://github.com/kunchenguid/backpass) configuration and finish any missing
  integration, requested 2026-09-12. It is installed, declared in npm, and
  `dot_config/backpass/config.json` matches the deployed copy, directing user instruction edits to
  `.chezmoitemplates/global-agent-rules.md`. Preserve that source ownership and the interactive review of
  proposed edits. Decide how accepted skill extractions enter the managed store and provenance lock,
  instead of leaving undeclared directories. CLI upgrades already use uu's npm lane; do not reinstall
  merely to complete the old evaluation task.
- [ ] Plan installation and configuration of [no-mistakes](https://github.com/kunchenguid/no-mistakes)
  and [firstmate](https://github.com/kunchenguid/firstmate), requested 2026-09-12. Neither has a managed
  declaration in the reviewed source. The operator confirmed that Backpass, no-mistakes and firstmate all
  refer to `kunchenguid`'s repositories. Verify any existing checkout before installing. Firstmate is a
  repository-based agent distribution, not a standalone CLI; configure its Herdr backend and review its
  treehouse dependency alongside the existing worktree workflow. Select harnesses, project roots,
  validation commands and review/merge authority explicitly. Preserve existing hooks and per-invocation
  destructive-action approvals when configuring no-mistakes' Git proxy and repair behavior. Track tools,
  upstream skills and their update paths through existing uu lanes or a producer where needed. Reuse
  [6hPV483GJgGHX95M](https://app.todoist.com/app/task/6hPV483GJgGHX95M) for these and Backpass.
- [x] Recheck [claude-code-owasp](https://github.com/agamm/claude-code-owasp), requested 2026-09-12.
  `owasp-security` already has the correct upstream in `npxTracked`, an on-demand tier, the Claude
  symlink and invocation override, and a deployed shared-store skill with its reference files. Codex
  scans that store; uu's skills lane refreshes it. No duplicate install is needed. `hermesProfiles` is
  currently empty; extending its delivery to Hermes needs a profile choice during configuration review.
- [x] Implement and merge [Plannotator](https://github.com/backnotprop/plannotator) setup in #530 for
  Claude Code, Codex, Gemini and Hermes, including nicodemus. Merged and applied on 2026-09-13;
  interactive acceptance remains in the review-tools section above. Plannotator 0.27.14 is installed via
  the upstream minimal installer. A weekly uu command producer reruns that installer; Claude's plugin,
  shared skills and Herdr plugins retain their existing update lanes. Codex and Gemini hook mergers
  preserve unrelated settings and hooks. Codex trust remains operator-owned. Gemini retains its native
  final plan confirmation, including after hook failures or timeouts. Hermes has no native automatic
  browser-plan hook; it receives the command/skill workflow and Herdr document/reply review.
  `plannotator`, `plannotator-annotate` and `plannotator-review` come from the upstream core skill
  subdirectory; `plannotator-tui` comes from Herdr Annotate. All four are on-demand, tracked by uu and
  declared for Hermes default, butters, concerned, elaine and nicodemus. Codex and Gemini discover the
  shared store natively. Claude retains its own plugin commands and receives the terminal skill. The
  browser's last-reply skill is excluded from the shared store because unsupported harnesses can fall
  back to Claude transcripts; use Herdr's reply action. Pi and OpenCode had configuration directories but
  no installed executables in this audit; no new harness was added. Full
  [Herdr Annotate](https://github.com/plannotator/herdr-annotate) is installed and tracked through the
  shared installer/update roster. Its terminal review executable reports 0.8.0 and the managed launcher
  resolves its enabled plugin root dynamically. Terminal defaults remain upstream's.
  `$frontend-design:frontend-design` was consulted for the interface/configuration review. Bindings
  approved 2026-09-13: `prefix+a` capture, `prefix+shift+a` recent agent replies, `prefix+f` documents,
  `prefix+m` annotation manager, `prefix+y` copy annotations and `prefix+shift+y` copy and archive. Move
  herdr-nvim to `prefix+e` toggle and `prefix+shift+e` pick a file, with native scrollback editing on
  `prefix+alt+e`. Move reviewr to `prefix+v`; move quota settings/refresh to `prefix+u` /
  `prefix+shift+u`, preserving native configuration reload on `prefix+shift+r`. Worktrunk uses `prefix+w`
  for its default-branch picker, `prefix+shift+w` for the current-branch picker, `prefix+shift+g` to
  include remote branches, `prefix+backspace` to remove and `prefix+shift+m` to merge. Move the native
  workspace picker to `prefix+alt+space` and workspace rename to `prefix+alt+comma`. The operator
  selected this Worktrunk layout with `prefix+shift+g` instead of the proposed `prefix+ctrl+w`; the
  earlier Alt-based Worktrunk group was rejected. Preserve native worktree shortcuts and the other
  navigation and split bindings. The combined mapping was checked against the live and PR configurations
  plus Herdr 0.9.0 defaults; no duplicate assignments remain after these moves. The configuration is
  merged and the operator reported a successful apply. Verify permissions, feedback delivery, placement
  and plugin updates before acceptance. Obsidian saving remains optional; this integration does not
  replace tuicr, reviewr or the planned process-toggle plugin. Implementation task:
  [6hVpPJJ7662Qr9mM](https://app.todoist.com/app/task/6hVpPJJ7662Qr9mM).
- [ ] Install and configure [Dockerfile Roast](https://github.com/immanuwell/dockerfile-roast), both the
  `droast` CLI and [droast.nvim](https://github.com/immanuwell/droast.nvim), requested 2026-09-12. Use
  the supported Homebrew package after verifying availability, and manage the upstream Neovim extension
  through the existing lazy.nvim configuration and lock. It already runs the CLI and publishes native
  diagnostics, so a second none-ls adapter is unnecessary. Preserve Docker language-server support, and
  review overlap with the existing none-ls hadolint diagnostics before changing either linter. Verify
  lint-on-save and command-driven diagnostics using a representative Dockerfile. Track CLI and plugin
  upgrades through the existing uu Homebrew and Neovim lanes. Track it in
  [6hVpPJWQq79vMM3v](https://app.todoist.com/app/task/6hVpPJWQq79vMM3v).
- [x] Review native Neovim language-server configuration, requested 2026-09-12. Installed Neovim is
  `0.12.5`; `dot_config/nvim/lua/plugins/lsp.lua` already uses `vim.lsp.config()` and `vim.lsp.enable()`,
  with no legacy `require("lspconfig").SERVER.setup()` calls. Keep nvim-lspconfig for maintained server
  definitions and Mason/mason-lspconfig for installation and native activation.
  [Upstream guidance](https://github.com/neovim/nvim-lspconfig#readme) deprecates the old setup
  framework, not the definitions plugin. No migration is needed. none-ls and lsp-format still provide
  external linting/formatting; their separately deferred replacement is not a prerequisite for droast.
- [ ] Reconcile the remaining tool evaluations for strix, apple/container, minutes and gnhf. Check prior
  removals and rejections before proposing adoption. Herdr remains the selected multiplexer. The rejected
  git-absorb and deferred gh-dash/companion tools are not additions to #530.
- [ ] Review the still-open bqf, dadbod/dadbod-ui and dblab/database-workflow tasks, including the older
  PostgreSQL shell/client configuration task and SSH `Host *` client-hardening task. Source declares
  PostgreSQL, but that alone does not supply the requested client configuration. bqf and dadbod are
  absent from the current source/live plugin locks. Avoid duplicating completed xcodebuild, dap and
  neotest infrastructure; the language-specific gaps above remain separate.
- [ ] Reconcile completed or superseded Todoist review items with evidence: Neotest parser findings
  (`6hR5vFgFXgjHVGMv`, `6hRHmWWqFjr5RmVv`), Atlas (`6hR59h6FQWpv33p8`), Overseer (`6hR5Gg6XXRFVhfpg`) and
  the pns builder-input fix (`6hV7jMW3jMGWcCMv`, commit `0b55db04`). Current source contains their fixes
  or recorded replacement decisions. Preserve Atlas's deliberate omission of checkout mappings. Update
  the remaining stale acceptance claims rather than reinstalling. B111's temporary-repository fsmonitor
  exclusions and fixture fix already shipped; the proposed worktree reaper was rejected. Likewise, the
  old graphify exclusion-removal proposal predates the current committed `graphify-out/graph.json` and
  post-commit rebuilding policy; do not delete that map or its exclusions as unfinished cleanup.
- [ ] Reconcile [6ggcw4qfqfqjxP3v](https://app.todoist.com/app/task/6ggcw4qfqfqjxP3v), the old tiling
  window-manager task. Issue #14 is already closed as completed (2025-12-28); AeroSpace is declared and
  configured. This needs task-list closure with that evidence, not another installation or issue closure.
- [ ] Reconcile stale GitHub issues #8 (Kulala-LS is declared), #9 (gh-notify was superseded), #13
  (Zellij predates the Herdr decision), and #18 (fixed), then align the surviving issues and Todoist
  items with this file. The earlier migration's cutover ledger has all five completion markers dated
  2026-08-10; do not confuse those completed gates with the new posture cutovers.
- [ ] Resolve scope for the older warden import/quarterly-cleanup tasks and obsolete agent-session
  restoration tasks. The restic script is the operator's learning exercise; keep its later LaunchAgent
  dependent on that work and do not take over writing it without a new instruction. These older tasks
  require disposition, not automatic inclusion in the active implementation queue.

### Safe agent credentials and Infisical client integration

Operator decisions, 2026-09-12: self-host Infisical for selected agent credentials. Personal passwords
remain in the KeePass database used by KeePassXC and Strongbox, including iPhone AutoFill and offline
access. Infisical's server and Agent Proxy are planned in homelab `docs/plans/PLAN-v12.md`, A6,
[task 6hVpX4M8hmV4Gmr3](https://app.todoist.com/app/task/6hVpX4M8hmV4Gmr3). This is a selected service,
not an open product comparison. Continue the
[credential-access task](https://app.todoist.com/app/task/6hPCF99XjRP87GmM) for laptop integration and
the separate local credential/chezmoi boundary. Planning does not authorize credential access or applies.

- [ ] Track Infisical's client installation, endpoint/trust configuration and tool upgrades through uu on
  each development laptop. Configure scoped identities for Claude Code, Codex and Hermes, including
  nicodemus. Coordinate private connectivity with homelab A6/F4; retain the current Tailscale path until
  an approved NetBird cutover. Credential migration inherits A6's F1 platform, F2 restore and OpenBao
  ownership checks. Verify approved operations and denied access using dummy credentials before migrating
  real ones. Research supported client behavior and self-hosted feature entitlements before choosing a
  paid edition or promising the required isolation.
- [ ] Register only credentials selected for agent use in Infisical, preferably dedicated accounts or
  restricted tokens. If a personal login must exist in both stores, record rotation ownership and update
  both copies. Do not introduce automatic vault synchronization or make mobile password access depend on
  a website. Broker-dependent operations stop clearly during a home-server/network outage.
- [ ] Evaluate supported KeePassXC access only for remaining local operations, including chezmoi's
  file-rendering needs. An outbound credential proxy does not solve that boundary by itself. Distinguish
  handing an agent a password from allowing a tool to use it for an approved operation. Prefer supported
  integrations over a custom broker; Passage remains background research, not a selected migration.
- [ ] Evaluate [kpxc-cli](https://github.com/mietzen/keepassxc-cli) as an unadopted third-party
  candidate: it uses KeePassXC's browser protocol and macOS biometric unlock. Verify entry approval,
  association key protection, revocation and locked-database behavior. It is not a drop-in replacement
  for the current title/attribute-based chezmoi integration: its documented lookups use URLs, and custom
  fields have additional requirements. Do not treat Touch ID support or masked output as proof of
  isolation.
- [ ] Define and verify the actual access boundary: permitted secrets and operations, approval duration,
  revocation, audit records without secret values, and denial outside the permitted set. Account for
  arbitrary shell access, readable rendered configuration and agent-editable templates/scripts. Hiding a
  value from chat or putting it in a child environment does not keep it inaccessible to an unrestricted
  process running as the same user. Establish the isolation needed for the claimed protection.
- [ ] Include fresh-machine recovery and current secret exposure paths in that design. The age-key
  restoration script calls `keepassxc-cli` directly, outside template lookup. Moshi's pairing script
  currently places its token in command arguments. Review supported alternatives without printing the
  values or modifying upstream tools; replacing the template function alone would leave both paths
  unaddressed.
- [ ] Demonstrate any proposed chezmoi flow with dummy credentials first, including locked/denied access,
  secret-free output and a complete render/deployment/manifest cycle. The current operator-only apply
  rule remains in force until a reviewed replacement is approved. `--exclude=templates` is a retired
  workaround, not the current agent apply procedure, and must not be revived.

### Homelab plan coordination

- [ ] Keep the requested server deployments in `webdavis/homelab`: Infisical (A6), NetBird (F4), Dozzle
  (F5) and Open Notebook (L6) are recorded in `docs/plans/PLAN-v12.md` and its service matrix. Dotfiles
  owns their needed laptop configuration and managed client updates. Record actual cross-project
  dependencies without importing the full homelab deployment backlog into this modernization.
- [ ] Coordinate vpp's optional Open Notebook handoff with L6. Preserve one capture/transcription
  pipeline and canonical originals; decide the handoff format during integration design. vpp and Bob must
  not require Open Notebook merely to read or produce ordinary notes.

### vpp (Voice Processing Pipeline)

Planning addition, 2026-09-12. Build vpp in Rust to collect everyday Apple Voice Memos synced to the Mac,
preserve their original audio format, transcribe them, and produce agent notes and summaries. Track the
Mac workflow in [6hVpPJC2cjJW3V9M](https://app.todoist.com/app/task/6hVpPJC2cjJW3V9M). No ingestion or
transcription was started during this audit.

- [ ] Reconcile the existing sources before designing a second transcription system:
  `~/workspaces/Ivy/webdavis/homelab/docs/plans/PLAN-v12-experiments-backlog.md`, L-R5, records the vpp
  feature decisions and carries forward the earlier local transcription experiment. The vault's
  `CLAUDE.md` already defines `agent-processing-pipeline/raw/audio/`, `transcripts/` and `analysis/`,
  with audio excluded from Git. Homelab `PLAN-v11.md`, Phase 6, and
  [6gjGcHp69phXmXj3](https://app.todoist.com/app/task/6gjGcHp69phXmXj3) describe the broader ElevenLabs
  Scribe/whisply pipeline, Markdown transcripts, subtitle and word-timing exports, and local processing
  for sensitive audio or service outages. These are related plans; none explicitly specifies a watcher
  for Apple Voice Memos synced to macOS.
- [ ] Design automatic discovery of fully synced recordings, preserving original audio and capture
  metadata without modifying Apple's source recordings. Verify the supported macOS access/export path and
  actual audio format before choosing an ingestion mechanism. Handle interrupted sync, retries and
  repeated discovery without duplicate notes or lost audio. Keep original recordings, transcripts and
  agent analysis separately linked using the existing vault layout.
- [ ] Use redundant transcription and compare disagreements; flag uncertain text and unsupported notes
  for review, notifying through pns's producer application programming interface (API). Preserve the
  alternatives and source references. Multiple engines agreeing does not prove correctness. The proposed
  feature for playing audio from a summary sentence was rejected; original audio preservation remains.
- [ ] Support agent-suggested tags and relationships between recordings, with a defined metadata schema
  and configurable output paths. Use explicit links and deterministic filing rules for automatic routing.
  Markdown output can live in an Obsidian vault and use its existing mobile sync, but Obsidian is
  optional.
- [ ] Plan meeting briefs using relevant notes, with optional read-only calendar and Todoist inputs.
  Record Bob, the future Hermes executive assistant, as a consumer of vpp's notes and briefs. The exact
  trigger, scheduling owner, access scopes and provider choices remain under discussion. Keep source
  references and unresolved transcription issues visible to Bob and in the brief.
- [ ] Support a separate redacted draft for sharing, reviewed before release, preserving private
  originals. Choose the summary format, retention, transcription engines and local/cloud processing
  before implementation. Speaker labels and dated digests remain unapproved candidates.
- [ ] Keep vpp application code in its own project, Mac installation and service configuration in
  dotfiles, output content in the configured directory (Ivy for this operator), and homelab deployments
  in homelab. Reuse existing transcription tasks. vpp must work without Bob, Forzare or the full homelab;
  Forzare integration follows the existing post-modernization ordering.

## SP8, macOS agent workflow manager

Planned 2026-09-12. ON HOLD since 2026-09-14: the operator ruled that SP8 is not part of the current
goal, which completes when everything else in this ledger is done, and that Forzare no longer waits on
it. Nothing below starts until the operator lifts the hold. Original ordering, kept for when it does: the
final modernization subproject, after SP4, SP5 and the other modernization tasks, including the
process-toggle plugin and worktree review launcher.

- [ ] Install the upstream skills from [pstack](https://github.com/cursor/plugins/tree/main/pstack)
  through the managed skills store, recording provenance, harness delivery, and updates through uu. Check
  its Cursor-specific dependencies and compatibility with the harnesses that will build the app. Use
  pstack's setup, engineering, design, review, and verification skills throughout development.
- [ ] Design and build a macOS graphical frontend for managing agent tooling. Use
  [MoltenBase](https://www.moltenbase.com/) as the product and visual reference, matching its look and
  feel in the layout, typography, colors, spacing, and management views. Choose the application
  architecture during this subproject rather than committing to a framework now.
- [ ] Cover all installed harnesses and their profiles, including Claude Code, Codex, and Hermes profiles
  such as nicodemus. Support both user-wide configuration and project-specific configuration, with a
  clear scope selector and a view of the effective settings and their source. Discover and verify each
  harness's supported operations before implementing its management controls.
- [ ] Manage skills, Model Context Protocol (MCP) servers, harness instructions such as `CLAUDE.md` and
  `AGENTS.md`, agent memory, and plugins. Provide discovery, inspection, editing, installation, removal,
  and enable/disable controls where the owning harness supports them. Show inheritance and project
  overrides so a user can tell which settings and instructions an agent actually receives.
- [ ] Also track hooks, reusable commands, subagents, model/provider settings, permissions and trust, and
  environment/credential references. Keep credential values in the existing secret manager. Include
  search, filtering by harness and scope, provenance, installed versions, update status, connection
  health, duplicate/stale entries, and configuration drift. Show usage and last-used data where reliable
  harness records exist; distinguish unavailable data from zero usage.
- [ ] Keep local files and their existing owners authoritative. Integrate with chezmoi, the shared skills
  store and lock, and uu for installation/update workflows. Edits to managed configuration must reach its
  source rather than a deployed copy that the next apply would overwrite. Preview changes and affected
  scopes, provide backups and rollback, and preserve the operator-run chezmoi apply flow.
- [ ] Install and configure [gnhf](https://github.com/kunchenguid/gnhf), the overnight agent orchestrator
  ("each iteration makes one small, committed, documented change towards an objective"), requested by the
  operator on 2026-09-14. It is an npm CLI, so it goes on the fnm lane in
  `.chezmoidata/system_packages_autoinstall.yaml` (pinned, like the other npm tools there), with its
  `~/.gnhf/config.yml` deployed from source through chezmoi (secrets by keepassxc reference only) and
  `GNHF_TELEMETRY=0` set in the managed shell. Point it at the Claude CLI in non-interactive mode, decide
  which repository and objective it runs against first, and wire its run into the same worktree rule as
  every other agent (`herdr worktree create`). Its overnight runs are one of the four triggers for the
  Discord progress recap (see the pns recap task), so land that recap producer before the first
  unattended night.

## Late in the goal: slim the global instruction files

Operator request 2026-09-14. Ordered late on purpose, after the code lanes above and before Forzare: it
is manual, tedious, and needs the operator's judgment on every cut, so it runs as an operator-overseen
session rather than a Workflow.

- [ ] Refactor `~/.claude/CLAUDE.md` and `~/.codex/AGENTS.md` (both rendered from
  `.chezmoitemplates/global-agent-rules.md`; 251 lines rendered on 2026-09-14) by pulling knowledge out
  into dedicated skills and leaving only the rules that must load on every turn. Candidates the file
  already carries as blocks: the Work recaps layout (into the pns `work-recap` skill, leaving a
  three-line rule), the Git worktrees mechanics (a `herdr` note or the herdr skill), the pull request
  body contract (already in `~/.claude/commands/pr.md`; keep only the pointer), the shell-script and
  naming rules (a `bash-style` skill), and the backup naming scheme. Method: one section per pass, the
  operator decides keep, move or delete, every move lands in the store with its lock rows and per-harness
  delivery, and the rendered line count is recorded before and after each pass. Target: a file a new
  session reads in one screen; measure, do not guess.

## After modernization: Forzare and PR #51

The operator reaffirmed on 2026-09-12 that this executive assistant is still wanted, and on 2026-09-14
made completing it PART of the current goal: it follows all other modernization work except SP8, which is
on hold. Preserve the proposal while that work finishes; reviewing its status does not start its build.

- [ ] Reconcile and finish #51 after the rest of modernization. Its Bob executive-assistant spec and plan
  are still unmerged. Reverify July's CLI contracts, paths and harness assumptions against current tools,
  including the replacement of Bats with bashunit. Review the final documents before merging, then
  implement their approved stages with the recorded staging and go-live gates. Todoist remains the user's
  task store. Preserve the stopped-gateway activation transaction, external watchdog, rollback and
  explicit approval for real outbound communication.
- [ ] Correct #51's integration path before merging its documents. It targets `integration/modernization`
  at remote `034d9a07`, not main; its two-file review becomes 222 changed files against current main
  (`76b37ae4`). The local integration branch also has nine additional commits. Preserve that history and
  carry the two reviewed Forzare documents onto current main in an isolated branch, then review the final
  diff and explicitly supersede or update #51. Do not merge the old integration branch as a shortcut.
- [ ] Let Bob consume vpp's transcripts, metadata and briefs for meeting preparation. Keep provenance and
  unresolved transcription warnings visible; do not turn uncertain notes into confirmed commitments.
  Decide whether Bob supplies optional calendar/Todoist context or vpp reads it directly during design.
- [ ] After the canonical Forzare documents merge, replace the research folder's Phase 2 copy with a
  pointer to them. Reconcile the residue in
  `~/Documents/ADHD_Task_System_Research_20260521/REVIEW-LEDGER-2026-07-11.md`. Its post-v1 items remain
  deferred: the mutation wrapper, per-channel delivery lease, Langfuse, email/communications triage and
  extra delivery lanes. Old graphify/pre-commit complaints need current verification before reopening.

## Open questions

Walk through operator choices one at a time and record each answer beside its affected task. These are
decisions before the affected feature starts, not reasons to block all independent modernization work. Do
not ask again about settled choices such as Bash, nix-darwin exclusion, optional Obsidian support,
personal passwords staying in KeePassXC/Strongbox, or Forzare's final position in the schedule.

1. RESOLVED 2026-09-12. Hermes investigates Critical alerts only, with immediate original delivery and a
   separate advisory afterward. Daily-digest investigation is excluded.
1. RESOLVED 2026-09-12. Pause persistent agent-status lighting during `pns quiet` and macOS Focus;
   preserve the approved security-banner and phone-alert mute bypass. Resolve the separate hook
   timing/card-content choices after verifying which harness events can implement them reliably.
1. uu plugin pinning: Herdr supports `plugin install --ref`. Proposed policy, awaiting the operator:
   retain weekly automatic updates by default, with an optional explicit revision per plugin to hold it
   until unpinned. The alternative is requiring review before each plugin update.
1. Zig: maintain a working Zig/ZLS development workflow, or remove the unusable unused configuration.
   This choice gates that language's setup, not Neovim's other corrections.
1. New development integrations: select the default harness/profile audience for no-mistakes, firstmate
   and any additional OWASP delivery, subject to each upstream's supported integrations. Plannotator's
   scope is resolved: all configured harnesses, with upstream support gaps recorded during
   implementation. Existing tuicr/archify/codegraph choices already specify Claude Code, Codex and Hermes
   nicodemus.
1. YNAB: select the harnesses and Hermes profiles that need financial access. Keep read-only behavior;
   this decision does not grant writes or authorize reading credentials during planning.
1. OpenSpec and agent workflow tools: select the initial project roots. Configure global defaults without
   automatically generating files throughout every checkout. Preserve per-project instructions and the
   existing review/merge boundaries.
1. vpp: decide permitted local/cloud processing and cost before selecting redundant engines. Then settle
   whether vpp reads optional calendar/Todoist context directly or accepts it from Bob, which calendars
   and projects it may read, and requested versus scheduled briefs. Discuss these separately rather than
   treating them as one approval. Choose output layout and retention before ingestion starts.

Research can settle API compatibility, supported permission scopes, Herdr feasibility, update mechanisms
and measured performance without operator preference questions. Bring back choices that research makes
necessary, such as a paid Infisical edition or a feature that upstream interfaces cannot support. Later
feature reviews still own their configuration defaults and measured latency budgets.

Live applies, hardware checks, PR review and exact deletion approvals remain execution gates in their own
sections. Keep them separate from this decision queue; planning answers are not those approvals.

- RESOLVED 2026-09-09. posture 0.2, the pns priority-route, could not be confirmed because the hermes
  config is age-encrypted. Decrypting the source and counting route keys alone, with no value printed,
  shows `priority` and `pns` side by side, which is what the plan's step 0.3 asks for.
- The final sweep included a bounded `webdavis/pns.nvim` source/documentation check. Its minimum-version
  documentation correction is listed above; no fresh plugin runtime acceptance was performed.
