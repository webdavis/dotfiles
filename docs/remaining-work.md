# Remaining work

The open task list for the dotfiles modernization, including pns, posture, uu, lights, Neovim, terminal
review tools and the deferred subprojects. Use the resume order below; task numbers are stable
references.

Updated as tasks complete. Last updated 2026-09-15.

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

On 2026-09-15 the operator ran a full `chezmoi apply` and it passed, closing the apply gate that most of
the entries below were waiting on. The first attempt that night failed in
`.chezmoiscripts/80-bootstrap-nvim.sh`, where Mason could not install ansible-lint because `ensurepip`
crashed: Homebrew's python@3.14 3.14.7 bottle carried a `pyexpat` module that expected a newer system
libexpat than macOS 26.2 shipped, so `import pyexpat` failed with
`Symbol not found: _XML_SetAllocTrackerActivationThreshold`. The operator upgraded macOS to 27.0, after
which both `import pyexpat` and the full apply passed. `claude plugin update` does not exist on Claude
Code 2.1.272, where it falls through to a Claude self-update check, so the pns plugin moved from 0.2.0 to
0.4.0 through `claude plugin marketplace update pns`, then `claude plugin uninstall pns@pns` followed by
`claude plugin install pns@pns`; `claude plugin list` now reads `pns@pns` 0.4.0, enabled. The Neovim
dashboard's Notifications command, the `gh api` call PR #619 introduced, ran through Neovim the same day
and returned five rows at exit 0.

Merged on 2026-09-14 into 2026-09-15: #617 moved the hermes routes into a chezmoi modify template with
one KeePassXC secret per route; #618 designed the posture critical-page explainer; #619 dropped gh-notify
from the Neovim dashboard; #620 designed the pns GitHub source; #621 moved `pns-loop` and
`pns-work-recap` into the shared skills store; #622 retired the Bash alerter's dispatch library and its
drainer; #623 made posture pns-agnostic, delivering to hermes directly or through any producer command
and routing critical pages to `priority`; #624 gave pns per-route hermes keys; #625 took the `.tmpl`
suffix off that modify template, which chezmoi had been rendering as a template and then executing
(`exec format error`), and added a test that drives `chezmoi diff` over a scratch source with the real
basename; #626 renamed the routes to `pns-events`, `uu-runs` and `posture-pages` to match the Discord
channels; #627 added the seven-step lights rotation (Nightlight, Dimmed, Rest, Soho, Relax, Read and
Energize, falling back to Read); #628 dropped the `pns-recap` route so recaps post to the default route;
and #629 added five time-of-day lights presets (morning Energize, afternoon Concentrate, evening Relax,
dusk Rest on F4, night Nightlight on F7).

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
  but Claude Code runs the INSTALLED copy and re-copies it only on a version change ("already at the
  latest version (0.1.0)", cache still `loop` only; `claude plugin update` turned out not to exist on
  2.1.272, so the working sequence is `claude plugin marketplace update pns`, then
  `claude plugin uninstall pns@pns` and `claude plugin install pns@pns`), so a follow-up,
  [PR #568](https://github.com/webdavis/dotfiles/pull/568) (`chore/pns-plugin-version-bump`, merged
  2026-09-14, `3edf50dc`), bumps `plugin.json` to 0.2.0 and records the rule in
  `docs/runbooks/claude-code-settings.md`. Operator steps left: `chezmoi apply`,
  `claude plugin marketplace update pns`, `claude plugin uninstall pns@pns`,
  `claude plugin install pns@pns`, restart Claude Code; then `/pns:work-recap` exists. (2) and (3)
  remain. Schedule (1) with the next Workflow round and (2) and (3) after the posture queue (#552, #549,
  #548, #553) clears, and before gnhf's first unattended night. (2) and (3) shipped together 2026-09-14
  on `feat/pns-recap-agent` in [PR #604](https://github.com/webdavis/dotfiles/pull/604)
  (`feat(pns): add recap agent --stdin and recap git`, merged): `pns recap agent --stdin` reads the
  markdown recap an agent composed, sanitizes it through the shared `sanitize::printable_line` filter,
  fits it under the same 1,800-character ceiling the night recap posts under (shedding whole sections in
  a fixed order, `User Tasks` never shed), and delivers it through the unchanged `post_return_recap`
  (posts to `pns-recap`, falls back once to the default route with an explaining line). `pns recap git`
  prints the Git block and, in one fenced block, the stack graph and file list, resolving the pull
  request through `gh-axi pr list --head` (gh-axi's `pr view` has no `--json` and no branch form) and the
  stack from git ancestry, since neither worktrunk nor `gh-axi stack` can answer it here. Both behaviors
  are written into `pns/docs/specs/return-recap.md`, and the work-recap skill and the shared agent rules
  now call these commands instead of saying they are not built; the pns plugin moved to 0.3.0.
  Twenty-five new tests, `just lint-check`, `just test-unit`, both template renders and a live
  `pns recap git` run all passed. Operator steps left: `chezmoi apply`,
  `claude plugin marketplace update pns`, `claude plugin uninstall pns@pns`,
  `claude plugin install pns@pns`, restart Claude Code, then run `/pns:work-recap` once and confirm the
  recap lands. Open question left for the operator: `pns-adapters` now shells `npx -y gh-axi` while the
  sibling `recap/merges.rs` shells `gh` directly, so the two adapters disagree about which GitHub CLI
  they depend on; needs a ruling on which one moves. On 2026-09-15 the full `chezmoi apply` ran and the
  plugin move landed: `claude plugin list` reads `pns@pns` 0.4.0, enabled, and
  [PR #621](https://github.com/webdavis/dotfiles/pull/621) moved `pns-loop` and `pns-work-recap` into the
  shared skills store. [PR #628](https://github.com/webdavis/dotfiles/pull/628) dropped the `pns-recap`
  route the same day, so a recap now posts to the default route rather than `#pns-recap`. Still owed: one
  live `/pns:work-recap` run.

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
  secret that needs a rotation story. DESIGN WRITTEN 2026-09-14 in
  `docs/superpowers/specs/2026-09-14-pns-http-tap-design.md`, and its recommendation is to DECLINE the
  build for now rather than ship it. Task 71 already spent most of this task's motivation: the forced
  command is `command="<binary> tap",restrict` and names no marker path, so the path is written down
  once, in pns, and the silent mismatch this section's intro describes is gone. What is left to buy is
  one `authorized_keys` paste per machine, the Remote Login prerequisite task 71a added, a phone-readable
  result with no Mac-side respelling of the forced command, and one less hand-managed file outside
  chezmoi. What it costs is pns's first listener reachable from a network, its first endpoint that
  authenticates a caller, its first place hostile input arrives from something other than a hook or a
  config file, and the property the task itself names: sshd execs `pns tap` as a one-shot process and
  taps correctly with `[daemon] enabled = false`, while an HTTP listener is a daemon child. That cost is
  worse than it looks, because the harness hooks and the shell notifier deliver synchronously in process
  and each reads the marker to pick a surface, so a machine with a dead daemon still notifies and still
  needs to know where the operator is, which is exactly when this transport is down. Verified against
  Apple's Shortcuts guide (`Request your first API`, apd58d46713f): `Get Contents of URL` has a method
  selector, a headers list and a JSON request body, so the request is buildable with no third-party app,
  but Shortcuts' Generate Hash action takes no key, so there is no native HMAC on the phone and the
  hermes signing shape cannot be reused. Authentication is therefore a config secret in a header, and pns
  cannot make the transport confidential because it knows nothing about the operator's network. The
  design specifies it anyway, in enough detail to implement test-first: `[tap.http]` with `bind` and
  `key`, both rendered commented with no default through `Sample::Example`; a refusal to bind when `bind`
  is written and `key` is missing or under a 32-character floor; a socket address rather than a hostname,
  and the `[failures]` port floor of 1024; `POST /tap` only, with a GET that never taps because
  `moshi-hook` probes local ports and would tap from a scan; the `pns.tap/1` object as the 200 body and
  the same schema with null `marker` and null `surface` as the 401, so no schema change is needed;
  bounded reads with read and write timeouts, which the loopback failures page does not have; the key
  re-read per request so a rotation needs no restart while a `bind` change needs one; one Pairing row in
  `pns doctor`; and seventeen fail-first behaviors. Fourteen choices made in the operator's place are
  listed with their alternatives, and six open questions wait on the operator, the first being whether to
  build it at all. Full document: `docs/superpowers/specs/2026-09-14-pns-http-tap-design.md`. Operator
  steps: (1) Read docs/superpowers/specs/2026-09-14-pns-http-tap-design.md; the recommendation is in the
  Approaches section under D. (2) Decide: build the HTTP tap, or record task 74 as declined. Declining
  leaves the SSH tap as the only transport and leaves task 71b's device verification as the remaining
  work on this feature. (3) If declined, tick 74 with the reason and nothing else changes; both
  transports already write the same marker through the same code, so no cleanup is owed. (4) If built,
  answer the six open questions before the first behavior test, starting with the bind address and what
  makes that address confidential (pns must not detect it). (5) If built, create a KeePassXC entry for
  the tap key with at least 32 characters and add it to dot_config/pns/config-values.toml as { keepassxc
  = "...", field = "Password" }, then regenerate the template with `just pns-config-render`. (6) If
  built, decide whether dotfiles supervises `pns tap serve` under its own LaunchAgent with KeepAlive,
  which is the only way the HTTP tap survives a dead pns daemon. Open questions: (1) Build the HTTP tap
  at all, or record task 74 as declined? Task 76's research explicitly does not authorize it, and this
  design recommends declining for now. (2) If it is built, what bind address, and what makes that address
  confidential? The secret travels in a header and pns terminates no TLS, so the operator states the
  transport's confidentiality and accepts it; pns must not detect it. (3) Is losing daemon-independence
  acceptable, or should dotfiles supervise `pns tap serve` under its own LaunchAgent from the start so a
  wedged daemon does not take the tap with it? (4) Does `pns tap --install` become config-aware (printing
  whichever route is configured), or does a fourth `operation` value `install_http` join the `pns.tap/1`
  vocabulary the phone's parser reads? (5) Where does the key live on the phone, given that the shipped
  Shortcut sets Hostname, SSH Port and Username through a mechanism recorded as "Get all global
  variables"? And does a Shortcut exported after setup carry the key with it? The SSH variant is safely
  shareable because its private key lives in the phone's own key store rather than in the Shortcut; this
  is unverified for a text field. (6) Does the HTTP tap change the answer to task 75? Narrowing SSH
  exposure to the tailnet narrows the SSH tap to the tailnet, and an HTTP listener bound to a tailnet
  address is reachable from exactly the same places, so neither looks like a reason for the other until
  that is confirmed. (7) Table name `[tap.http]` as designed, or `[tap]` with `bind` and `key`, or
  `[tap] type = "http"` to match the 2026-08-31 "type everywhere" ruling at the cost of a key with one
  possible value?

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
  deadline still require operator acceptance. No device state was changed in this investigation. The
  latency measurement protocol was written 2026-09-14 in
  [PR #598](https://github.com/webdavis/dotfiles/pull/598) into the same
  `pns/docs/pns-tap-device-acceptance.md`: trigger-to-confirmation and trigger-to-failure across four
  cases (Mac locked, Mac sleeping, Mac unavailable, phone on a remote network), five trials each, timed
  by phone screen recording and the Clock stopwatch, with `pns tap --info --json` offered as an optional
  read-only split. The file states no latency deadline is accepted until the tables are filled. Operator
  steps left: run the trials and accept or reject a deadline from the numbers.

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
  wake behavior of the Mac against the Shortcut. That drill was written 2026-09-14 in
  [PR #598](https://github.com/webdavis/dotfiles/pull/598)
  (`docs(pns): tap device acceptance drills and corrected shortcut instructions`, merged) into
  `pns/docs/pns-tap-device-acceptance.md`: the three Mac readings, the four states (awake, asleep with
  network wake possible, asleep without it, off) with their exact Mac and phone steps and expected
  Shortcut result, a recording table, and the four reasons network wake is conditional. Operator steps
  left: run the four states on the devices and fill the table.

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
  failure. The corrected instructions were drafted 2026-09-14 in
  [PR #598](https://github.com/webdavis/dotfiles/pull/598) into `pns/docs/pns-tap-apple-shortcut.md` as a
  section marked NOT YET SHIPPED, leaving the verbatim records of what currently ships untouched: it
  corrects the three global variables and four fields on the Run Script Over SSH action, and wires the
  confirmation to the command's own result (`pns tap` prints one line on stdout and exits zero when it
  records; a failure prints on stderr and exits non-zero) instead of to fixed text. Operator steps left:
  make the two edits on the phone (the Comment text and the notification body), verify a real tap
  including an unavailable Mac and a write failure, then promote the drafted blocks into the verbatim
  records and delete the pending section.

## SSH exposure (not a pns task)

- [x] 75. Restrict this Mac's SSH exposure to the tailnet using a supported mechanism. This belongs to
  dotfiles; pns remains network-independent. The earlier `ListenAddress` proposal did not account for
  launchd owning Remote Login's listening socket, documented in `executable_ssh-hardening.sh` and the
  installed `/System/Library/LaunchDaemons/ssh.plist`. Investigate that ownership and available controls
  before choosing the change. Preserve recovery access, review the exact activation and rollback with the
  operator, then verify allowed and disallowed reachability over IPv4 and IPv6, the listener state, a
  real SSH login and a real phone tap. Do not infer network isolation from `sshd -T` alone. The measured
  proposal behind this task landed 2026-09-14 in [PR #600](https://github.com/webdavis/dotfiles/pull/600)
  (`docs(specs): ssh tailnet-only exposure proposal`, merged) at
  `docs/superpowers/specs/2026-09-14-ssh-tailnet-only-proposal.md`. Task 75 shipped 2026-09-14 on
  `feat/ssh-refuse-outside-tailnet`, option A of that proposal, in
  [PR #609](https://github.com/webdavis/dotfiles/pull/609) (merged `68dcc1e2`): the managed sshd drop-in
  gained one `Match LocalAddress "!127.0.0.0/8,!::1,!100.64.0.0/10,!fd7a:115c:a1e0::/48,*"` block with
  `RefuseConnection yes` in both generators (the posture template and `ssh-hardening.sh`'s `print_config`
  heredoc), byte-identical by a new test. Both verifiers gained a fourth check that resolves one
  `sshd -G -T -C` sample per arrival address, six of them, confirmed against the real OpenSSH 10.0p2
  binary, including proving the loop by dropping the tailnet IPv6 negation and watching the sample fail.
  The quickstart runbook records the rule, its ceiling (launchd owns the listening socket, so the port
  stays open and only a real ssh attempt tests the refusal), the console recovery path, and four
  acceptance checks runnable from the Mac itself.

  Deployed 2026-09-14 ~12:00 UTC with the operator's prior approval: `posture ssh install`, `verify` and
  `reload` (built from main at `68dcc1e2`) all exit 0; verify reports all 7 protected directives holding
  and all 6 sampled arrival addresses resolving the demanded verdict; the previous drop-in is backed up
  at `~/workspaces/backups/2026-09-14T11-52-54.000-ssh-hardening-conf.backup.conf`. Checks from the Mac:
  `ssh stephen@192.168.1.26 true` ends with "Connection closed by 192.168.1.26 port 22" (refused, as
  intended); the link-local IPv6 attempt likewise ends with "Connection closed by fe80::...%en0 port 22";
  `ssh stephen@100.77.192.92 true` reaches key authentication ("Permission denied (publickey)", so the
  tailnet arrival is admitted; the agent's own key is not in authorized_keys, which is unrelated to the
  rule); the tailnet IPv6 self-address times out at TCP connect from this Mac itself, before any sshd
  exchange, so that half is proven only by the sshd -G -T -C resolve table in the PR. Remaining for the
  operator: one real phone tap after confirming the Shortcut's Hostname global variable is 100.77.192.92
  or the MagicDNS name, not 192.168.1.26; expect one file-integrity alert naming the drop-in. Closed
  2026-09-15: [PR #609](https://github.com/webdavis/dotfiles/pull/609) merged and deployed on 2026-09-14,
  and the phone tap was tested with Tailscale both on and off, which was the last check this task owed.

  Open questions left: (1) LOUD, unconfirmed: the pns tap Shortcut's `Hostname` global variable was not
  read from the repository (it lives only on the phone), so if it still holds the LAN address or a
  `.local` name, the phone tap stops working the instant the drop-in deploys; read and move it to
  `100.77.192.92` or the MagicDNS name before relying on the tap. (2) Whether `RefuseConnection` should
  join the tree Match scan's keyword set, since a later or earlier `Match LocalAddress` block setting
  `RefuseConnection no` for an address none of the six samples names would pass every check today; left
  out of scope, worth a follow-up task. (3) The refused IPv6 check uses en0's link-local address because
  en0 carries no routable IPv6 on this network; move it to a routable address if that ever changes.

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
  live page/digest, checkpoint and retry acceptance after the operator applies. On 2026-09-14 branch
  `feat/posture-alert-cutover` carried this work through six commits: `b80dbfde` repoints the plist and
  allowlist tuple to `posture alert`; `8e6a02a9` deletes `executable_results-alerter.sh`, its six private
  helpers and the seven shell tests that pinned them, keeping `pipeline-verdict.sh` for
  `pipeline-audit.sh`; `f1d5cd31` corrects the surviving producer-list comments; `502bb3b6` merges
  `origin/main` in; `701d93b9` names the three Bash monitors that still source the dispatch library; and
  `f1f6cc4f` gates the cutover on a live hermes posture route. Independent review returned two SEV-1s and
  one SEV-3, all fixed on the branch: a content conflict in the launchd allowlist (fixed by `502bb3b6`,
  keeping main's file and repointing only the results-alerter row, verified by a zero-exit
  `git merge-tree`); posture's pns route having no hermes endpoint, so every alert and digest leg
  dead-letters at HTTP 404 (fixed by gating the apply on that route existing rather than guessing a
  routing change, `f1f6cc4f`); and stale producer-list comments left by the merge (fixed by `701d93b9`).
  [PR #584](https://github.com/webdavis/dotfiles/pull/584) opened against `main` with `just ship` green
  locally and pushed. NOT MERGED as of 2026-09-14: GitHub Actions never triggered a Lint check-suite for
  the PR across three retrigger attempts (open, an empty synchronize commit, reopen) over roughly 30
  minutes, while sibling PRs in the same window triggered normally; `gh-axi pr checks 584` still reads
  "no CI checks configured". This is an environmental GitHub-side blocker, not a code or merge problem;
  per standing instructions the branch stays open rather than merging without a real "0 failed" result.
  Operator steps once it ships: a full `chezmoi apply` (no by-name apply, no `--exclude=templates`, the
  plist and allowlist both sit in the pipeline known-good manifest arm); confirm the swap with
  `launchctl print gui/$(id -u)/com.webdavis.osquery-results-alerter | grep -A3 arguments`; confirm one
  live tick in `~/.local/log/osquery/results-alerter.log`; confirm the allowlist tuple with
  `posture allowlist list`; THEN trash `~/.local/libexec/osquery/results-alerter.sh` and the six files
  under `~/.local/libexec/osquery/results-alerter/` except `pipeline-verdict.sh`, expecting one integrity
  page from that trash (`~/.local/libexec/osquery/%%` is tracked whether or not the manifest lists a
  file, and a DELETED verb pages before any manifest lookup); verify the digest spool handoff on the next
  daily digest; and verify at-least-once retry against the live cursor with the daemon or gateway
  unreachable. Stays open: getting Actions to trigger a Lint run on PR #584, without which it cannot
  merge (either a manual re-run from the GitHub UI or a look at whether the `blacksmith-sh` app has
  broken Actions dispatch for this repository); whether `posture/docs/acceptance/allowlist-integrity.md`
  and `enrichment.md` need annotating for the shell tests this branch retires (left untouched as dated
  port plans); and a stale doc comment at `uu/crates/uu-adapters/src/lanes/brew/upgrade_record.rs:8`
  naming the deleted `file-integrity-triage.sh`, deferred as a separate cargo workspace out of this
  slice. [PR #584](https://github.com/webdavis/dotfiles/pull/584) has since merged and the full
  `chezmoi apply` ran and passed on 2026-09-15: `osqueryi` reads `com.webdavis.osquery-results-alerter`
  as `/Users/stephen/.cargo/bin/posture alert`. Still owed: one live tick in
  `~/.local/log/osquery/results-alerter.log`, the `posture allowlist list` tuple check, trashing
  `~/.local/libexec/osquery/results-alerter.sh` and the six files under
  `~/.local/libexec/osquery/results-alerter/` except `pipeline-verdict.sh` (all eight were still on disk
  on 2026-09-15), the digest spool handoff and the at-least-once retry check.
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
  [PR #547](https://github.com/webdavis/dotfiles/pull/547) merged at `1f934c7b`. Also on 2026-09-14 the
  plist cutover itself landed in [PR #575](https://github.com/webdavis/dotfiles/pull/575) (merged
  `7bdecf6b`, branch `feat/posture-plist-cutovers`), commit `3dd4b56e`: the uptime-watchdog LaunchAgent
  now runs `posture watchdog` with its whole `EnvironmentVariables` dict deleted, because state,
  snapshots, the legacy queue, both known-good manifest paths, the gateway route and timeout, the canary
  window and `AuditBounds` all already equal the Bash defaults, so the streak memory, pending-growth
  baseline and audit page-once fingerprint carry over untouched; the six watched agent labels were
  checked byte-identical to the Bash `AGENTS` array. Review's only finding for this commit was an
  80-character subject line, fixed by an amend-and-cherry-pick rewrite to 72 characters with the tree
  unchanged (verified by matching tree hashes). Operator steps: the full `chezmoi apply` this shares with
  tasks 47 and 48; confirm the swap with
  `osqueryi --json "SELECT label, COALESCE(NULLIF(program,''), program_arguments) AS program FROM launchd WHERE label='com.webdavis.osquery-uptime-watchdog'"`
  reads `/Users/stephen/.cargo/bin/posture watchdog`; since its interval is 900 seconds, kick it directly
  with `launchctl kickstart gui/$(id -u)/com.webdavis.osquery-uptime-watchdog`, then
  `launchctl print gui/$(id -u)/com.webdavis.osquery-uptime-watchdog | grep -E 'runs|last exit code'`
  expecting exit code 0. `tail -n 20 ~/.local/log/osquery/uptime-watchdog.log` stays EMPTY whether or not
  the tick pages, because a page is the `Reported` outcome and that outcome prints nothing and exits 0
  (`posture/crates/posture/src/watchdog.rs:79`), so log silence is not evidence of phone silence. EXPECT
  EXACTLY ONE CRIT watchdog page on this first tick, plus one independent banner titled "Posture
  notification engine unhealthy" naming 11 dead-lettered pns delivery obligations, and do NOT read it as
  a regression or roll back on it. The Bash-written state carries no `pns_pending` key (measured
  2026-09-13: its only keys are `agents`, `pending` and `pipeline_audit`), so the first Rust tick decodes
  the prior dead-letter count as never observed and reports the standing count once by design: only an
  INCREASE is news, the count is then carried forward, and the next 15-minute tick is silent. The count
  is real: a read-only `sqlite3 "file:$HOME/.local/state/pns/pns.db?immutable=1"` over `ledger_legs`
  reports 0 unacknowledged and 11 dead-lettered, with `delivery_health` generation 23 and acknowledged
  23, so no delivery-health alarm rides along. Then `jq . ~/.local/state/osquery-watchdog-state.json`
  should show all six agents with `runs`/`streak`, a `pending` block, a NEW `pns_pending` block reading
  `{"count":0,"growth_streak":0,"deadletters":11}`, and a `pipeline_audit` block whose `fingerprint`
  clears by the next 15-minute tick rather than reaching the streak-of-two page threshold on this first
  post-apply tick. Stays open: `uptime-watchdog.sh`, `pipeline-audit.sh` and
  `results-alerter/pipeline-verdict.sh` retire from source together in a follow-up pull request that also
  retires the firewall-gatekeeper-monitor and tailscale-monitor Bash producers, after all three lanes'
  live acceptance; nothing was trashed by #575. The full `chezmoi apply` ran and passed on 2026-09-15 and
  `osqueryi` reads `com.webdavis.osquery-uptime-watchdog` as
  `/Users/stephen/.cargo/bin/posture watchdog`. Still owed: the kickstart and exit-code check, the one
  expected CRIT watchdog page with its dead-letter banner, the `osquery-watchdog-state.json` read, and
  the follow-up pull request retiring `uptime-watchdog.sh`, `pipeline-audit.sh` and
  `results-alerter/pipeline-verdict.sh` from source.
- [ ] 47. posture 6.5: finish poll composition and cut over its plist. The application transaction and
  command merged in [PR #544](https://github.com/webdavis/dotfiles/pull/544), and local main contains it.
  Independent review passed 909 workspace tests and 15 private Bash/native command comparisons, including
  exact alerts, baseline bytes, markers and submission order. The full repository gate and required
  continuous integration passed, with four local Rust test workers and unchanged deadlines. Preserve the
  existing baseline and verify exposure and recovery across two live ticks before removing the Bash
  producer. Security-page sound parity is supplied by merged
  [PR #540](https://github.com/webdavis/dotfiles/pull/540), with independent review, full checks and
  required continuous integration passed. Operator deployment and audible acceptance remain separate. On
  2026-09-14 the plist cutover itself landed in [PR #575](https://github.com/webdavis/dotfiles/pull/575)
  (merged `7bdecf6b`, branch `feat/posture-plist-cutovers`), commit `31888162`: the
  firewall-gatekeeper-monitor LaunchAgent now runs `posture poll`, keeping its `PATH` dict, the one
  cutover among the three whose program set is not fully absolute, because `query_path` still walks
  `PATH` for `osqueryi` before falling back to the absolute `/usr/local/bin/osqueryi`; every other Rust
  default already equals the Bash default, so no `EnvironmentVariables` override was needed. Review's
  only finding was an 80-character subject line, fixed by an amend-and-cherry-pick rewrite to 72
  characters with the tree unchanged (verified by matching tree hashes). Operator steps: a full
  `chezmoi apply` (no by-name apply, no `--exclude=templates`, the plist and allowlist both sit in the
  pipeline known-good manifest arm), shared with tasks 46 and 48; confirm the swap with
  `osqueryi --json "SELECT label, COALESCE(NULLIF(program,''), program_arguments) AS program FROM launchd WHERE label='com.webdavis.osquery-firewall-gatekeeper-monitor'"`
  reads `/Users/stephen/.cargo/bin/posture poll`; two poll ticks 60 seconds apart via
  `launchctl print gui/$(id -u)/com.webdavis.osquery-firewall-gatekeeper-monitor | grep -E 'runs|last exit code'`
  before and after `sleep 130`, expecting `runs` to advance by two or more and exit code 0; then the
  exposure/recovery drill: `sudo /usr/libexec/ApplicationFirewall/socketfilterfw --setglobalstate off`,
  wait 70 seconds, expect exactly one CRIT page and `firewall` to read 0 in
  `~/.local/state/osquery-posture-state.json`; re-enable, wait 70 seconds, expect silence on recovery and
  `firewall` back to 1; a second off/on cycle must page again, proving the marker rearms. Stays open: the
  Bash `firewall-gatekeeper-monitor.sh` (with `pipeline-audit.sh` and `pipeline-verdict.sh`) retires from
  source in the follow-up pull request under task 46, after this acceptance; nothing was trashed by #575.
  The full `chezmoi apply` ran and passed on 2026-09-15 and `osqueryi` reads
  `com.webdavis.osquery-firewall-gatekeeper-monitor` as `/Users/stephen/.cargo/bin/posture poll`. Still
  owed: the two ticks 60 seconds apart, and the firewall exposure and recovery drill.
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
  `0efb2119`. Also on 2026-09-14 the plist cutover itself landed in
  [PR #575](https://github.com/webdavis/dotfiles/pull/575) (merged `7bdecf6b`, branch
  `feat/posture-plist-cutovers`), commit `8754b3df`: the tailscale-monitor LaunchAgent now runs
  `posture funnel`, pinning `EnvironmentVariables` to `OSQUERY_TAILSCALE_BIN=/opt/homebrew/bin/tailscale`
  and dropping the `PATH` dict, because that dict is what made the shell resolve the headless brew
  formula and pinning takes PATH ordering out of a detector whose own blind window already pages CRIT; a
  missing or wedged binary still pages a gap naming the path. Review's only finding was the same
  80-character subject line fixed under task 47, carried forward unchanged as `8754b3df` (tree hash
  verified). Operator steps: the same full `chezmoi apply` shared with tasks 46 and 47; confirm the swap
  with
  `osqueryi --json "SELECT label, COALESCE(NULLIF(program,''), program_arguments) AS program FROM launchd WHERE label='com.webdavis.osquery-tailscale-monitor'"`
  reads `/Users/stephen/.cargo/bin/posture funnel`; one tick after `sleep 70`, confirm exit 0 via
  `launchctl print gui/$(id -u)/com.webdavis.osquery-tailscale-monitor | grep -E 'runs|last exit code'`;
  `cat ~/.local/state/osquery-tailscale-funnel.json` reads `{"funnel":"inactive"}` with no `.gap`
  sibling, cross-checked by hand against `/opt/homebrew/bin/tailscale funnel status --json`. Stays open:
  the Bash `tailscale-monitor.sh` (with `pipeline-audit.sh` and `pipeline-verdict.sh`) retires from
  source in the same follow-up pull request as task 47, after this acceptance; nothing was trashed by
  #575. Also on 2026-09-14, `git cherry origin/main feat/posture-funnel` still listed all five fix
  commits as absent from main, because PR #551 merged into `feat/posture-watchdog-health` after that
  branch had already merged into main, so the merge carried the fixes onto a side branch rather than onto
  the trunk. This pull request carries them to main in their original order: bound the funnel exposure
  page by key count (`0037bc33`), name the funnel command in its queue failures (`b1a8b777`), read the
  funnel binary through the shared executable check (`4ea6e8b9`), pin the exposure sort, the worst case
  and the executable arms (`4cb11f21`), and read a disabled funnel timeout as no limit rather than a
  failure (`c50f95d6`). It matters because #575 already points the tailscale-monitor LaunchAgent at
  `posture funnel`, so the next full apply would otherwise deploy a funnel without these fixes. That pull
  request merged 2026-09-14 as [PR #587](https://github.com/webdavis/dotfiles/pull/587)
  (`fix(posture): carry the reviewed funnel fixes to main`), carrying all five fix commits to main in
  their original order. The full `chezmoi apply` ran and passed on 2026-09-15 and `osqueryi` reads
  `com.webdavis.osquery-tailscale-monitor` as `/Users/stephen/.cargo/bin/posture funnel`. Still owed: one
  tick after `sleep 70` at exit 0, and the `osquery-tailscale-funnel.json` cross-check against
  `tailscale funnel status --json`.
- [x] 49. posture 6.7: retire the drainer only after every producer has migrated, all three queue tables
  are empty and the operator has reviewed dead-letter disposition. Remove its loaded job, monitored
  label, legacy queue reader and growth state together. The drainer is still loaded at audit time.
  Preserve an export of reviewed dead letters, obtain fresh approval for exact-row removal and reread all
  three counts. Unresolved rows retain the route, key, drainer and queue. Once empty and no other
  consumer needs them, retire the old `priority` route/key and propose cleanup of the queue's three
  files, `osquery-spool/`, `osquery-tailscale-funnel` and `~/.config/osquery/webhook-secret` as required
  by the port plan. Secret values stay out of logs and review artifacts. Done:
  [PR #622](https://github.com/webdavis/dotfiles/pull/622) removed the drainer and the Bash alerter's
  dispatch library from source on 2026-09-15, and after the apply the LaunchAgent was booted out and its
  plist, the two scripts, the webhook secret file, the queue database (0 pending rows) and its log were
  trashed the same day. Confirmed 2026-09-15:
  `launchctl print gui/$(id -u)/com.webdavis.osquery-alert-drainer` reports no such service in the user
  domain, and `~/.local/libexec/osquery/drain-undelivered-alerts.sh` is absent.
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

- [x] 57e. Retire the `deleteValueAtPath "skillOverrides.<name>"` lines in
  `private_dot_claude/modify_settings.json`. The branch `docs/clean-code-rust-test-first` added nine
  (clean-code, clean-code-rust, clean-code-swift, defuddle, obsidian-bases, obsidian-cli,
  obsidian-markdown, owasp-security, tuicr) so one apply scrubs the stale `user-invocable-only` key a
  promotion to core leaves in the live file. They are tombstones: once every machine that carried those
  keys has applied (check `jq .skillOverrides ~/.claude/settings.json` shows none of the nine), delete
  the lines. Longer term, derive the whole block from the lock's `tiers` table with `include` and
  `fromJson` so a promotion needs one edit and no tombstone; requested in the operator's Plannotator
  review on 2026-09-13. Attempted 2026-09-14 on `chore/settings-tombstones`: the nine lines were dropped
  (`fd3d68b0`) and [PR #582](https://github.com/webdavis/dotfiles/pull/582) is open but not merged. The
  branch stopped at the `git fetch origin && git merge origin/main` step: a real content conflict landed
  in this same file's install-comment block (this branch's trimmed wording against origin/main's fuller
  2.1.257/2.1.270 history), not confined to `graphify-out/graph.json`, so the merge was aborted, leaving
  the branch clean at `37a1e55a`. No push, ship run, or merge was attempted past that point; the wording
  conflict needs a human decision before resuming. Closed 2026-09-15:
  [PR #582](https://github.com/webdavis/dotfiles/pull/582) merged on 2026-09-14, and
  `grep -n deleteValueAtPath private_dot_claude/modify_settings.json` now returns a single line, inside a
  comment, with no tombstone call left.

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

- [x] 57h. Nested worktrees leak their `.chezmoidata` into every apply. Measured 2026-09-14 on dresden:
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
  own. Audited 2026-09-14 (snapshot `2026-09-14T04:44:39Z` against `origin/main` `7bdecf6b`), inventory
  only, nothing deleted, pruned, moved or killed. Git held 55 registrations, the source checkout plus 54
  external, and not one was stale: `git worktree prune --verbose --dry-run` printed nothing and exited 0,
  no registration carried a `locked` line, and every registered path existed on disk, so the 187 above
  was a pre-cleanup number and git itself had nothing left to prune. The 54 held 24.7 GB of checkouts
  plus 12 MB of `.git/worktrees` metadata: 40 under `~/.herdr/worktrees/dotfiles/`, 10 under
  `~/workspaces/dotfiles-worktrees/`, 3 under `~/.paseo/worktrees/1sk17y2x/` and one sibling of the
  source checkout. `.worktrees/` held only a `.DS_Store`. Classified 14 live (an open herdr workspace
  from `herdr worktree list`, a non-stale process by `lsof -d cwd`, or an open pull request), 24 whose
  content was in main, 4 detached carrying a unique commit, and 12 on a named branch whose content was
  not in main. Merge-base ancestry alone mislabeled 12 of the 54, so two further tests were added and
  every row carried one verdict built from all three. Five landed by squash, each a merged pull request
  whose `head.sha` equaled the worktree HEAD and whose merge commit was an ancestor of main:
  [PR #525](https://github.com/webdavis/dotfiles/pull/525),
  [PR #526](https://github.com/webdavis/dotfiles/pull/526),
  [PR #528](https://github.com/webdavis/dotfiles/pull/528),
  [PR #530](https://github.com/webdavis/dotfiles/pull/530) and
  [PR #531](https://github.com/webdavis/dotfiles/pull/531). Three were already upstream by patch identity
  per `git cherry` (`docs-sol-lane4-fixes`, `pns-daemon-fixture-children`, `pns-failure-reporting-spec`)
  and four carried only merge commits of other branches, introducing nothing unique (`g3-throwaway`,
  `nvim-acceptance-rehearsal`, `s61-g3`, `s61e-g3`). The reverse trap fired once:
  [PR #551](https://github.com/webdavis/dotfiles/pull/551) was merged into
  `feat/posture-watchdog-health`, its merge commit `559b9e61` was not an ancestor of main, and
  `feat/posture-funnel` still carried five commits with no upstream equivalent. Four findings stopped a
  blind sweep. `herdr-process-plan` was an ancestor of main yet held 105 untracked files implementing the
  `herdr-process` plugin, and `git ls-tree` on that path returned zero files on main and on every ref
  from `git for-each-ref refs/heads refs/remotes`, while the sibling `herdr-smart-nav` path returned 17
  on the same command, so the empty result was the plugin's absence and not a bad path. Three detached
  HEADs pinned a commit no ref contained (`osq-review-4b` `e71776b8`, `r1-review-4b` `daf7fc98`,
  `rev4b-114` `4cb7de39`, all graphify or review scratch), while `integration` `2c6bd7f3` was covered by
  `integration/modernization`. `posture-converge-staging` was a gutted checkout, 1571 of its 2200 tracked
  files absent from the working tree while the index still listed them, safe because
  [PR #470](https://github.com/webdavis/dotfiles/pull/470) merged that exact HEAD. And sixteen stale
  `pns failures serve` debug processes held a worktree cwd, eight in `tuicr-config` and eight in
  `pns-status-quiet` and `pns-tap-mac`, two paths that no longer exist on disk and have no
  `.git/worktrees` entry, so their inodes stay pinned; pid 1797 still ran from the retired nested
  `.worktrees/tuicr-config/pns/target/debug/pns`. No worktree held an interrupted operation, the
  `MERGE_RR` files present being leftover rerere state. The four-tier prune proposal with the exact
  per-path command, the full uncommitted-file listing and the seven-item risk register sit in
  `~/workspaces/backups/2026-09-14T04-44-39.dotfiles-external-worktree-registrations.backup.txt`: tier 1
  is 7 registrations and 2421 MB with nothing to decide, tier 2 is 17 and 5326 MB where the content is in
  main but uncommitted files sit in the tree, tier 3 is the 4 detached, tier 4 the 12 unlanded, and the
  14 live ones are marked keep. Recorded on Todoist `6hVvc56G8r6xJppM` (comments `6hW4JcwhpF4RrrpM` and
  `6hW4JjFpQ5WJjCMM`); the daemons were split out as `6hW4JgqJJcR43QjM` and the plugin preservation
  already had `6hW4HCJxjWQvvvQM` from the same night's task 68 recheck, which this audit confirmed
  independently rather than duplicating. The count moved from 51 to 57 registrations between 04:34:52Z
  and 04:48:14Z as concurrent agents created worktrees, so the liveness block must be re-run immediately
  before any removal. Remaining: the operator approves the exact prune and removal set; nothing was
  removed. Owed from the operator: (1) Re-run the liveness block before deciding anything. The
  registration count moved from 51 to 57 between 04:34:52Z and 04:48:14Z as concurrent agents created
  worktrees, and origin/main advanced from f24aba51 to 7bdecf6b mid-audit:
  `cd /Users/stephen/workspaces/Ivy/webdavis/dotfiles && git fetch --prune origin && git worktree list --porcelain | grep -c '^worktree ' && HERDR_ENV=1 herdr worktree list && lsof -d cwd -Fpcn | grep -E 'worktrees/dotfiles|dotfiles-worktrees'`;
  (2) Decide R1 first, because it gates a tier 2 entry: either commit the 105 untracked files in
  ~/.herdr/worktrees/dotfiles/herdr-process-plan onto feat/herdr-process, or copy
  dot_local/share/herdr/plugins/herdr-process/ (and the fixture tree named in Todoist 6hW4HCJxjWQvvvQM)
  to ~/workspaces/backups/2026-09-14T<HH-MM-SS>.herdr-process-plugin-source.backup/. Nothing else about
  that worktree may be acted on until this is done.; (3) Approve or reject tier 1 as a block, 7
  registrations and 2421 MB, nothing at stake:
  `git worktree remove ~/.herdr/worktrees/dotfiles/g3-throwaway`, `... nvim-acceptance-rehearsal`,
  `... pns-daemon-fixture-children`, `... pns-failure-reporting-spec`, `... s61-g3`, `... s61e-g3`,
  `... uu-run-logging`; (4) Read the per-path uncommitted listing in the artifact, then approve tier 2
  path by path, 17 registrations and 5326 MB, each needing `git worktree remove --force <path>`. Three of
  them hold work against a layout main retired (lights-implementation, pns-executable-deadline,
  herdr-smart-nav-clean-code, see R5); tuicr-config is blocked by its 8 resident pns processes;
  herdr-process-plan is blocked by R1.; (5) Decide tier 3, the 3 detached HEADs whose commit no ref
  contains. To keep one before removing its worktree: `git branch archive/osq-review-4b e71776b8`,
  `git branch archive/r1-review-4b daf7fc98`, `git branch archive/rev4b-114 4cb7de39`. All three are
  graphify or review scratch. ~/workspaces/dotfiles-worktrees/integration needs no archive branch, its
  HEAD is contained by integration/modernization.; (6) Decide tier 4, the 12 named branches whose content
  is not in main, 4607 MB. Removing a checkout keeps every commit because the branch ref survives, so
  this is only a disk decision. Look at feat/posture-funnel first: 4128 MB and 5 commits that PR #551
  never carried to main, so it may want a fresh pull request rather than a removal.; (7) Decide the 16
  stale pns processes (Todoist 6hW4JgqJJcR43QjM). Terminating them releases the inodes that the two
  deleted directories still pin and unblocks tuicr-config in tier 2. Killing processes is a gated action,
  so no command is proposed here; also decide whether pns needs a guard so a test run stops leaking a
  `pns failures serve` daemon.; (8) After any removal run `git worktree prune` once, then
  `du -sh /Users/stephen/workspaces/Ivy/webdavis/dotfiles/.git/worktrees` (12 MB now) and
  `git worktree list | wc -l` to confirm. Deleting a branch is a separate decision and belongs with
  ledger tasks 67 and 68.; (9) Reconcile with the sibling audit:
  ~/workspaces/backups/2026-09-14T04-40-09.worktree-inventory-task68.backup.txt covers the same
  registrations with an ancestry-only merged column that reads 12 of the 54 as unmerged when their
  content is in main. Use the 57h verdict column for that question and the task 68 file for the rest.
  Closed 2026-09-15: `.worktrees/` holds only a `.DS_Store`, and `git worktree list` shows every
  remaining worktree under `~/.herdr/worktrees/dotfiles/`.

- [ ] 2026-09-14: the merged-worktree sweep from 57h became a repository tool in
  [PR #605](https://github.com/webdavis/dotfiles/pull/605)
  (`feat(worktrees): sweep merged, clean worktrees through herdr`, merged), not a numbered task.
  `dot_local/libexec/executable_prune-merged-worktrees.sh`, deployed to
  `~/.local/libexec/prune-merged-worktrees.sh`, walks `git worktree list --porcelain`, joins it against
  `herdr worktree list` for a workspace id when `HERDR_ENV` is set, and after a `git fetch --prune`
  removes every linked worktree whose HEAD is an ancestor of `origin/main` and whose tree is clean apart
  from `graphify-out/graph.json`. A registered checkout goes through `herdr worktree remove --force` so
  the sidebar row leaves with the directory; an unregistered one through `git worktree remove --force`.
  Detached HEADs, unmerged branches, dirty trees and the worktree the run is standing in are kept and
  print the reason; `--dry-run` reports the same decisions and removes nothing; no branch is ever
  deleted. `just worktrees-prune` runs it. Eight bashunit behaviors pin the decisions, and the shared
  agent ruleset now tells agents a lane's worktree is removed through herdr once its pull request has
  merged, with this recipe as the sweep. A live dry run reported six merged, clean worktrees. Operator
  steps left: run a full `chezmoi apply` (KeePassXC unlocked), then `just worktrees-prune --dry-run`
  followed by `just worktrees-prune` for real, confirming no session is still using a listed worktree
  first since the sweep cannot detect that itself. The full `chezmoi apply` ran and passed on 2026-09-15
  and `~/.local/libexec/prune-merged-worktrees.sh` is deployed. Still owed: the
  `just worktrees-prune --dry-run` read and the real run.

- [x] 57j. Espanso `,,ee` for `echo $?` (operator request 2026-09-14):
  [PR #565](https://github.com/webdavis/dotfiles/pull/565) (`feat/espanso-echo-exit-status`) adds the
  match to the Commands section of `snippets.yml`; merged 2026-09-14 (`1fdfb288`) and deployed by the
  2026-09-13 20:15 apply (the deployed file carries the trigger).

- [x] 57l. clean-code as a Claude Code plugin (operator 2026-09-14: a plugin for Claude specifically,
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
  deletes an undeclared target). Installed 2026-09-13 after the 20:50 apply: on Claude Code 2.1.270 the
  bare `claude plugin install clean-code` fails with "Invalid channel: clean-code" (the runbook's 2.1.257
  workaround is stale; the plugin sharing its marketplace's name trips the parser), and
  `claude plugin install clean-code@clean-code` succeeds (cache holds base, rust, swift). Update the
  runbook sentence about the bare form in the next docs pass. A restart of Claude Code makes
  `/clean-code:rust`, `/clean-code:swift`, `/clean-code:base` available. That docs pass is
  [PR #582](https://github.com/webdavis/dotfiles/pull/582) (`chore/settings-tombstones`, commits
  `c847c49e` and `fbdbae1c`): it names the 2.1.270 install form in both the template comment and this
  runbook sentence, but is not yet merged, stopped behind the same unresolved `modify_settings.json`
  merge conflict recorded under 57e. Closed 2026-09-15:
  [PR #573](https://github.com/webdavis/dotfiles/pull/573) shipped the plugin and
  [PR #582](https://github.com/webdavis/dotfiles/pull/582) carried the runbook sentence.

- [x] 57m. zoetrope (operator request 2026-09-14): `brew install furkankly/tap/zoetrope` (0.2.0, `zoe`)
  and `herdr plugin install furkankly/zoetrope/herdr-plugin` (`furkankly.zoetrope`, enabled) done by hand
  on dresden; the tap, trusted tap, formula and herdr plugin roster entry merged in
  [PR #571](https://github.com/webdavis/dotfiles/pull/571) (`feat/zoetrope`, 2026-09-14, `6da70bfb`). The
  operator ran `herdr plugin action invoke setup-keys --plugin furkankly.zoetrope` (succeeded): it
  appended a 25-line managed block to `~/.config/herdr/config.toml` (`[[keys.command]]` binding
  `prefix+shift+z` to `furkankly.zoetrope.open`, plus two commented placements), the same
  plugin-writes-into-config drift class as 57k, so the block was copied byte for byte into
  `dot_config/herdr/config.toml` in [PR #572](https://github.com/webdavis/dotfiles/pull/572)
  (`fix/herdr-zoetrope-keys`, merged 2026-09-14, `6c571969`). Acceptance: the next apply neither asks
  about the file nor drops the `prefix+shift+z` binding. Accepted 2026-09-15: the full apply ran without
  asking about the file, and `grep -n 'prefix+shift+z' ~/.config/herdr/config.toml` still reports the
  binding at line 340.

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

- [x] 57i. The post-commit graphify hook races the pre-push lint gate. Seen twice on 2026-09-14 (scalebar
  and espanso pushes made right after their commit): `chezmoi execute-template` in
  `shellcheck-rendered-template` aborts with
  `lstat .../graphify-out/cache/ast/<hash>.tmp: no such file or directory` because the hook is still
  rewriting its cache inside the source tree while chezmoi walks it, so the gate reports "lint drift"
  with 0 files changed and the push is refused. A second push a minute later passes. Fix candidates,
  robust first: move graphify's cache out of the source tree (its output directory setting, or a symlink
  like the `minutes` one), or have the pre-push hook wait for a running graphify rebuild before the gate;
  never a retry loop in the gate. Investigated 2026-09-14 on `fix/graphify-push-race` (not pushed, no PR
  opened): five local commits (`c93ff206`, `211e84a8`, `0c0598d3`, `15769313`, `5d4789db`) find that
  neither hook caused the two reported failures, since `scripts/treefmt/lib-render-context.sh` already
  gives the render a shallow source view and both failures predate that fix; they add a deterministic
  000-permission fixture standing in for the roughly-1-in-25 race, correct a stale date in this runbook,
  and fold the rebuilt graphify map. `git fetch && git merge origin/main` completed cleanly at
  `90473121`. Stopped per task instructions after `just ship` failed three times (1, then 1, then 3
  failing tests) on unrelated Rust process/signal-timing tests (the hue bridge deadline test,
  posture-adapters closed-pipe/grace/signal deadline tests) under confirmed heavy machine load (load
  average 40.43, 5 concurrent cargo/agent processes); the branch diff touches no Rust code. Ready to
  resume `just ship` once load subsides. The `fix/graphify-push-race` branch above was abandoned in favor
  of a cleaner root-cause finding, shipped 2026-09-14 in
  [PR #597](https://github.com/webdavis/dotfiles/pull/597)
  (`docs(git-hooks): document the graphify rebuild vs push-gate race`, merged): neither hook caused the
  failure. `treefmt` already excludes `graphify-out/**`; the real walker was `chezmoi execute-template`
  inside `shellcheck-rendered-template`, which reads the whole chezmoi source state before rendering, and
  `.chezmoiignore` filters deploy targets, not that walk. Main already carried the fix, a shallow source
  view in `scripts/treefmt/lib-render-context.sh` that lstats top-level symlinks instead of descending
  into them; both failed pushes came from worktrees branched before that landed. This pull request
  documents the finding in `docs/runbooks/git-hooks.md` and extends
  `test/unit/formatter-render-context.test.sh` with a mode-000 `graphify-out/cache/ast` fixture standing
  in for the race, since a walk that descends cannot open it on any run. `just ship` passed clean. Known
  limit recorded rather than fixed: eleven unit tests still hand chezmoi the real checkout as `--source`,
  so the same churn can still redden `just test-unit`; routing them through the shared render-context
  helper is not a drop-in since several set their own `HOME` and `EXIT` trap.

## posture cleanup

- [x] 58. posture 8.1 to 8.3: implement the SSH hardening port in its three planned stages. The command
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
  `/private/tmp/dotfiles-modernization/task58/HANDOFF.md`. Closed 2026-09-15:
  [PR #549](https://github.com/webdavis/dotfiles/pull/549) merged, and `posture ssh install`,
  `posture ssh verify` and `posture ssh reload` all exited 0 live on 2026-09-14.
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
- [x] 60. posture 9.2: finish the completion report, original 187-test successor/disposition mapping,
  before/after table and decision index. `posture/docs/test-baseline.tsv` is only the original result
  inventory. Preparatory mapping on `docs/posture-test-mapping` at `d95c39f3` preserves all original
  columns and maps all 187 leaves to exact assertions or explicit gaps. It records merged, unpublished
  and deployed status separately. On 2026-09-13 `main` was merged in (`93f3f288`), `just ship` passed
  (exit 0, 4m31s) and [PR #554](https://github.com/webdavis/dotfiles/pull/554) was opened against `main`;
  it merged into `main`. Independent review found the README's Task58 row and the B041 bullet needed to
  name the real constants; fixed at `068e34e1` before merge. The final post-port size comparison and
  decision index are still required. The Rust size gate already covers posture; do not add it again. On
  2026-09-14 `f16938d2` on `docs/posture-completion-report` added the two missing pieces to
  `posture/docs/README.md`: a before/after implementation-size table pairing every Bash entry point and
  sourced module with its posture successor and source state (845 Bash lines retired across four entry
  points, 9,292 still tracked across 18 files, six Rust crates holding 16,017 implementation lines of
  40,824 total across 358 files, measured with `wc -l` and the clean-code-rust file-size command), and a
  decision index mapping the seven boundary records under `posture/docs/decisions/` to their paired
  specification and Bash-derived acceptance map. No Rust size gate was added, since the existing one
  already covers posture. `just m` and `just lint-check` both passed (exit 0); this pull request carried
  no independent-review findings. Assumptions: the report is `posture/docs/README.md`, the only document
  that calls itself the report and names the missing pieces in its own closing paragraph; "retired Bash"
  counts only the four entry points the port has actually deleted, with the remaining 18 files read as
  still-in-scope "before" rather than omitted; implementation size means source lines of the tools, not
  their tests, with both columns reported for Rust since its test share is the main reason the Rust total
  exceeds the Bash total; the decision index indexes the seven records in `posture/docs/decisions/` per
  the plan's own phrase "the decision records index". The report's accepted dispositions and
  deployed-status refresh stay open by design, scoped to a separate change once the ports and cutovers
  meet their own gates. [PR #577](https://github.com/webdavis/dotfiles/pull/577)
  (`docs/posture-completion-report`, merged `528746a0`). No operator steps.
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
- [ ] 79. Stop the posture digest repeating the same file. The 2026-09-14 digest carried 110
  `agent_authfile_changed` findings for `~/.codex/config.toml`, which Codex rewrites from its own model
  while it runs, so every rewrite arrives as a fresh finding rather than as news. Decide between an
  allowlist entry for that path and a debounce that collapses repeats of one path inside a digest window,
  then build the one chosen. Record the reasoning either way, because an allowlist entry stops watching
  an agent credential file while a debounce keeps watching it.

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
  its deployed copy and obsolete logs after acceptance. DONE 2026-09-14 in
  [PR #608](https://github.com/webdavis/dotfiles/pull/608)
  (`feat(aerospace): point the seven light keys at the lights binary`, merged): F4 to F10 in
  `dot_aerospace.toml` now call the `lights` binary for the Studio. Operator steps left: run
  `aerospace reload-config` after the apply, then press the keys in the Studio to verify. Separately, on
  2026-09-14 lights gained whole-house presets bound to aerospace keys f1 through f3, merged in
  [PR #611](https://github.com/webdavis/dotfiles/pull/611)
  (`feat(lights): whole-house presets and bedroom alias fix`, merged): a `[presets]` table names each
  preset's ordered per-room plan, `lights preset <name>` walks it reporting every room and exiting
  non-zero on the first failure, `lights preset` alone lists the configured names, and `--room` alongside
  a preset is refused. Three presets (morning, afternoon, evening) ship uncommented, mirroring the Studio
  and bedroom's existing Hue automation slots and reusing the Studio's scene names for the kitchen, since
  `--room` cannot address the `Kitchen` zone the kitchen's own automation actually drives. The same PR
  fixed the shipped `bedroom` alias, which had expanded to the nonexistent `3F - Master Bedroom` and made
  every `--room bedroom` command exit 2; it now reads `3F - MBedroom`, correcting the config template,
  the compiled default, both fixtures, the argument-surface case list and the two documents that had
  recorded the stale name. No lamp was changed in this work; all nine room-and-scene pairs were checked
  only against a read-only bridge listing. Operator steps left: run a full `chezmoi apply` with KeePassXC
  unlocked (rebuilds and reinstalls `lights`, deploys `~/.config/lights/config.toml` and
  `~/.aerospace.toml`); run `aerospace reload-config`; confirm `lights preset` lists `afternoon`,
  `evening` and `morning`; press f1, f2 and f3 and watch the Studio, bedroom and kitchen move to
  Energize, Concentrate and Read; confirm `lights --room bedroom status` now reports `3F - MBedroom`
  instead of exiting 2; edit a preset through the `[presets]` table in
  `dot_config/lights/private_config.toml.tmpl` and re-apply, mirroring any change into
  `lights/crates/lights-adapters/tests/fixtures/defaults.toml`. Open questions: whether a fourth and
  fifth preset (night, late) should cover the 20:00 Relax and 23:00 Nightlight automation slots and on
  which keys; whether `lights` should learn to recall a Hue smart scene, since the kitchen and bedroom
  motion automations ramp colour temperature through a `smart_scene` resource no preset step can express;
  whether `lights` should learn to target a zone, starting with the kitchen's own `Kitchen` zone ladder,
  instead of borrowing the Studio's scene names; whether evening should also turn off the pass-through
  rooms the shipped presets leave alone (`1F - Front door`, `2F - Staircase`, `1.5F - Staircase`);
  whether f1 through f3, which sit under the macOS brightness and Mission Control glyphs, are the right
  keys versus F4 through F10 or the fn row beyond F10; whether `lights preset` with no name should also
  print each preset's steps; and that an acceptance run against real hardware, one press of each key with
  all three rooms in view, has not happened yet. On 2026-09-15
  [PR #627](https://github.com/webdavis/dotfiles/pull/627) added the seven-step rotation (Nightlight,
  Dimmed, Rest, Soho, Relax, Read and Energize, falling back to Read) and
  [PR #629](https://github.com/webdavis/dotfiles/pull/629) added five time-of-day presets, morning
  Energize, afternoon Concentrate, evening Relax, dusk Rest on F4 and night Nightlight on F7. The full
  apply ran that day and keys F1 to F4, F7, F8, F9 and F10 were tested live. Still owed: F5 and F6, and
  the three lamp drills, which were not run.

- [ ] 80. Lights features the operator wants built but NOT bound to a key and NOT set in
  `~/.config/lights/config.toml` (operator ruling 2026-09-15): `bed` and `away` presets whose steps
  switch rooms off (`off = true`), `lights preset now` picking the preset from clock windows, `--all` on
  `scene` and `brightness`, and `--over <duration>` for a fade. Each of these is a change to `lights`
  alone; `dot_aerospace.toml` and the shipped `[presets]` table stay as they are until the operator asks
  for them. Ship them as separate pull requests in that order, since only the first two touch the preset
  walker.

- [ ] 63. lights: decide manifest coverage for `~/.cargo/bin/lights`, its current install target. The
  existing generated-binary exception covers posture only. Update the stale target in the lights plan and
  spec when recording the decision. On 2026-09-14 a design for this decision was written and lives at
  `docs/superpowers/specs/2026-09-14-lights-manifest-coverage-design.md`. It recommends NO:
  `~/.cargo/bin/lights` carries no build record and joins neither known-good manifest. The pipeline
  manifest's stated responsibility is the osquery pipeline's own integrity (the slice-15 ruling in the
  generator's docblock) and the managed-bin manifest's criterion is unattended execution; lights meets
  neither, since its only callers are the seven aerospace keys, in the foreground, at the operator's own
  privilege. Measured on dresden: `~/.cargo/bin` is watched by no `file_paths` group and by none of the
  four packs, so a row there buys a fifteen-minute periodic hash from the two audits and no event-driven
  coverage at all; `~/.local/state` holds build records for pns and posture only; pns owns its own Hue
  client in `pns/crates/pns-adapters/src/hue.rs` and never spawns the lights binary; and hashing the
  3,096,064-byte binary costs 0.05s against a 900s tick, so cost is not the argument. The document
  records that the port is a coverage regression, because `~/.local/libexec/control-hue-lights.sh` is
  manifested and watched today, and that the same regression already happened unremarked for uu, which
  runs weekly under `com.webdavis.uu` from `~/.cargo/bin/uu` with no record and no row. Under the
  recommendation the implementation is documentation only: the stale `~/.local/libexec/lights` target and
  its retired libexec justification in `docs/superpowers/specs/2026-09-06-lights-design.md` and in
  `docs/superpowers/plans/2026-09-06-lights-plan.md`, plus one sentence in the generator's comment naming
  why its loop holds pns and posture alone. `docs/remaining-work.md:287` keeps its
  `~/.local/libexec/lights` reference: that leftover is real on disk (3,091,456 bytes, dated Sep 9) and
  the line is a trash-this note. No test is added, because a guard on the loop's membership is
  declaration-consistency checking under the 2026-08-05 ruling. Two alternatives are priced in full:
  posture-parity (build record, row, ceiling, `pipeline_path` arm, a new bashunit suite, and roughly a
  hundred lines of security-critical Bash in the lights builder), and widening the watch set to
  `~/.cargo/bin` with a manifest-driven tracked set, which is recommended as its own task rather than
  folded in here. Five open questions wait on the operator, including whether uu gets a record and
  whether pns's absence from the exact-match list in `posture/crates/posture-domain/src/known_good.rs` is
  intentional. Full document: `docs/superpowers/specs/2026-09-14-lights-manifest-coverage-design.md`.
  Operator steps: (1) Read docs/superpowers/specs/2026-09-14-lights-manifest-coverage-design.md and
  accept or reject the NO recommendation for `~/.cargo/bin/lights`. (2) Answer the five open questions at
  the end of the document, above all whether `~/.cargo/bin/uu` gets a build record and a row; it runs
  weekly under launchd with none today and is a stronger case than lights. (3) If the recommendation is
  accepted, the follow-up is a documentation-only pull request (two docs plus one comment). No apply, no
  rebuild, no manifest change, no new test. (4) If approach B is chosen instead, confirm the lights
  artifact ceiling before builder work starts (6 MiB / 6,291,456 bytes proposed, about twice the
  3,096,064 bytes measured), and confirm that a lights row must never reach a machine before the first
  successful lights build, because an `unbuilt` row over a present binary pages every audit tick. (5)
  Trash the pre-move leftovers, already on the deployed-leftovers list: `~/.local/libexec/lights`
  (3,091,456 bytes, dated Sep 9) and `~/Library/Logs/smart-lights.log`. Both are operator-run and need
  per-invocation confirmation. Open questions: (1) Does `~/.cargo/bin/lights` carry a build record and a
  known-good manifest row? The document recommends no; accepting or rejecting that is the decision this
  task asks for. (2) Does `~/.cargo/bin/uu` carry one? It runs weekly under `com.webdavis.uu` from that
  path with nobody watching, which satisfies the managed-bin manifest's own stated criterion, and it has
  no record and no row today. (3) Is pns's absence from the exact-match list in `pipeline_path`
  (posture/crates/posture-domain/src/known_good.rs) intentional? The generator writes a pns row while
  that function names only posture; the disagreement is inert today because nothing watches
  `~/.cargo/bin`. (4) Should `~/.cargo/bin` join the osquery watch set with a manifest-driven tracked
  set, so the manifested binaries get event-driven coverage instead of a fifteen-minute periodic hash?
  That needs an operator-run osqueryd restart and the churn question answered against uu's own weekly
  cargo upgrades. (5) Is the manifest generator's comment the right home for the tier rule, or should it
  live only in the lights specification? Two homes means two places to keep true; one means a reader at
  the loop does not find it.

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
  decisions remain open: run drill two in
  [`docs/acceptance/nvim-acceptance-drills.md`](acceptance/nvim-acceptance-drills.md).

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

- [x] 67. Reconcile local branches before further cleanup. On 2026-09-13 there are 577, of which 518 are
  ancestors of `origin/main`. No `wf_*`, `worktree-agent-*` or `agent-*` branches remain. The four
  `backup/*` branches contain unmerged work and were deliberately retained by the Claude session.
  Classify the other throwaway candidates by reachability, attached worktree, dirty state and owner.
  These counts do not authorize deletion. Obtain approval for the exact proposed groups. Inventory done
  2026-09-14; nothing was deleted and no `git branch -D` or `git worktree remove` ran. The classification
  is pinned to `origin/main` `7bdecf6b`, the tip left by PR #575, because the repository moved twice
  during the run: a first pass at 04:34Z read 601 branches against `f24aba51`, and by 04:46Z there were
  607 with six more worktrees. At the pinned base: 607 local branches, 549 contained in `origin/main`, 58
  not, 545 heads still on origin, and 57 worktree registrations, 51 on a named branch and six detached.
  The 577 branches and 518 ancestors this line recorded on 2026-09-13 are both superseded. Seven groups,
  summing to 607: P, nine protected, being `main`, the four `backup/*` refs, the heads of the three open
  pull requests #546, #51 and #24, and `integration/modernization`, which is the base of #51; B, 20
  merged but checked out, six of them live work from the same night's concurrent sessions; C1, 17 merged
  with no worktree whose remote head is already gone; C2, 511 merged with no worktree whose remote head
  stays on origin; D, 28 unmerged and checked out, overlapping task 68; E1, eight unmerged with no
  worktree whose tips are reachable from another ref; and E2, 14 unmerged with no worktree that
  `git branch --contains` and `git branch -r --contains` prove are the only copies of their commits. That
  proposes 536 deletions and withholds 71. `git branch -d` suffices for all 528 in C: 33 of them have no
  upstream, so the HEAD test would decide those, and although local `main` sat five commits behind
  `origin/main`, all 528 are contained in local `main` too. The sole-copy branch worth protecting most is
  `docs/nvim-acceptance-ledger` `5001c94f`, nine commits ahead, last committed 2026-09-13, whose
  `.worktrees/nvim-acceptance-ledger` registration no longer exists. Two naming traps surfaced:
  `feat/posture-converge-staging` `d6448de1` is a different ref from `feat/posture-converge-foundation`
  `ea51fa5a`, which is what the worktree named `posture-converge-staging` actually holds; and
  `feat/test-123`, `worktree/brave-harbor-7ea0` and `worktree/quiet-river-d205` are three names for
  `fb483fc5`, all contained in `integration/modernization`, while `pr25-head` `fef2fcbd` is a second name
  for the tip of `feat/osquery-alerter-three-tier`. Rendering twice against the same base, at 04:46:29Z
  and 04:54:17Z, left C1, C2, E1 and E2 identical name for name; only `docs/posture-completion-report`
  moved, from B to D, when a concurrent session committed to it. The churn therefore lands on the groups
  the proposal already withholds. The classified list is at
  `~/workspaces/backups/2026-09-14T04-34-55.local-branch-classification.backup.txt` and the read-only,
  shellcheck-clean classifier that regenerates it is at
  `~/workspaces/backups/2026-09-14T04-34-55.local-branch-classifier.backup.sh`. The evidence was recorded
  on Todoist task `6hW4J6GVgVQ4w2c3` as comment `6hW4J7wmGvWcwFmV`. No GitHub write was made, since no
  open issue tracks branch hygiene. Still open: the operator approves or amends the exact groups, and the
  classifier is re-run first, because a merge landing later moves branches into C and grows the proposal
  past whatever was approved. Owed from the operator: (1) Read the classified list: less
  /Users/stephen/workspaces/backups/2026-09-14T04-34-55.local-branch-classification.backup.txt . Its
  final section names exactly what is being asked.; (2) Re-run the classifier first, because the
  repository moved twice during the inventory run and a later merge moves branches INTO group C, growing
  the proposal past whatever was approved:
  /Users/stephen/workspaces/backups/2026-09-14T04-34-55.local-branch-classifier.backup.sh . It reads
  only, takes an optional output path, and otherwise writes a fresh timestamped file under
  ~/workspaces/backups.; (3) Diff the fresh run against the 04-34-55 file and confirm C1, C2 and E1
  membership still matches before approving anything.; (4) DECIDE group C1, 17 branches: merged into
  origin/main, no worktree, remote head already gone. Approve or reject as a group. Lowest risk in the
  inventory.; (5) DECIDE group C2, 511 branches: merged into origin/main, no worktree, remote head stays
  on origin. Approve or reject as a group. Say separately whether the 511 matching remote heads should
  also be pruned; that is NOT part of this proposal.; (6) DECIDE group E1, 8 branches: absent from
  origin/main so each needs git branch -D, but every tip is reachable from another ref. Note that
  feat/scalebar-report-format, fix/issue-18-filevault-detection and part2-daemon-core survive only
  through their own origin branch, so weigh those three separately from the other five if that guarantee
  is too thin.; (7) DECIDE group E2, 14 branches, one at a time: no other ref anywhere holds these
  commits. Start with docs/nvim-acceptance-ledger (5001c94f, 9 commits ahead, last commit 2026-09-13),
  which is the only copy of a full day's work and the highest-value ref in the inventory.; (8) Do NOT
  decide groups B (20) or D (28) here. Both are checked out, so task 68 removes or keeps the worktrees
  first; six of group B is live work from tonight's concurrent sessions and must not be swept.; (9) Run
  the approved deletions yourself, or reply with the group letters and an agent can run exactly those.
  Nothing was deleted tonight.; (10) Review Todoist task 6hW4J6GVgVQ4w2c3 in project homelab, which
  carries the same approval gate and the evidence as comment 6hW4J7wmGvWcwFmV, and complete it once the
  groups are settled. Closed 2026-09-15: the operator approved the deletions and 28 stale branches were
  deleted that night, each on its own evidence (`git cherry` against `origin/main` plus a file-presence
  check on main); 8 branches remain, and Todoist task `6hW4J6GVgVQ4w2c3` was closed.

- [x] 68. Finish the worktree inventory and approved cleanup. There are 238 registrations: 28 under
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
  on a reading diet. Fable orchestrates, Opus implements, Sonnet does mechanical work. Recheck,
  2026-09-14: inventory only, nothing removed. A snapshot at 2026-09-14T04:40:09Z against main `f24aba51`
  found 55 registrations, not 238: one source checkout, 40 under `~/.herdr/worktrees/dotfiles`, 10 under
  `~/workspaces/dotfiles-worktrees`, three under `~/.paseo/worktrees/1sk17y2x`, and the sibling
  `dotfiles.workflow-job-search-luke-morrison-smith`. The 75 `.worktrees` and 19
  `dotfiles-agent-worktrees` registrations are gone, both directories now hold no worktree, and the
  missing-directory registration no longer exists: every registered path is present and
  `git worktree list --porcelain` reports zero `prunable` lines. All four named dirty worktrees were
  compared file by file and all four are superseded, so their uncommitted source no longer needs
  preserving. `pns-refactor-6-5`'s untracked `run_nag.rs` and its 13 tests merged as
  `pns/crates/pns-application/src/nag.rs`, declared at `lib.rs:72-73`. `pns-executable-deadline`'s
  `finish_bounded` split merged as `pns/crates/pns-adapters/src/process/bounded.rs:79` with its fixture
  and deadline tests alongside, and its tip `e97416d0` is only the merge commit of
  [dotfiles #435](https://github.com/webdavis/dotfiles/pull/435), so that branch carries no commit of its
  own. `herdr-smart-nav-clean-code`'s five-crate split merged as `f7d9208d`, `defedebb` and `d5e985f3`,
  which replaced the draft's `direction_to_chord` with the `Direction` enum. And
  `lights-implementation`'s 43 files all map onto the root `lights` workspace after the `a9b6e00d`
  relocation, 20 byte-identical and none absent, with main eight commits further along through
  `d224062c`. Seven of the eleven continuation branches have since merged (`9c99a880`, `73d70f7e`,
  `45a64bc4`, `79f28454`, `8f3150b1`, `1f934c7b`, `068e34e1`) and their worktrees are already
  unregistered. Four remain unmerged: `docs/nvim-acceptance-ledger` at `5001c94f`, whose worktree is gone
  and whose work survives only as the branch ref plus `stash@{3}`, `fix/nvim-mcp-boundary` at `295b84e3`
  behind [dotfiles #546](https://github.com/webdavis/dotfiles/pull/546),
  `fix/pns-daemon-fixture-children` at `c6b99b96`, and `feat/posture-funnel` at `0efb2119`. The four
  excluded-dirty branches merged too: `edfd18b7`, `e5b5f877`, `9432c139` and `4014ff49`. A new
  highest-risk finding replaces the old worry: `~/.herdr/worktrees/dotfiles/herdr-process-plan` holds 105
  untracked files and 10,243 lines implementing the whole five-crate `herdr-process` plugin, while
  `git ls-files` returns zero for both of its paths and `git log origin/main -- '*herdr-process*'`
  returns nothing, so `feat/herdr-process` being merged covers its commits and not this source. A
  `git worktree remove` there would destroy all of it, and preserving it was split to its own Todoist
  task because it must happen whether or not the cleanup is approved. The exact proposal groups all 55
  with no remainder: 11 protected because they are live right now (a herdr `open_workspace_id` and a
  running process cwd, one of them the head of
  [dotfiles #575](https://github.com/webdavis/dotfiles/pull/575)), two protected for open pull requests
  #546 and [dotfiles #51](https://github.com/webdavis/dotfiles/pull/51), one protected for unbacked
  source, 21 unmerged where a branch holds the commits so worktree removal loses nothing, 14 merged and
  removable of which seven need a named file copied out first, and five detached whose tips no branch or
  tag contains, so removal would make those commits unreachable. `posture-converge-staging`'s 1571 dirty
  files are all deletions against the merged `ea51fa5a`, an emptied working tree with nothing at risk.
  None of the 17 stashes is stranded by a worktree removal, and task 68c's stash is intact at
  `stash@{4}`. The registration count is moving: 51, then 55, then 57 within eleven minutes from
  concurrent sessions, so the recheck has to run again at approval time. The full classified list,
  per-group membership, the salvage file list and a recheck block are in
  `~/workspaces/backups/2026-09-14T04-40-09.worktree-inventory-task68.backup.txt`. Still owed: operator
  approval of the exact removal set, and the removal itself. Owed from the operator: (1) FIRST, before
  approving anything: preserve the herdr-process source, which sits on no git ref. Either commit it on
  its branch,
  `git -C ~/.herdr/worktrees/dotfiles/herdr-process-plan add dot_local/share/herdr/plugins/herdr-process test/fixtures/herdr-process && git -C ~/.herdr/worktrees/dotfiles/herdr-process-plan commit`,
  or copy it out:
  `cp -a ~/.herdr/worktrees/dotfiles/herdr-process-plan/dot_local/share/herdr/plugins/herdr-process ~/workspaces/backups/2026-09-14T04-40-09.herdr-process-plugin-source.backup/`.
  Tracked as Todoist 6hW4HCJxjWQvvvQM.; (2) Re-run the recheck block at the bottom of
  ~/workspaces/backups/2026-09-14T04-40-09.worktree-inventory-task68.backup.txt immediately before
  removing anything. It was 57 registrations at 04:47:02Z, up from the 55 in this inventory. Any path
  that comes back from `herdr worktree list` with an open_workspace_id, or from the `lsof -a -d cwd`
  line, is off limits whatever the file says.; (3) Decide group E1, the only set removable with no
  salvage step, seven worktrees:
  `git worktree remove ~/.herdr/worktrees/dotfiles/posture-converge-staging`, and the same for
  `herdr-smart-nav-clean-code`, `lights-implementation`, `pns-executable-deadline`, `lights-spec`,
  `uu-tooling-lanes-spec-amend`, and `~/workspaces/dotfiles-worktrees/render-coverage`. All seven are
  merged into f24aba51 and all their dirt is generated, disposable or superseded. `--force` will be
  needed on the dirty ones.; (4) Decide group E2, seven worktrees that need a file copied out first. The
  exact files are listed per worktree in the backup file: `dot_local/share/pns/INTEGRATION-NOTES.md`, six
  `slice-*-plan.md`/`slice-*-report.md` files across the four s9 worktrees,
  `docs/superpowers/plans/2026-07-21-s9-reland-slice-4-alerter.md`, and the three
  `docs/research/2026-06-2*.md` skill papers. Each was verified absent from every commit on every ref.
  The three docs/research papers are drop-in candidates for a commit, since docs/research is tracked on
  main.; (5) Decide group F, five detached worktrees whose tips no branch or tag contains (osq-review-4b,
  r1-review-4b, s61-g3, s61e-g3, rev4b-114). Either tag them first, for example
  `git tag archive/osq-review-4b e71776b8`, or say explicitly that the loss is accepted. The osquery and
  pns mutation-testing pins in the first two are the ones worth a look before deciding, since whether
  those asserts survived into main's suite was not established here.; (6) Decide group D, the 21 unmerged
  worktrees where a branch holds the commits. Worktree removal there loses nothing as long as no
  `git branch -D` follows, which is task 67's scope, not this one. The three dead-Paseo worktrees under
  ~/.paseo/worktrees/1sk17y2x are the strongest candidates: Paseo is dead and their tips are from June.;
  (7) Separately, decide the 24 stray agent scratch files sitting beside the worktree directories in
  ~/.herdr/worktrees/dotfiles (chain-553.sh, ledger-pr.sh, five merge-5\*.sh, ledger-pr.log, and sixteen
  commit-*/push-* logs). None is tracked by git and none belongs to a worktree.; (8) Leave groups A, B
  and C alone: 11 live worktrees, two behind open pull requests #546 and #51, and herdr-process-plan.
  Closed 2026-09-15: eight stale worktrees were removed that night, and `git worktree list` now shows
  four, the `main` checkout plus `fix/nvim-mcp-boundary` (behind open
  [PR #546](https://github.com/webdavis/dotfiles/pull/546)), `feat/herdr-process` (the uncommitted plugin
  source) and `refactor/herdr-smart-nav-clean-code` (an uncommitted crate split).

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
  exact-file cleanup approval. The full `chezmoi apply` ran and passed on 2026-09-15 and all four old
  binaries are still on disk: `ls` reports `~/.local/libexec/pns/pns`, `~/.local/libexec/uu/uu`,
  `~/.local/libexec/posture/posture` and `~/.local/libexec/lights`, each dated 2026-09-09. Still owed:
  the operator's approval of the exact files, then the trash pass.

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
- [x] The clean-home apply from PR #385, gates task 65. The full `chezmoi apply` ran and passed on
  2026-09-15, after a first attempt that night failed in `.chezmoiscripts/80-bootstrap-nvim.sh` (Mason
  could not install ansible-lint because `ensurepip` crashed on Homebrew's python@3.14 3.14.7, whose
  `pyexpat` expected a newer system libexpat than macOS 26.2 shipped); the operator upgraded macOS to
  27.0 and both `import pyexpat` and the apply then passed.
- [ ] The lamp drills, gates task 62. ONE OF FOUR DONE 2026-09-09: `bulk_read_latency` is measured and
  answered. Seven samples each against the operator's own bridge: the shipped bulk read of
  `/clip/v2/resource` runs a 210 ms median (127 min, 261 max), which is over the design's 150 ms bound,
  but the targeted strategy the plan named as its alternative measures WORSE, at a 267 ms median for the
  two-call room-plus-grouped form and 455 ms for the three-call form that adds scenes. Verdict: keep the
  bulk read, do not adopt targeted. The other three (`seven_commands_preserve_key_intent`,
  `brightness_floor_and_power`, `held_steps_match_isolated_steps`) still need the operator's eyes, and
  must run against the Kitchen or MBedroom rather than the Studio: `[lights.lamp.*]` routes loop, blocked
  and unread to four lamps including `3F - Studio - HCL3`, so pns animates the Studio while an agent is
  working and every brightness reading taken there is mid-animation. The runbook for the remaining three
  drills, plus a Studio key check, was written 2026-09-14 in
  [PR #599](https://github.com/webdavis/dotfiles/pull/599) (`docs(lights): lamp drill runbook`, merged)
  at `lights/docs/acceptance/lamp-drills.md`. The Kitchen and bedroom lamp drills and the Studio key
  check remain operator-run, so no tick. Still not run as of 2026-09-15.
- [x] Archive `webdavis/neovim-config` and remove `~/.config/nvim/.git`. Both verified complete on
  2026-09-12.
- [x] Approve the branch and worktree deletions, gates tasks 67 and 68. The operator approved them on
  2026-09-15 and they were executed the same night.
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
  reimplementing them. Reconciled 2026-09-14 against the memory record and the live machine, with `main`
  at `f24aba51`; the full classified table is
  `~/workspaces/backups/2026-09-14T04-34-55.pns-part2-drill-reconciliation.backup.txt`, and nothing was
  applied, deleted or reimplemented. The apply precondition is met: the operator's 2026-09-13 20:15 apply
  rebuilt the engine, so `~/.cargo/bin/pns` (20:50) carries the literal added that evening by `57fc3dd4`.
  Nine drills that the record left "pending after next apply" are now CLOSED on live evidence from
  `~/.local/state/pns/pns.db` and `pns doctor`: slice 6's L2 locked screen (decisions rows 1999 and 2000,
  `locked=yes surface=Away plan=banner:no,card:yes legs=mobile:delivered,hermes:delivered`, at 04:25:56Z
  and 04:32:27Z), slice 10's decision log, slice 11's moshi pairing (paired as dresden,
  `host_54f7755392cf4f3fa2f5695ccef7ca42`), slices 12 and 13's journal and catch-up replay (three
  `pns-return` `missed` events, seq 296, 1048 and 1078, one with a phone leg), slice 14's StopFailure
  (one hook in `~/.claude/settings.json`, six `claude/failed` events), item 21's denied (seq 161, a real
  refusal) and item 22's asked and plan-ready (21 asked, seq 78 `ExitPlanMode`). Drill 28 is a PARTIAL
  pass: Codex ran 1072 events through pns (758 blocked, 289 done, 25 asking, seq 448 to 1849) with all
  1072 hermes legs acknowledged, 19 banner deliveries on blocked and 22 phone cards on done and asking,
  and `~/.codex/config.toml` now carries a `trusted_hash` for both pns entries in `~/.codex/hooks.json`,
  which closes the drill ledger's last open item; what remains is a blocked Codex event answered on the
  phone, since 0 of 758 produced a `mobile` leg. Drill 29 CANNOT PASS as written: `run_after_62` still
  repoints both `moshi-hooks.ts` extensions at `/Users/stephen/.cargo/bin/pns` (regenerated 2026-09-09
  23:42 under moshi-hook 0.3.16), but that generation calls `helperBinary` only for debug replay, spawns
  nothing but `tmux`, and sends events over moshi's unix socket, so pns is out of pi and omp's path and
  no event has ever carried `agent='pi'`; filed as
  [the pi/omp gate decision](https://app.todoist.com/app/task/6hW4H5jRxrJp8XJG). NotHome is blocked
  upstream of itself: `pns home` reads `unknown`, the request-failed verdict (`home_report.rs:24`), while
  the router answers 200 at `https://192.168.1.1/` and 401 at `/proxy/network/integration/v1/sites`
  without a key, so the 2026-08-28 Home pass no longer reproduces and the credential or site id is the
  suspect ([task](https://app.todoist.com/app/task/6hW4H6c9J64h79vp)). The blocked, loop and daylight
  comparison has its code deployed (`pulse.rs` xy 0.3395/0.1379 and 0.1532/0.0475,
  `breathe_then_flare_cycle` at `lights/breath.rs:86`, 13 lamps routed with 4 on blocked and 4 on loop)
  and is owed the operator's eyes in daylight, on the Kitchen or MBedroom rather than the Studio. The
  total-runtime performance pass stays open on
  [its task](https://app.todoist.com/app/task/6hPxWVHM8pG4qgwp), whose 2026-09-13 native-probe medians
  measure the probe stage and not total runtime, and the Part 2 intent review stays open on
  [its task](https://app.todoist.com/app/task/6hPxWVwHGX9qFWpG), the 2026-08-31 grill session having
  covered the lights behaviours only. Slice 7's quiet window stays deferred: the template ships the key
  commented, the `quiet` table is empty, and an agent cannot read the deployed config. Configuration
  generation needs no rework, its byte-equality gate having passed inside `just test-rust` on main's Lint
  run 34803943215. Two incidentals: `pns doctor` reports the hermes gateway missing the `pns-recap` and
  `posture` routes, where only recap has a documented fallback
  ([task](https://app.todoist.com/app/task/6hW4H7XQ6fXPJc3G)), and the 28 `pns` crash reports in
  `~/Library/Logs/DiagnosticReports` are interrupted `pns setup` runs (26 SIGQUIT through `Hushed::drop`
  in `ask_hidden`), not engine crashes. The one `pns doctor` run this sweep needed sent a real test
  notification down every channel at about 22:35 local. Owed from the operator: (1) Drill 28's last leg:
  leave the desk (or lock the screen), provoke a Codex permission request, and answer the card on the
  phone. Every Codex approval so far landed on the banner (19 of 19), so the phone path for a Codex block
  is the one thing unproven. Confirm afterwards with `pns doctor` (the decision line should read card:yes
  with a mobile leg).; (2) Decide drill 29 (Todoist 6hW4H5jRxrJp8XJG): moshi-hook 0.3.16's generated pi
  and omp extensions no longer spawn `helperBinary`, so the pns presence gate cannot see them. Pick one:
  accept ungated pi and omp pushes and mark drill 29 closed as obsolete, ask moshi for a pre-send hook or
  socket shim, or retire the repoint and `run_after_62`'s first check. An agent will not touch moshi's
  own generated files either way.; (3) Fix the router probe before the NotHome drill (Todoist
  6hW4H6c9J64h79vp). With KeePassXC unlocked, run: curl -sk -H "X-API-KEY: \<UniFi :: API Key
  (dresden-udr)>" https://192.168.1.1/proxy/network/integration/v1/sites . A 200 means the site id or
  client query is at fault; a 401 means the key was rotated and needs re-pasting into the vault entry,
  then a full `chezmoi apply`.; (4) Once `pns home` reads Home again, run the NotHome drill: phone off
  wifi, `pns home` expects "NOT on the home network", wifi back on, Home again within about 15 seconds.;
  (5) Run the daylight lights comparison in daylight: blocked breathing, loop breathing, then the two
  side by side. Use the Kitchen or MBedroom lamp, never the Studio, because [lights.lamp.\*] routes loop,
  blocked and unread there and every reading taken on it is mid-animation.; (6) Schedule the two open
  program items when you want them: the total-runtime performance pass (Todoist 6hPxWVHM8pG4qgwp) and the
  /grill-me intent review over all of Part 2, not just lights (Todoist 6hPxWVwHGX9qFWpG).; (7) Optional,
  cheap: enable `quiet_hours` on [plugins.hue] if you still want it, which is the only thing standing
  between slice 7 and a drill.; (8) Decide on the missing hermes routes (Todoist 6hW4H7XQ6fXPJc3G):
  posture pages currently have nowhere to land, and the fix needs the encrypted hermes config edited, an
  apply, and `hermes gateway restart`.
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
- [x] Resolve the historical condenser-stall task
  [6hPCHVmfhXPM9FPM](https://app.todoist.com/app/task/6hPCHVmfhXPM9FPM). The named hook test still has a
  300 ms condenser deadline; production now bounds post-stdout waiting and cleans up process groups.
  Reproduce under representative load and record closure or fix the remaining cause. The audit found no
  demonstrated current failure and did not rerun the load drill. On 2026-09-14 the load drill ran and is
  recorded in `docs/research/2026-09-condenser-deadline-load-drill.md`. The two condenser deadline tests
  executed 390 times across four arms, including sixteen added busy loops on eight cores and a twenty-way
  concurrency arm at load average 55, with zero failures; the worst sandbox lifetime was 1885 ms against
  the suite's 5000 ms hard ceiling, and no orphaned stub process survived any run. The named test has no
  timing assumption to correct: both no-answer arms of `run_bounded` fall back to the reply, so an early
  expiry under load produces exactly the `detail` the test asserts. The historical mechanism, a forked
  grandchild surviving a single-process kill, was measured at the shell and is now reaped by the
  process-group guardian added in `60ea30cb` on 2026-09-08; the sandbox speed guard added in `bc361d86`
  on 2026-09-01 would fail any recurrence by name at five seconds instead of costing thirty in silence.
  Note that the polled post-stdout wait (`01434fa6`, 2026-08-12) predates the 2026-08-28 report and is
  therefore not the fix. Recommendation: close Todoist 6hPCHVmfhXPM9FPM on this evidence and keep both
  tests. The drill separately reddened
  `approval_payload::a_payload_at_the_cap_is_whole_and_is_still_submitted` in seven of ten full-binary
  runs under added load and
  `delivery_class::json_class_policy_crosses_the_real_mute_and_focus_edge_without_changing_hermes` in
  one; both are fixture ceilings running out rather than production deadlines, and the record recommends
  folding them into the sibling 6hPJVf2FJc3RHxqM item rather than opening new work here. Nothing was
  written to Todoist, so the closure itself is still owed. Full document:
  `docs/research/2026-09-condenser-deadline-load-drill.md`. Operator steps: (1) Read
  docs/research/2026-09-condenser-deadline-load-drill.md; its Verdict section is the whole decision and
  runs about a screen. (2) Close the Todoist task with `td complete 6hPCHVmfhXPM9FPM`, or say what
  further evidence you want first. Nothing was written to Todoist overnight on purpose: closing a task
  you filed, on a verdict you had not read, citing a scratchpad path, would have been deciding for you.
  (3) Decide where the two load-sensitive tests the drill found get filed. Recommendation: fold them into
  the existing ledger item 'Split and reconcile 6hPJVf2FJc3RHxqM', which already names ordinary hook
  fixtures inheriting the five-second payload deadline and already carries B105's approval-submission
  exit-code failure under load. (4) Answer the fixture-ceiling question: the hooks suite's HANG_LIMIT is
  5 s and the production default payload deadline in pns/crates/pns/src/hook_payload.rs:40 is also 5 s,
  so the fixture gives up at the same instant the production read it observes would. Your answer decides
  whether the approval-payload fix is a longer fixture ceiling, a shorter injected production deadline in
  that one test, or a smaller payload. (5) Tick the ledger item once the Todoist task is closed. It is
  left unticked only because its own done_means names that closure. Open questions: (1) Close
  6hPCHVmfhXPM9FPM on this local evidence, or hold it until the drill has also run on a GitHub runner?
  Recommended: close it. Every reading in the record is local, and the runner question has never been
  raised by an observed failure. (2) Where do the two other flakes get filed: folded into the sibling
  6hPJVf2FJc3RHxqM ledger item (recommended, same class), or a new Todoist task and ledger bullet? (3) Is
  a fixture ceiling equal to the production deadline it observes acceptable? HANG_LIMIT is 5 s and the
  default payload deadline is 5 s, and that equality is what loses the approval-payload test under
  contention. (4) Is the suite's 1000 ms advisory review line worth what it now costs? The two condenser
  sandboxes cross it under load while behaving correctly, and `allow_slow` cannot quiet them: reading
  `Drop for Sandbox`, the excuse lifts only the 5000 ms hard ceiling while the warning fires off
  `over_budget` unconditionally. Recommended: accept the noise; across ten runs those two sandboxes
  produced five lines while the rest of the suite produced between 8 and 38 per run. Closed 2026-09-15:
  the load drill is recorded in `docs/research/2026-09-condenser-deadline-load-drill.md` and that Todoist
  task was closed the same day.
- [ ] Split and reconcile [6hPJVf2FJc3RHxqM](https://app.todoist.com/app/task/6hPJVf2FJc3RHxqM). Ordinary
  hook fixtures still inherit the five-second payload deadline and need bounded fixture inputs. The
  Hermes redirect fixture already consumes the complete request and keeps its socket until disconnect;
  commit `f3b5a21b` records that repair. Verify historical closure without rebuilding it. Keep production
  deadlines distinct from fixture ceilings and follow the repository's test-runtime policy. Explicitly
  include B105's approval-submission exit-code failure, historically `0` instead of `42` under load. The
  September 7 disposition leaves it unresolved after #383 and #441; #378 closed unmerged. A bounded
  fixture is not proof of closure, and this audit did not establish a current reproduction.
- [ ] 87. Make the pns nag delivery test deterministic. Continuous integration for
  [PR #629](https://github.com/webdavis/dotfiles/pull/629) failed once on
  `nag_delivery::the_daemon_really_fires_the_nag_and_really_drops_it_when_the_marker_is_there`, which saw
  two cards where it expects one, although that pull request touched no pns code; a rerun passed.
  Reproduce it under load before changing anything, since a second card is either a real double fire or a
  fixture reading one delivery twice.
- [ ] Retain Moshi image recap cards as blocked on transport, not ready to build. The recorded reopening
  conditions are a homelab HTTPS image host, an upstream upload interface, or a documented data-URL path.
  An operator-approved single-card probe must establish actual image display before treating data URLs as
  supported. Revisit usefulness before adding a renderer; the proposed recap duplicates Discord. Source:
  `~/.claude/pipeline/slices/design-moshi-image-cards.md`. Re-checked on 2026-09-14 and one reopening
  condition now holds, so this entry's premise is wrong. Moshi's notification documentation carries a
  live upload interface, `POST https://api.getmoshi.app/api/v1/images/upload`, authenticated with the
  same push token pns already reads from `[plugins.mobile] token`; the route answered `401 Invalid token`
  to a bogus bearer against a `404` on a control path, and no real credential was used. The homelab-host
  condition still fails and is now moot, because the documentation says Moshi "passes it to Expo as a
  rich-content attachment", a server-side fetch that a tailnet-only address cannot serve and that a data
  URL cannot satisfy either, and the node still advertises no Tailscale Funnel capability. moshi-hook
  itself still exposes no `upload` subcommand, and none of releases 0.3.17 through 0.3.22 mentions images
  or uploads; the interface is on the web API that pns already posts to. The remaining blockers are
  therefore value, unchanged (the recap card duplicates the Discord recap, and `data` carries one `type`,
  so an image card trades away the herdr pane deep link), and the card-ownership refactor, confirmed
  still required because `replay_missed` spawns the detached recap child and then posts the card in the
  calling process a few instructions later. The recorded data-URL probe is superseded: the app's own
  notification-settings image test action settles display with no code and no token handling. The
  verdict, the evidence with file and line references, seven explicitly recorded assumptions and an
  operator-run upload probe live in `docs/research/2026-09-moshi-image-cards.md`. Recommendation: do not
  build, rewrite this entry as transport-available and unbuilt on value, and answer the value question.
  Also noticed while checking: moshi-hook is six releases behind (0.3.16 installed, 0.3.22 in the tap).
  Full document: `docs/research/2026-09-moshi-image-cards.md`. Operator steps: (1) Read
  docs/research/2026-09-moshi-image-cards.md, specifically the Verdict and the seven assumptions in
  "Assumptions made in the operator's place"; assumption 1 (whether a documented web endpoint counts as
  "an upstream upload interface", when moshi-hook still has no upload subcommand) is the one that decides
  whether this entry reopens at all. (2) Tap the image test action in the Moshi app's notification
  settings on `mister`. Zero code, no token, one tap. It answers whether a rich image notification
  displays on that device at all, and everything else is moot if it fails. (3) Decide the value question:
  is one saved tap into Discord worth two pull requests plus moving recap card ownership from the hook
  process into the detached `pns recap` child? The technical answer is now yes-it-can-be-built; the
  2026-09-01 value answer was no and this pass did not overturn it. (4) Rule on the token's path: may the
  Moshi token ride an `Authorization: Bearer` header on the upload leg?
  `pns/crates/pns-adapters/src/destinations/moshi.rs` currently states the rule as the request body "and
  nowhere else". A no here leaves the task blocked and needs nothing further. (5) If you want the real
  round trip proved, run the two-command probe at the end of the research document while awake. It spends
  one of ten hourly uploads and sends one real card to your phone. (6) Rewrite the ledger entry either
  way. "Blocked on transport" is now false and the two honest replacements are "transport available,
  unbuilt because it duplicates the Discord recap" and "closed, will not build". (7) Separately from this
  task, consider `brew upgrade moshi-hook`: 0.3.16 is installed and 0.3.22 is in the tap, and 0.3.20
  through 0.3.22 carry Pi agent detection, Codex named-session reset fixes and Herdr sidebar controls
  that touch this machine's daily path. Open questions: (1) Does the Moshi app's own image test action
  actually display a rich image notification on `mister`? Everything below is moot if it does not. (2) Is
  one saved tap into Discord worth two pull requests and moving recap card ownership into the detached
  recap child? This is the whole remaining decision, and it is a value call rather than a technical one.
  (3) May the Moshi token ride an `Authorization: Bearer` header on the upload leg, amending moshi.rs's
  "request body and nowhere else" rule to "body or Authorization header"? A no keeps the task blocked.
  (4) Do you want the real upload-then-webhook round trip proved with your token, and if so may it happen
  while you are awake rather than overnight? (5) Does this ledger entry get rewritten as
  transport-available-and-unbuilt-on-value, or closed outright as will-not-build? (6) Does a documented
  web API endpoint satisfy the recorded condition "an upstream upload interface", given that moshi-hook
  itself still has no `upload` subcommand? Read narrowly, condition 2 fails and the entry stands exactly
  as written. (7) Should the multipart body be hand-built through the existing `send()` (about twenty
  lines, no feature change), or should ureq's `multipart` feature be enabled despite living in its
  `unversioned` module, whose stated policy is that breaking changes there will not produce a major
  version bump? (8) Unrelated to the verdict: upgrade moshi-hook from 0.3.16 to the tap's 0.3.22 now, or
  leave it pinned?
- [ ] 78. Decide whether a recap card on the phone carries an image, and build it only if the answer is
  yes (operator ruling 2026-09-15, low priority). The research above settled the technical half: the
  upload interface exists and the app's own image test action proves display, so what is left is the
  value call and the card-ownership refactor an image card would need. Nothing starts until that decision
  is recorded here.
- [ ] Preserve the pns refactor plan's explicitly carried-forward behavior work (section 7). B1 needs a
  reviewed Hue bridge certificate/identity-pinning design; `pns/crates/pns-adapters/src/hue/bridge.rs`
  still disables certificate verification. Define enrollment, changed-certificate handling and recovery
  before changing that behavior. Designed on 2026-09-14 in
  `docs/superpowers/specs/2026-09-14-hue-bridge-certificate-pinning-design.md`, unapproved and unbuilt;
  verification behavior is unchanged. The live bridge was measured: its certificate is
  `CN=<bridge id>, O=Philips Hue, OU=BSB003`, issued by `CN=root-bridge`, valid to 2038, with NO
  subjectAltName, and the unauthenticated `/api/config` reports the same bridge id, so chain verification
  against the published Hue root succeeds (verified locally with `openssl verify`) while name
  verification is impossible on any modern stack. ureq 3.4.x exposes no custom-verifier hook and couples
  native-tls's two danger flags to one, so verification requires a custom rustls verifier behind a
  connector supplied through `Agent::with_parts`; a scratch prototype built only from ureq's public
  `unversioned::transport` items accepted the matching pin (HTTP 200) and refused a one-bit-flipped pin
  with our own message intact, over a loopback fixture (the agent sandbox blocked the live LAN handshake,
  and the stock-ureq control failed the same way, so a live handshake remains an acceptance gate). The
  recommendation is to pin the bridge's own certificate fingerprint rather than the Hue root plus
  identity: one `[plugins.hue] certificate = "sha256:..."` key that is a config refusal when hue is
  enabled without it, one `UreqBridge::new` constructor replacing seven struct literals, a print-only
  `pns lights enroll` that refuses when the certificate common name and the reported bridge id disagree,
  and a single permanent mismatch report through the 2026-09-08 delivery-failure path with `pns doctor`
  showing the pin state. lights (`lights/crates/lights-adapters/src/hue.rs`) and the UniFi client
  (`pns/crates/pns-adapters/src/unifi/client.rs`) also disable verification and are out of this design's
  scope, filed as follow-ups. Ten assumptions and seven open questions are listed for the operator; the
  design waits on their read. Full document:
  `docs/superpowers/specs/2026-09-14-hue-bridge-certificate-pinning-design.md`. Operator steps: (1) Read
  docs/superpowers/specs/2026-09-14-hue-bridge-certificate-pinning-design.md and answer the seven open
  questions, starting with approach A versus B versus C. (2) Decide whether a missing pin refuses at
  config parse (recommended) or warns for one release, since that choice is what the first pull request
  encodes. (3) Decide whether the pin is committed in dot_config/pns/config-values.toml (recommended) or
  stored as a KeePassXC attribute on the "OpenHue :: API Key (hue-bridge-pro)" entry. (4) Decide whether
  lights gets the same change in this wave and whether the UniFi router client's unverified TLS becomes
  its own design task; its credential is a router API key. (5) After approval and after pull requests one
  and two land, run `pns lights enroll` on dresden with the bridge reachable, check that the printed
  certificate common name equals the bridge id, and paste the `certificate = "sha256:..."` line into
  dot_config/pns/config-values.toml before pull request three turns pinning on. (6) Run the full
  `chezmoi apply` yourself once the config carries the pin; agents do not apply, and the shipped template
  is regenerated with `just pns-config-render`. Open questions: (1) Approve approach A (pin the bridge's
  own certificate fingerprint) over B (Hue root CA plus bridge identity) and C (keep the current behavior
  and record the risk)? (2) Fail closed on a missing pin at config parse, or one release of
  warn-then-refuse? (3) Does the pin live in the committed dot_config/pns/config-values.toml, or as a
  KeePassXC attribute beside the bridge address and key? (4) Should `pns lights enroll` accept the bridge
  id out of band, read off the device's own label, so an impostor present at enrollment time is refused
  rather than merely made harder? (5) Does lights get the same pinning change in this wave, and is the
  UniFi router client's unverified TLS worth its own design task now? (6) Is `pns lights enroll` the
  right command name, or should the bridge's identity live under `pns doctor` and a flag? (7) If approach
  B is chosen after all: is a Philips Hue root certificate copied from a third-party mirror acceptable as
  a trust anchor, given that this bridge's real certificate verifies against it?
- [ ] Resolve the related B6/B20/B39 hook design: the answered-wait race, when `AskUserQuestion` should
  arm a waiting indicator and what its notification contains, and alerts for sandbox network approval
  requests. The `AskUserQuestion`-specific `asked` wiring runs after the tool completes, and network
  permission waits remain explicitly uncovered. Inspect current harness events and agree behavior before
  changing hooks; a prompt-only guess does not establish an actual permission wait. A design is written
  at `docs/superpowers/specs/2026-09-14-hook-wait-events-design.md` (2026-09-14), naming per harness
  event whether a real wait is observable, against the Claude Code 2.1.270 event vocabulary and input
  schemas read out of the installed bundle. It reverses two of the three premises. `PermissionRequest`
  already fires for `AskUserQuestion` and `ExitPlanMode`, because both declare `requiresUserInteraction`,
  so the wait is already armed before the dialog is drawn; 26 of the 35 live `claude` `blocked` events in
  `~/.local/state/pns/pns.db` are a question or a plan, and the nag already nudges them. What is broken
  is the opposite of B20's filing: the `asked` and `plan-ready` arms on `PostToolUse` fire after the
  answer and, being in `LAMP_BLOCKED`, re-arm the wait, racing the asynchronous `resolved` that should
  end it, and the `asked` card recites the operator's own choice back to them. B6 needs no sidecar:
  2.1.270 added `ElicitationResult`, which is the elicitation's answer signal and carries
  `elicitation_id`, and `PostToolUse` is the answer for a dialog-shaped tool. The recommendation is three
  declaration edits routing both `PostToolUse` matchers and a new asynchronous `ElicitationResult` entry
  to the existing `pns hook resolved`, deleting the `plan-ready` arm and state word, plus an End that
  refuses to remove a marker armed after its own moment, claimed by rename per
  `pns/docs/decisions/0001-ownership-by-rename-not-by-unlink.md`. No new state file, no new arm. Ten
  behaviors are listed to pin test-first. B39 is designed and deliberately not built: sandbox network
  dialogs reach the dialog host directly with no `PermissionRequest`, their only hook-visible trace is a
  `Notification` whose type defaults to `permission_prompt` so no matcher can separate it from a tool
  approval, its payload cannot name the host, and no `sandbox` block exists in the managed template or
  the live settings, so exposure on dresden is currently zero (though flag and policy settings can open
  it without a local change). The interim wiring is written out in full. The design waits on the
  operator's read: five open questions, including whether `denied` should keep arming a wait, whether
  `SubagentStop` should end a subagent's, whether to build B39 now, and whether to ask upstream for a
  distinct notification type. Full document:
  `docs/superpowers/specs/2026-09-14-hook-wait-events-design.md`. Operator steps: (1) Read
  /private/tmp/claude-501/-Users-stephen-workspaces-Ivy-webdavis-dotfiles/1bf0ef19-e242-4ea5-8746-60cb679ebafc/scratchpad/docs-wave/hook-wait-design.md
  and confirm it lands at docs/superpowers/specs/2026-09-14-hook-wait-events-design.md. (2) Confirm or
  reject the six assumptions in 'Assumptions made in the operator's place', especially assumption 1 (a
  post-answer event clears a wait instead of carding you) and assumption 5 (B39 designed, not built). (3)
  Answer open question 1: should `denied` stay in LAMP_BLOCKED, or be routed as an observation so a
  classifier refusal stops colouring a lamp that claims someone is waiting? (4) Answer open question 2:
  add a fifth declaration routing `SubagentStop` to `pns hook resolved`, so a subagent's approval stops
  holding the parent session's lamp until the parent's Stop? (5) Answer open question 3: build the B39
  interim wiring now (about an hour, text-allowlisted), or wait for the sandbox to be switched on or for
  a distinct notification type upstream? (6) Decide whether to file one upstream request asking that the
  sandbox_network_access dialog get its own notification_type, which is what would make the B39 alert
  robust rather than text-matched. (7) If the design is approved, schedule it as one small PR: three
  declaration edits in private_dot_claude/modify_settings.json, the plan-ready deletion, the marker End
  change, and the ten pinned behaviors. Note that the declaration change only takes effect after a full
  `chezmoi apply`. Open questions: (1) Does `denied` belong in LAMP_BLOCKED? PermissionDenied fires after
  the auto-mode classifier refused a call on its own, so nobody is waiting on an answer, yet the word
  arms a wait only the session's next event ends. Recommendation: route it as an observation, keeping the
  card and dropping the lamp. One live `denied` event exists, so this is nearly theoretical, and it was
  not in the three filed rows. (2) Should `SubagentStop` end a subagent's wait? Today a subagent's
  approval arms the parent session's marker and `resolved` deliberately skips subagent batches, so it
  holds until the parent's own Stop. Recommendation: yes, as a fifth declaration routed to `resolved`,
  which shortens the wait without making a subagent approval invisible. (3) B39: build the
  text-allowlisted Notification arm now, or wait? Recommendation is to wait, with the trigger being
  either switching the sandbox on locally or Claude Code giving the dialog its own notification type. The
  full interim wiring is written out if you would rather have the alert standing. (4) Should the
  sandbox-network gap be reported upstream? A distinct `notification_type` for `sandbox_network_access`
  would make every option robust instead of text-matched. Worth one issue, and it is your call whether to
  file it. (5) Is the `[lights]` gate on arming a wait marker still right? It is the only reason the
  state-based discriminator for B39 cannot be the recommendation, because on a machine with no lamps
  configured the dedup read always finds nothing. Nothing needs changing today.
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
  contains it. Operator deployment and live pane-move acceptance remain: run drill one in
  [`docs/acceptance/nvim-acceptance-drills.md`](acceptance/nvim-acceptance-drills.md).
- [ ] Resolve B97's Zig tooling decision: supply a working, compatible Zig/ZLS pair or remove the unused
  ZLS configuration after that decision. At audit time Zig reported `0.12.0-dev.3158+1e67f5021`, Mason
  ZLS reported `0.15.1`, and `zig env` failed to locate its installation. The Zig neotest adapter is also
  absent; decide whether that language workflow is wanted before adding it.
- [x] Preserve B107's deferred JavaScript test-discovery responsiveness work. Caching shipped, but cold
  parsing remains synchronous; the recorded 7.4-second UI stall was not remeasured in this audit. Source
  for B97/B103/B107: `~/.claude/pipeline/backlog-consolidated-2026-09-02.md`. B92's canonical-hour,
  rainbow and X11 colour cycles were deliberately excluded, not missed implementation. On 2026-09-14
  `a849b5ae` on `fix/nvim-js-discovery-cold-parse` remeasured the stall and bounded it: a headless
  harness over 500 files of 129 KB spent 11,290 to 12,062 ms in the first adapter's synchronous
  `is_test_file` sweep, matching the recorded 7.4 s figure and its 22.2 s total. Since all four query
  shapes capture a string fragment compared against `node:test`, and that text is a slice of the file's
  own bytes, `imports_node_test` now answers a file that never names it from a plain byte-literal find
  and caches that answer the way it caches a parsed one; the first-adapter sweep fell to 157 ms (a
  realistic 10.8 KB tree fell from 1,180 ms to 39 ms), while a tree whose files really do import
  `node:test` still parses each one (656 ms over 500 realistic files, against 706 ms before), since only
  a parse tells a real import from a mention in a comment. Making discovery asynchronous was rejected:
  the pinned neotest client filters through a plain synchronous loop and calls the predicate from
  contexts that are not nio tasks. The existing parse-count case in
  `dot_config/nvim/tests/neotest_spec.lua` gained a third file version naming `node:test` nowhere and
  three mutants of the new branch die against the suite; `just test-nvim`, `just test-unit` and
  `just lint-check` each passed at exit 0. Independent review found the committed "0.024 ms each read and
  scanned" comment understated the real cost and a test-rename commit was mistyped as `fix`; both fixed,
  the comment corrected to "0.08 ms" against a 20-iteration measurement (`1944743c`) and the commit
  retyped to `test(nvim)` through a safe rebase (`29037025`). Assumptions: "bounds the cold path" means
  removing a parse only where it cannot change the answer, since a tree that genuinely imports
  `node:test` still pays one parse per file version; the byte-literal prefilter is behavior-preserving
  because every query pattern captures a `(string_fragment)` whose text must equal the literal bytes,
  which a mutation-tested comparison confirms empirically; no new spec file was added since the existing
  suite already pins both arms of the branch by mutation; the measurement harness at `/private/tmp/b107`
  is not committed, a one-off tool rather than a gate.
  [PR #589](https://github.com/webdavis/dotfiles/pull/589) (`fix/nvim-js-discovery-cold-parse`, merged
  `2d9c6e188b2db896c7be78ec89dabf412a075bcf`). Operator: run a full `chezmoi apply` to deploy
  `~/.config/nvim/lua/plugins/neotest.lua` (no template touched, no KeePassXC or manifest involved).
  Closed 2026-09-15: the full apply ran and passed, and `diff` reports
  `dot_config/nvim/lua/plugins/neotest.lua` identical to the deployed
  `~/.config/nvim/lua/plugins/neotest.lua` at exit 0.
- [ ] Preserve B95's Rust neotest discovery and duplicate-client follow-up. Rust, Java and Elixir
  adapters are absent from the configured adapter list; verify Rust's intended workflow before calling
  language coverage complete. Record the Java/Elixir disposition against plan task 46b, step 2, which
  permits withholding adapters that fail their verification. Their absence alone is not an instruction to
  add them. Existing neotest infrastructure and other language adapters remain implemented. Researched
  2026-09-14 and written up as `docs/research/2026-09-rust-neotest-disposition.md`. Rust's intended
  workflow is spec 5.3's rustaceanvim adapter, which is absent from the configuration: none of the eight
  configured adapters accepts a `.rs` path, so `<leader>tt` in a Rust buffer reaches no runner and the
  language row is not complete. The adapter itself is now proven to work here: against rustaceanvim
  `a8c4f9af` on a scratch two-test crate it discovered
  `lib.rs[file] tests[namespace] adds_two[test] adds_three[test]` and held that tree from 3.1 s through
  36 s. B95's blocker (b) is root-caused as a readiness race, not a broken adapter.
  `experimental/runnables` passes through three phases after `client.initialized` goes true: one
  `cargo check --workspace` runnable for the first 0.8 s, which the adapter turns into a file-only tree
  whose `build_spec` returns nil so `<leader>tt` silently does nothing; zero runnables from 1.2 s to 2.4
  s, which makes the adapter call `neotest.lib.positions.parse_tree` with an empty list and RAISE at
  `positions/init.lua:337`; and the correct five runnables from 2.8 s onward. Nothing retries: the pinned
  neotest re-discovers a file only on `BufAdd` and `BufWritePost` (`client/init.lua:400,463`) while its
  `BufEnter` handler only sets the focused file, so a tree captured before rust-analyzer is ready
  survives any amount of polling, which is exactly what B95 measured as zero over 240 s. Blocker (a) is
  still live (`lsp.lua:209` carries `automatic_enable = true` with `rust_analyzer` in `ensure_installed`)
  and the collision is structural: nvim-lspconfig's client is named `rust_analyzer` and rustaceanvim's is
  `rust-analyzer`, so neither stands down for the other; rustaceanvim's README warns against exactly this
  pairing, and B95's proposed `automatic_enable = { exclude = { "rust_analyzer" } }` is the form
  mason-lspconfig documents at `doc/mason-lspconfig.txt:124-129`, with this same server as its own
  example. Java is WITHHELD under plan 46b step 2: its JUnit 5 verification cannot be attempted, because
  `/usr/bin/java` is the macOS stub with no runtime, `mvn` and `gradle` are absent, no JDTLS-based server
  is configured, the `java` parser is not in `treesitter.lua`, and no Java source or Maven/Gradle project
  exists anywhere; spec 5.3's Java row also understates the cost, since `rcasia/neotest-java` (`71354dd`,
  2026-09-05) requires a Java Development Kit, a build tool, `nvim-jdtls` or `nvim-java`, and the parser
  rather than one filetype-lazy pin. Elixir is WITHHELD on the same rule: `elixir`, `mix` and `erl` are
  absent and undeclared, and `jfpedroza/neotest-elixir` has had no commit since `a242aeb` on 2025-01-19.
  A separate live defect surfaced and needs its own task: Mason's rust-analyzer is the 2025-12-21 build
  while its registry offers 2026-09-07, `ensure_installed` never upgrades an installed package, and that
  build calls `cargo metadata --lockfile-path`, which cargo 1.98.1 rejects, so every Rust buffer loads
  through a `--no-deps` fallback today. Rust is not untested from Neovim meanwhile: overseer's `cargo`
  template ships `cargo test` and its `just` provider reaches `just test-rust`. The document closes with
  three readiness options (a local `discover_positions` wrapper in `plugins/neotest.lua`, accept the raw
  adapter and save-to-retry, or withhold the row like Java and Elixir), recommends the wrapper, and
  leaves seven questions for the operator. Nothing was installed and no code was changed. Full document:
  `docs/research/2026-09-rust-neotest-disposition.md`. Operator steps: (1) Read
  `docs/research/2026-09-rust-neotest-disposition.md` and pick one of its three readiness options for the
  Rust neotest row. The document recommends option 1, a roughly fifteen-line copy-and-override of
  rustaceanvim's `discover_positions` in `dot_config/nvim/lua/plugins/neotest.lua`, using the same
  pattern the file already applies to vitest. (2) Accept or reject the
  `automatic_enable = { exclude = { "rust_analyzer" } }` trade in `dot_config/nvim/lua/plugins/lsp.lua`.
  It makes Rust the one server not covered by the uniform `automatic_enable = true`; rejecting it forces
  option 3 (withhold the Rust row). (3) Refresh the deployed rust-analyzer, either
  `:MasonInstall rust-analyzer` for the 2026-09-07 build the registry already offers, or
  `rustup component add rust-analyzer` for a toolchain-matched server, then ask for the finding-3 timing
  probe to be re-run. This is a package install, so an agent must not do it. (4) Confirm the Java and
  Elixir withholding, so spec 5.3's two adapter rows and plan task 46b step 1 can be amended in a later
  documentation slice (struck, or marked withheld with this document as the reason). (5) Decide whether
  the stale Mason rust-analyzer becomes its own ledger task, and whether Mason packages get a deliberate
  pin-and-refresh policy rather than the current install-once posture. (6) Decide whether rustaceanvim's
  raised assertion (its two early returns call `lib.positions.parse_tree` with an empty list) gets
  reported upstream as an issue. Open questions: (1) Which readiness option for the Rust neotest row: 1
  (a local `discover_positions` wrapper), 2 (accept the raw adapter and live with save-to-retry), or 3
  (withhold the row like Java and Elixir)? The recommendation is option 1. Nothing else in this task can
  proceed until this is answered. (2) Is the mason-lspconfig `exclude` trade acceptable? It makes Rust
  the one server outside `automatic_enable = true`. If not, rustaceanvim cannot be used for the Rust row
  at all and option 3 is forced. (3) Does "language coverage complete" mean neotest specifically, or is
  overseer's `cargo test` plus `just test-rust` enough? Taking the second reading closes the whole Rust
  row today with no plugin, no pin and no decision. (4) Should the stale Mason rust-analyzer become its
  own task? It is a live defect on every Rust buffer, independent of neotest, and nothing in the
  configuration will ever upgrade it. Related: does this repository want Mason packages pinned and
  refreshed deliberately, or is install-once-never-update the accepted posture? (5) Do spec 5.3's Java
  and Elixir rows get struck, or marked withheld with this document as the reason? And should the Java
  row be corrected regardless, since it describes one filetype-lazy pin where the adapter needs a Java
  Development Kit, a build tool, a language server plugin and a tree-sitter parser? (6) Should the raised
  assertion be reported upstream to rustaceanvim? Its two early returns call
  `neotest.lib.positions.parse_tree` with an empty list, which raises in the pinned neotest, and the
  reproduction is one line. (7) Is a Java or Elixir project anywhere on the horizon? If yes, both
  dispositions should be filed as deferred with a named trigger rather than withheld, and each needs the
  toolchain declaration decisions that go with it.
- [x] Reconcile B96's first-use parser readiness. Go is omitted from the preinstalled parser list,
  missing-parser installation is asynchronous, and the Go adapter returns without discovery when its
  parser is absent. Verify the first test request in that state and provide a working first-use path
  through supported integration. The historical failure was not reproduced during this audit. On
  2026-09-14 `07304c85` on `fix/nvim-go-parser-readiness` reproduced it: against a clone of the live
  Neovim data directory with the `go` parser and its query directory removed, the first `<leader>tt` in a
  scratch Go module discovered no tests and notified nothing, because neotest-golang's readiness guard
  reads a `pcall` status where `vim.treesitter.language.add` answers with a value, and on Neovim 0.12.5
  that call returns nil rather than raising. `go` joined the core parser list in
  `dot_config/nvim/lua/plugins/treesitter.lua`, which the apply-time bootstrap installs, and
  `<leader>tt`, `<leader>tf`, `<leader>ta` and `<leader>ts` now route through one `neotest_with_parser`
  helper that installs and waits (bounded at 30 s) for the buffer's language parser before neotest's
  client is touched at all, since that client registers its discovery autocmds on its first API call.
  Re-measured from the identical starting state: the first request installed the parser in 4.3 s and
  discovery returned the module's test. One case in `dot_config/nvim/tests/neotest_spec.lua` pins the
  gate, mutation-verified three ways; `just test-nvim`, `just lint-check` and `just test-unit` each
  exited 0. Independent review found `<leader>ta` went through the parser gate with no test pinning it,
  and that the implementer's own report understated the branch (six non-merge commits, four files, not
  one); the gate coverage was fixed with a new directory-run case (`684be131`); the understated report
  could not be corrected in-repo (no ledger-text field on the return schema), so the true commit list is
  `07304c85`, `f3a0e136`, `ee930fd7`, `f455c3cc`, `b111c128`, `684be131`, merged with `fb714403`, across
  `dot_config/nvim/lua/plugins/neotest.lua`, `dot_config/nvim/lua/plugins/treesitter.lua`,
  `dot_config/nvim/tests/neotest_spec.lua` and `graphify-out/graph.json`. Assumptions: the readiness path
  is the preinstalled list plus a request-time wait rather than deferring the first run, since deferral
  measured as no message at all; the buffer's own language is the signal for a request, so a request from
  a different-language buffer in a Go module is covered by the preinstalled-list half, not the gate;
  `<leader>to` and `<leader>tS` were left ungated, since neither discovers; the agent used `rm -rf` on
  its own scratch harness at `/private/tmp/gp` instead of `trash`, disclosed as a rule violation touching
  nothing outside that directory. [PR #590](https://github.com/webdavis/dotfiles/pull/590)
  (`fix/nvim-go-parser-readiness`, merged `15e885bc`). Operator: run a full `chezmoi apply` (the nvim
  bootstrap re-runs and builds the `go` parser synchronously; that build measured 4.3 s in the probe).
  Closed 2026-09-15: the full apply ran and passed, `diff` reports
  `dot_config/nvim/lua/plugins/treesitter.lua` identical to the deployed
  `~/.config/nvim/lua/plugins/treesitter.lua` at exit 0, and `~/.local/share/nvim/site/parser/go.so` is
  present.
- [x] Correct B100's stale pane-selection contract in the canonical Neovim spec/plan: the owned helper
  uses `agent_pane(on_pane)`, not a synchronous returned pane identifier. Preserve cancellation/refusal
  behavior. This is documentation reconciliation, not a missing helper implementation. On 2026-09-13
  `3d92ca3e` on `docs/nvim-agent-pane-contract` rewrote the spec's 7.2 lookup paragraph, its 7.4
  interface bullet and the plan's PR 11 interface line against `M.agent_pane` in
  `dot_config/nvim/lua/custom_api/herdr.lua`; `just ship` passed on the second run (exit 0, 3m12s; the
  first run hit the pns `security_sound` fixture's 900 ms child guard under load, fixed separately) and
  [PR #556](https://github.com/webdavis/dotfiles/pull/556) merged into `main` on 2026-09-13. Verified
  2026-09-14: `origin/main` carries `3d92ca3e` under the merge of
  [PR #556](https://github.com/webdavis/dotfiles/pull/556), and both the spec (line 1326, "answers
  through the callback, never a return value") and the plan (line 867, `herdr.agent_pane(on_pane)`)
  describe the callback contract, so the box is ticked.

### Recover the remaining design from PR #24

- [x] Review #24 before deciding its disposition. Recovered on 2026-09-13 from head `2202dcbf`; four
  historical documents and thirteen upstream snapshots passed all 17 recorded hash checks. Retain its
  independent alert, restricted evidence and advisory-only requirements in the section below. Supersede
  the digest trigger and obsolete recipes in a smaller reviewed plan after the security decisions are
  settled. #24 remains open; do not merge its old instructions unchanged.
- [ ] Reconcile the approval interface separately: Butters tap-to-approve scoped to pending findings and
  the `/osquery allow|deny|list` Hermes skill. Verify the current posture command and trust contracts;
  investigation must not grant the analyst approval authority. On 2026-09-14 the reconciled scope was
  written to `docs/superpowers/specs/2026-09-14-osquery-approval-authority-design.md`. Verified on the
  host: `posture allowlist add|deny|list` is the writer, it refuses a label with no installed launchd
  agent, pins the plist hash with SHA-256, writes the chezmoi source and then refreshes the root-owned
  manifest; the allowlist is manifested (`0600 501`) and `allowlist_verdict` spends a vouch before it
  ever suppresses, so the 2026-07-26 option B plus D-prime both shipped, while option E did not. Nothing
  in posture, pns or the state tree has a pending-findings concept. Butters is now a Hermes profile (a
  computer-use LLM agent), not a bespoke bot; all three live Hermes webhook routes are `deliver_only`, so
  the Discord surface a page lands on cannot carry buttons without modifying third-party code; Hermes's
  own `ExecApprovalView` is session-scoped at 300 s and fires only for `DANGEROUS_PATTERNS`, which
  `posture allowlist add` does not match; the `terminal` toolset is enabled with `backend: local` and
  `sudo -n true` succeeds, so every Hermes agent already holds unprompted approval authority and the
  trust boundary is open rather than merely undesigned. Recommendation: authority stays in
  `posture allowlist`, narrowed to a derived pending set (results log plus the deployed allowlist plus a
  live launchd capture plus a decline marker) and announcing every grant and denial through pns, with one
  deterministic chezmoi-managed Hermes plugin command as the only new surface, a `pre_tool_call` shell
  hook as a labelled speed bump, `Interaction::AwaitDecision` left unimplemented as the stated direction,
  and PR #24's agent-mediated skill and bespoke discord.py bot rejected. Two prerequisites surfaced:
  `DISCORD_ALLOWED_CHANNELS` is unset so the slash surface is user-scoped only, and an unsigned probe of
  the live gateway answers 404 for `posture` against 401 for `pns` and `priority`, so posture's Discord
  leg is not being delivered at all. The design recommends gating the recovered investigator on this
  change. Awaiting the operator's trust-boundary decision and the eight open questions; nothing was built
  or changed. Full document: `docs/superpowers/specs/2026-09-14-osquery-approval-authority-design.md`.
  Operator steps: (1) Read docs/superpowers/specs/2026-09-14-osquery-approval-authority-design.md and
  decide open question 1: does approval authority stay in `posture allowlist` (recommended), move into
  pns behind `Interaction::AwaitDecision`, or sit in Hermes (rejected in the document)? (2) Decide open
  question 2: is detection (a pending scope plus a pns announcement on every grant) enough, or is
  prevention required, meaning a one-time code from KeePassXC that an agent cannot read? (3) Rule on
  whether the recovered Hermes security investigator stays blocked until the pending scope lands. The
  document's position is yes, because `terminal` is enabled with `backend: local` and
  `posture allowlist add` matches no Hermes dangerous pattern, so an investigator reading
  attacker-controlled evidence could write the suppression file in the same session. (4) Decide whether
  `DISCORD_ALLOWED_CHANNELS` is set. The `#osquery` channel id is already rendered into ~/.hermes/.env as
  `DISCORD_OSQUERY_CHANNEL` from the vault entry `Discord (Uriel) :: Channel ID (#osquery)` and is read
  by nothing; setting the allowlist variable also changes the scope of every other Hermes slash command.
  (5) Decide the disposition of the `posture` gateway route: add it to the Hermes webhook route table, or
  have posture stop naming a route so its pages post to the default `pns` route. Verified 2026-09-14 by
  unsigned POST probe: `posture` answers 404, `pns` and `priority` answer 401. Decide the orphaned
  `priority` route and its `{alert.title}` prompt in the same breath. (6) Pick where the Hermes plugin
  lives: chezmoi-managed under `private_dot_hermes/plugins/` (the document's assumption) or its own
  public repository installed with `hermes plugins install`, matching how `ponytail` and the custom
  Neovim plugins are handled. (7) Confirm or reject the four smaller assumptions: a derived rather than
  stored pending set, a new `decline` verb beside the existing `deny` (which today means undo an allow),
  a decline marker that suppresses only the re-page, and a bounded results-log window defaulting to the
  digest's own day. Open questions: (1) Does approval authority stay with `posture allowlist`, bounded by
  a pending scope and an announcement, rather than moving into pns behind `Interaction::AwaitDecision`?
  Everything else depends on this answer. (2) Is detection enough, or is prevention required? A grant
  still succeeds if an agent runs the writer against a genuinely pending finding; the only real
  prevention available on this host is a one-time code from KeePassXC, and it should be decided now
  rather than retrofitted. (3) How far back does the pending window reach? The digest's own day is the
  obvious default, and under it a finding reported Friday and approved Monday is refused, which is either
  a safety property or an annoyance. (4) Should `decline` be a verb at all, or should a declined finding
  keep paging? The decline marker is the only new state this design stores, and dropping it is the
  smaller design. (5) Chezmoi-managed plugin directory under `private_dot_hermes/plugins/`, or its own
  repository installed with `hermes plugins install`? The repo's precedent for custom plugins points to a
  separate repository, and this plugin is smaller than any of them. (6) Is `DISCORD_ALLOWED_CHANNELS`
  wanted? Setting it scopes the slash surface to one channel and changes the behavior of every other
  Hermes slash command at the same time. (7) Does the `posture` gateway route get added, or does posture
  stop naming a route and post to the default `pns` route? Verified 404 today, and a prerequisite for any
  Discord approval surface; the orphaned `priority` route needs a disposition too. (8) Is the recovered
  Hermes investigator gated on this change landing? The document's position is yes: an agent that reads
  attacker-controlled evidence and can write the suppression file in the same session is the
  configuration that exists today.
- [ ] Reconcile the June hardening-plan remainder and explicitly deferred FleetDM, beaconing and Wazuh
  research. Record accepted scope before implementation. Issue #18's FileVault fix and dead snapshot
  handling are already present in the current query, Bash and Rust paths; reconcile/close its stale issue
  rather than reopen that implementation. PR #19 is closed and was not merged. On 2026-09-14 the
  reconciliation ran with no implementation. [#18](https://github.com/webdavis/dotfiles/issues/18) was
  closed as completed after its FileVault fix and its dead-snapshot problem were verified present on
  `main` in all three paths: the query path in
  `dot_local/libexec/posture/converge/desired/packs/security-policy-regression.conf` at `3cae79ce`
  ([PR #62](https://github.com/webdavis/dotfiles/pull/62), merged 2026-07-22), where `filevault_state` is
  informational with a log-only removed row and `filevault_off` is differential at interval 3600 with no
  snapshot key; the Bash path in `dot_local/libexec/osquery/results-alerter/route.sh` at `f511e85a`
  ([PR #64](https://github.com/webdavis/dotfiles/pull/64), merged 2026-07-22), whose `protection_off`
  covers only `filevault_off` added and drops `filevault_state` to NOTICE; and the Rust path in
  `posture/crates/posture-domain/src/severity.rs` at `2522d0b2`
  ([PR #429](https://github.com/webdavis/dotfiles/pull/429), merged 2026-09-07), where `FilevaultOff`
  with `Added` is Critical and `FilevaultState` is Notice for every action. `3cae79ce` is also the
  dead-snapshot fix, because snapshot results land in `osqueryd.snapshots.log` which the alerter never
  reads, and it removed `firewall_off`, `gatekeeper_off` and both `screenlock` queries for the same
  reason. Coverage is pinned by `test_c2_differential_filevault_off_added_not_snapshot_fires_a_crit_page`
  in `test/e2e/osquery-alerter-criteria.test.sh`, `other_security_policy_rows_are_notice_never_info` in
  `posture/crates/posture-domain/src/severity/tests.rs` and `c2_added_filevault_off_pages` in
  `posture/crates/posture-domain/src/gate/tests/page.rs`.
  [PR #19](https://github.com/webdavis/dotfiles/pull/19) was confirmed closed unmerged on 2026-06-15
  (`merged_at` null, head `e057aecb` not an ancestor of `main`) and was left untouched. The three
  research items were dispositioned as still deferred against `origin/docs/osquery-design` head
  `2202dcbf`: fleet management stays out of this repository (master spec v2 sections 2 and 13), beaconing
  and new-listening-port detection stay deferred on noise grounds (reshape design line 114, build
  guidance lines 160 to 165), and the Wazuh migration is revisited only after the page tier proves calm
  and only in homelab automation (reshape design line 113, redesign decisions line 121). FleetDM is a
  chosen product nowhere on that branch, whose only two mentions are citation URLs, so this entry should
  read fleet management. Todoist task `6hPCHJM8PJjcx4cv` carries the same evidence in comment
  `6hW4G7vghG9cM3gv` and stays open for the hardening-plan remainder. Operator acceptance of the recorded
  scope is still outstanding. Owed from the operator: (1) Accept or amend the recorded scope before any
  implementation of the June hardening-plan remainder: read Todoist comment `6hW4G7vghG9cM3gv` on task
  `6hPCHJM8PJjcx4cv` (`td comment list 6hPCHJM8PJjcx4cv --all --lines 40`) and reply with acceptance or a
  correction.; (2) Decide whether the ledger entry's 'FleetDM' should be reworded to 'fleet management',
  since no document on `origin/docs/osquery-design` picks FleetDM as a product (its only two mentions are
  citation URLs at `docs/superpowers/research/2026-06-08-macos-persistence-monitoring-design-research.md`
  lines 144 and 145).
- [ ] Give the June hardening requirements explicit dispositions: signature-chain verification,
  interpreter-payload assessment and per-run grouping of repeated findings about the same subject.
  Current Rust code reads signature metadata, leaves interpreter payloads unverified and retains each
  finding in a batch. Porting the existing behavior did not implement these proposed changes. A design
  recording all three dispositions was written on 2026-09-14 and lives at
  `docs/superpowers/specs/2026-09-14-june-hardening-dispositions-design.md`. Each disposition names
  current behavior, measured against the installed `posture` binary and the live results log rather than
  read from the plan. Signature chain: change, but not as proposed. `spctl --assess -t exec` rejects
  every non-bundle executable on macOS 26.2, `/bin/echo` and `/usr/bin/codesign` included, so June's task
  4 gate would mark nearly every launch job untrusted; the working test is one
  `codesign --verify --strict -R="anchor apple generic"` call, whose exit codes separate an absent or
  invalid signature (1) from an unsatisfied requirement (3). Its measured marginal detection on this
  machine is one case (BlueBubbles, an invalid seal that plain `--verify` also catches), because 19 of 45
  live launch plists already enrich untrusted, every one a legitimate job, and four of the nine
  allowlisted labels page anyway since the untrusted verdict promotes Notice to Critical ahead of the
  allowlist. Interpreter payload: change. Twelve of 45 plists front an interpreter and all twelve read
  trusted today, with seven carrying a payload the resolver cannot reach (`sh -c` command strings,
  `python -m` module names, one relative path); the recommendation is to stop vouching for an unverified
  payload, ask the known-good manifest where a payload resolves, and add an explicit unresolved state,
  while refusing June's modification-time and writable-but-not-owned conditions with the reasons
  measured. Per-run grouping: change, and it is now the only lever, because the ingestion model's D2 is
  closed: the `launch_agents` and `launch_daemons` watched trees now feed the file-integrity arm, so
  removing them would delete the integrity watch on the seven manifested osquery launch agents. Measured
  duplication on this machine is 11 rows for one path inside one 300-second window against a page that
  renders 8 blocks, plus 26 paths that emit two rows per write because `managed_bin` contains
  `pipeline_integrity`. Recommended as one design and three pull requests, grouping first so the
  inspection load falls before it rises. Three findings are recorded as observations rather than proposed
  work: the one shared 10-second inspection deadline covers the whole batch while its comment claims one
  finding, a failed or timed-out inspection is reported to the operator as the word UNSIGNED, and the two
  watched trees overlap. Eight open questions await the operator, led by what the signing verdict is for
  now that it answers untrusted for two fifths of the machine's own launch jobs. No code was written or
  changed. Full document: `docs/superpowers/specs/2026-09-14-june-hardening-dispositions-design.md`.
  Operator steps: (1) Read docs/superpowers/specs/2026-09-14-june-hardening-dispositions-design.md and
  answer the eight open questions, starting with question 1 (what the signing verdict is for), because
  the other seven sit downstream of it. (2) Accept or reject each of the three dispositions separately.
  Nothing is implemented until you do; the ledger task stays unticked. (3) Rule on the eleven assumptions
  listed under "Assumptions made in the operator's place". The two most consequential are whether an
  uninspectable finding promotes a Notice to a Critical page, and whether an unmanifested ordinary user
  script counts as an untrusted interpreter payload. (4) Decide whether the overlapping watched trees are
  narrowed as their own change. `managed_bin` watches ~/.local/libexec/%% and fully contains
  pipeline_integrity's ~/.local/libexec/osquery/%% and ~/.local/libexec/posture/%%, so 26 measured paths
  emit two rows per write. osquery file path patterns have no exclusion form, so narrowing means
  enumerating ~/.local/libexec's other children by name. (5) Decide whether the live codesign spoof drill
  is worth your time. Signing with a self-signed certificate whose common name imitates Apple needs
  `security add-trusted-cert`, which needs an administrator; without it the claim that today's parser
  trusts a spoofed name rests on reading a pure function rather than on a demonstration. (6) Discard the
  scratch key material at
  /private/tmp/claude-501/-Users-stephen-workspaces-Ivy-webdavis-dotfiles/1bf0ef19-e242-4ea5-8746-60cb679ebafc/scratchpad/spoof/,
  which holds the self-signed certificate and two PKCS#12 files from the abandoned drill. The temporary
  keychain it used was never added to the keychain search list and was deleted in-session;
  `security list-keychains` confirms only login.keychain-db and System.keychain remain. (7) No apply is
  needed. Nothing in this task touched the source tree, and every measurement in the document is
  reproducible read-only from the commands in its final section. Open questions: (1) What is the signing
  verdict for? It answers untrusted for 19 of 45 legitimate launch jobs on this machine and is the only
  input that can promote a Notice to a Critical page, and four of the nine allowlisted labels page
  because of it. Is it a page trigger, or a display fact with the allowlist and the manifest deciding?
  Every other question sits downstream of this one. (2) Does an uninspectable finding promote a Notice to
  Critical? Fail-closed adds noise from unreadable root-owned plists such as com.tailscale.tailscaled
  (mode 0700) and com.docker.socket. Fail-open leaves an inline `sh -c` payload quieter than an ad-hoc
  signed binary. (3) Is an unmanifested ordinary user script an untrusted interpreter payload? On this
  machine all five resolvable payloads are manifested pipeline scripts, so the rule is free here; on a
  machine with user scripts in launch agents it is not. (4) Should the allowlist suppress a promotion?
  Today it cannot: the promotion runs before the allowlist is consulted and outranks it. Letting the
  allowlist win closes the four measured false pages and also lets an entry quiet a genuinely unsigned
  program. (5) Is `anchor apple generic` the right bar, or should Developer ID be a separate lesser tier
  than Apple platform code rather than equal to it? Measured: `anchor apple` alone flips Ghostty and
  every other Developer ID application to untrusted. (6) Should the overlapping watched trees be narrowed
  as a separate, smaller change? It removes a measured doubling for 26 paths but means enumerating
  ~/.local/libexec's other children by name, since osquery file path patterns have no exclusion form. (7)
  Is the live spoof drill worth an administrator's time? Without `security add-trusted-cert` the
  spoofed-name claim rests on reading `classify_signing` as a pure function rather than on a
  demonstration. (8) One change set or three pull requests? The recommendation is one design and a ladder
  of three, grouping first, because disposition 1 adds a process per code finding against a deadline that
  disposition 3 relieves.
- [ ] Preserve deferred install-state kernel-extension monitoring and off-host machine-death detection.
  The former needs reconciliation with July's decision to alert on untrusted loaded extensions; do not
  restore June's obsolete delivery block. The latter needs an external host and remains homelab scope: a
  watchdog on the monitored Mac cannot detect that Mac disappearing from outside it. Keep intentional
  log-only `es_launchd_writes` handling and accepted residual risks out of the implementation queue.
  Design written 2026-09-14 and pending the operator's read:
  `docs/superpowers/specs/2026-09-14-extension-install-state-and-machine-death-design.md`. Install-state
  monitoring is reconciled as ADDITIVE to July's 2026-07-22 ruling rather than a re-tiering of it: the
  loaded-extension arms keep their current behavior (S049 log-only unless promoted, S050 digest unless
  promoted) and a new `extension_install` file-integrity watch category over `/Library/Extensions/%%` and
  `/Library/StagedExtensions/%%` pages unconditionally, because the install set changed once in six
  months while the loaded table churned 657 times. Measured on this machine to justify the split: four
  third-party kernel extensions are installed or staged (HighPointIOP, HighPointRR, SoftRAID, and
  macfuse.fs under `Library/Filesystems`, which a narrower staged glob would miss) and ZERO are loaded,
  so the only table the July ruling reads says nothing about any of them. The recommended change is one
  config declaration, one `FileCategory::ExtensionInstall` variant, one gate arm with no direction gate,
  and one shared subject-collapse in `render_page`, which is needed because five files per bundle would
  otherwise consume five of the eight `BLOCK_LIMIT` page blocks and evict a concurrent critical finding;
  that same collapse answers the sibling per-run grouping bullet. June's Task 6 delivery block stays
  retired on three grounds: its Apple-filtered premise was measured false four days later, July already
  supplied a better load-state answer, and its dispatch path no longer exists now that posture is a
  producer piping into `pns submit --json`. Off-host machine-death detection is confirmed homelab scope
  and out of the implementation queue: every pns destination originates on dresden (hermes posts to
  127.0.0.1:8644, moshi spawns a local binary), `tailscale status` shows one live Mac, one stale node and
  an iPhone, and dresden is a MacBook Pro that sleeps on battery, so per
  `/usr/share/man/man5/launchd.plist.5` a `StartInterval` beacon is missed across sleep while a
  `StartCalendarInterval` one coalesces on wake, which makes the daily heartbeat the only correct carrier
  and a roughly 48-hour two-missed-beacon threshold the only honest granularity. No beacon is built ahead
  of a chosen consumer because its interface is decided by that choice. Intentional log-only
  `es_launchd_writes` handling and the five June accepted residuals are recorded as explicitly out of
  scope. Nine open questions await the operator; questions 1, 2 and 6 are the gating ones. Full document:
  `docs/superpowers/specs/2026-09-14-extension-install-state-and-machine-death-design.md`. Operator
  steps: (1) Read docs/superpowers/specs/2026-09-14-extension-install-state-and-machine-death-design.md
  and answer the nine open questions; questions 1 (build install-state monitoring at all), 2
  (unconditional page versus signing-gated) and 6 (is there or will there be a second always-on host)
  gate everything else. (2) Nothing to deploy or apply: no code changed and no configuration was edited
  by this task. (3) If install-state monitoring is approved, note before scheduling it that the change
  edits a templated desired-state file under dot_local/libexec/posture/converge/desired/, so it needs a
  FULL chezmoi apply and the pipeline audit will page a CRIT on every tick until that apply lands. (4)
  Decide whether the subject-collapse behavior may close the sibling ledger bullet about per-run grouping
  of repeated findings, since the design proposes one shared mechanism in posture-domain rather than two.
  Open questions: (1) Build install-state extension monitoring, or close the deferral and keep extensions
  exactly as they are? (2) Should the new category page unconditionally, or be signing-gated like the
  loaded-extension arm (which would leave a legitimately signed but vulnerable driver at digest)? (3)
  Does the subject collapse land as the shared per-run grouping mechanism in posture-domain, closing the
  sibling ledger bullet, or scoped to this detector only? (4) Does Page.count keep counting critical
  findings, or count subjects so a five-file install reads '1 CRITICAL'? (5) Is a macFUSE or SoftRAID
  upgrade paging acceptable, or is a path-keyed extension allowlist wanted, accepting a new trust surface
  for a roughly annual event? (6) Is there, or will there be, a second always-on host, and on what
  horizon? This decides whether any interim machine-death coverage is built at all. (7) If interim
  coverage is wanted: a scheduled GitHub Actions workflow with a private gist beacon, or a hosted
  dead-man's-switch service (a new vendor holding a home-occupancy signal)? (8) What is the longest
  period the machine is legitimately dark (travel, a weekend away)? That sets the overdue threshold and
  the false-page rate. (9) Where does planned-downtime suppression live, given it cannot live on dresden?

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
  source-specific facts supplied by security producers and immediate original alert delivery. Reconciled
  on 2026-09-14 in `docs/superpowers/specs/2026-09-14-hermes-critical-alert-investigation-design.md`,
  which supersedes the daily-digest investigation scope in all four #24 documents and keeps the workflow
  Hermes-owned. Superseded: the original design's section 11 orchestration, section 4 sandbox claims and
  section 8 threaded-reply contract; the plan's whole task ladder, its `mouse` profile name, `--attach`
  and egress allow-list task; D-V2-12's "Mouse's real role" digest advisory; and master spec v2 section
  13's digest scope and PR-#3 sequencing. Kept: the immediate-alert and separate-advisory invariant, the
  monotonic advisory contract, the fixed remediation vocabulary and the failure-notification requirement.
  Recommendation: one kanban card per Critical alert on the installed pin `a4091e49`, assigned to a
  Docker-backed profile, evidence handed over as `--workspace dir:<path>`, and the advisory published by
  a deterministic validator off a schema-constrained `result.json` rather than as model prose. Measured
  against the installed source and live config, not upstream docs: `kanban attach` and `--attach` do not
  exist here and `--workspace dir:` replaces them; `--board` is a global flag before the subcommand; the
  container holds tool execution while the worker's model call stays on the host, so `--network=none`
  through `terminal.docker_extra_args` buys networkless evidence execution without breaking the model;
  nothing in hermes ever passes the `DockerEnvironment(network=...)` parameter, so that flag is the only
  lever; skill-registered credential files and the skills directory are bind-mounted automatically;
  `container_persistent` and `docker_persist_across_processes` must both be off for an ephemeral box; the
  worker inherits the gateway's whole environment via `env = dict(os.environ)`; the webhook delivery path
  never passes `reply_to`, so a Discord threaded reply is not buildable; hermes ignores pns's
  `Idempotency-Key` on the webhook path and keys dedup off a millisecond timestamp; auto-decompose
  touches triage cards only, so an investigation card is never decomposed; and posture's `class` is
  `security`, never a severity, so the Critical tier does not cross the wire. Prerequisite confirmed as a
  live mismatch: there is still no `posture` route in `~/.hermes/config.yaml`, so posture alerts 404 and
  dead-letter. The design is NOT approved and NOT built; it records nine assumptions made in the
  operator's place with their alternatives and eight open questions, of which the model-provider
  disclosure decision and the trigger's placement are blocking. Full document:
  `docs/superpowers/specs/2026-09-14-hermes-critical-alert-investigation-design.md`. Operator steps: (1)
  Read the design at docs/superpowers/specs/2026-09-14-hermes-critical-alert-investigation-design.md; it
  is not applied by this task and needs a commit. (2) Answer open question 1, the blocking one: does the
  investigator's model see the artifact bytes, or only the host-produced fact sheet? Evidence reaches
  https://chatgpt.com/backend-api/codex either way, which is what makes this a disclosure decision rather
  than an architecture one. (3) Answer open question 2: where the deterministic trigger lives, among a
  hermes no_agent cron job, a hermes plugin this repository owns, or a host watcher on posture's results
  log. (4) Review the nine assumptions in the operator's place and overrule any that are wrong;
  assumption 8, the investigator profile's name, is flagged as the weakest because this machine's four
  existing hermes profiles all use character names. (5) Decide whether the gateway's own environment is
  acceptable as the credential surface a kanban worker inherits wholesale, or whether it must be narrowed
  before any investigator exists. (6) Resolve the prerequisite separately: posture names the route
  `posture` at every call site and no such route exists in ~/.hermes/config.yaml, so its alerts 404 and
  dead-letter today. An investigation hanging off a dead-lettering alert path is never seen. (7) Nothing
  here was applied. No code was written or changed, no config was touched, and no apply is needed or safe
  to run off this task. Open questions: (1) Does the investigator's model see the artifact bytes, or only
  the host-produced fact sheet? This is the model-provider disclosure decision the ledger records as
  blocking; the design builds either and the answer is one configuration line. (2) Where does the
  deterministic trigger live: a hermes `no_agent` cron job (needs a wrapper script physically under
  ~/.hermes/scripts/ and depends on the gateway daemon), a hermes plugin this repository owns, or a host
  watcher of posture's results log under a LaunchAgent? (3) What does the gateway's own environment
  contain? The kanban worker inherits it wholesale through `env = dict(os.environ)`, and that is the
  credential surface the container boundary cannot close. (4) How is "the gateway is down, so nothing was
  investigated" surfaced? It is the one failure with no notice of its own; a card sits ready forever and
  publishes neither an advisory nor a failure. posture's watchdog already probes the gateway route, so
  letting it own this is cheap but needs saying. (5) Is the investigator profile a character name
  (matching butters, concerned, elaine and nicodemus on this machine) or a function name (matching the
  self-documenting and user-agnostic naming rulings)? (6) Does the alert wire gain a severity field, a
  structured evidence reference, or neither? Today Critical can only be inferred from producer plus event
  name, because posture's `class` is the literal `security`. Adding both is the honest fix but touches
  two independent workspaces and their golden fixtures. (7) Should run_after_59-hermes-config-migrate
  cover profile configs? It migrates only the root config.yaml forward to the installed schema, so a new
  profile's config.yaml carrying a Docker block can fall behind a pin bump with nothing to catch it. (8)
  Is the advisory's Discord destination the same channel as the page, or a separate one? The ledger notes
  a named route selects one destination rather than broadcasting to two, so this is a route decision.

- [ ] Preserve immediate deterministic alert delivery. Hermes then starts a dedicated, ephemeral
  investigator over a bounded copy of the supplied evidence and publishes a separate advisory associated
  with the alert. Failure or timeout must be distinguishable from an all-clear. The investigator cannot
  suppress, delay or rewrite the original alert, approve trust, or perform remediation. Verify the
  current correlation and evidence-transfer contract before wiring it. The recorded advisory contract
  permits adding concern or explanation but forbids clearing the original finding, and limits suggested
  responses to a fixed vocabulary. Verify those limits outside the prompt before promising enforcement.
  On 2026-09-14 a design was written for this bullet and lives at
  `docs/superpowers/specs/2026-09-14-bounded-security-advisory-design.md`. It names an enforcement point
  for each of sixteen advisory limits and proves the four untouchability properties (cannot delay,
  suppress, rewrite, clear) from measured code, and it corrects four claims in the preceding
  reconciliation document: `kanban_task_blocked` never fires on a timeout, crash or circuit-breaker trip
  (only `block_task()` fires it; `enforce_max_runtime` and `_record_task_failure` write `blocked` by
  direct SQL and emit `timed_out`/`gave_up` events instead), `kanban_task_completed` fires in the worker
  process, `kanban notify-subscribe` does cover all five terminal kinds from the gateway but publishes
  the worker's summary verbatim and then uploads any host file whose absolute path appears in it
  (`validate_media_delivery_path` accepts any regular file outside a credential denylist that misses
  `~/Documents`, `~/workspaces`, `~/.local/state` and `~/.claude.json`), and the alert's correlation
  identity is not stable across a retry that reads more results rows. Recommendation: one host-side
  reconciler on a LaunchAgent timer as the sole trigger and publisher, reading pns's `ledger_events`
  (which exists only after the page committed) as its trigger input, a new `posture evidence` subcommand
  for the bounded no-follow copy so the file handling reuses posture's existing refusal posture and
  signing inspection, a Docker-backed profile with `--network=none` through `docker_extra_args` and no
  skills directory (the credential, skills and cache mounts are ungated but resolve under the profile's
  own `HERMES_HOME`, so an empty profile root closes them), and a six-field schema-constrained
  `result.json` validated against a `record/facts.json` kept outside the container's only writable mount.
  The 2026-06-03 sandboxed-agent research report was read as an input and its recommendation 3 settles
  the fact-sheet-versus-artifact-bytes question in favour of the host-produced fact sheet. Still blocked
  on the operator: the execution and network boundary, the gateway environment's contents, and ten open
  questions recorded in the document. Full document:
  `docs/superpowers/specs/2026-09-14-bounded-security-advisory-design.md`. Operator steps: (1) Read
  `docs/superpowers/specs/2026-09-14-bounded-security-advisory-design.md` and settle the execution and
  network security boundary the ledger records as the blocker; nothing is built until then. (2) Answer
  open question 1: is the reconciler bash under `~/.local/libexec` or a fifth cargo workspace installed
  to `~/.cargo/bin`. (3) Answer open question 2: does the bounded evidence collector go into posture as a
  `posture evidence` subcommand, or does producing evidence count as the agent orchestration posture may
  not carry. (4) Answer open question 3: is a `kanban notify-subscribe` belt wanted for the failure half,
  given it requires `gateway.strict: true` machine-wide to be safe and changes media delivery for every
  other profile. (5) Inspect what `~/.hermes/.env` gives the gateway, since the worker inherits that
  environment wholesale via `env = dict(os.environ)` and the container boundary does not close it (open
  question 4). (6) Decide who watches the reconciler itself, the one remaining silent failure; extending
  posture's existing `GatewayProbe` watchdog is the cheap answer (open question 5). (7) Decide
  `kanban.max_in_progress_per_profile` for the investigator profile; it is `null` today so a burst of
  Critical alerts starts one Docker container per card (open question 6). (8) Decide whether posture's
  request gains `severity` and an evidence reference in its existing `extensions` map now or later (open
  question 7). (9) Decide the advisory's Discord destination: the page's `priority` channel or a separate
  route (open question 8). (10) Decide whether `run_after_59-hermes-config-migrate` should cover profile
  `config.yaml` files, which now carry the whole execution boundary and nothing migrates across a pin
  bump (open question 9). (11) Decide the investigator profile's name: a function name or the machine's
  character-name convention (open question 10). (12) Resolve the missing `posture` webhook route first;
  it is a prerequisite, since an investigation hanging off a route that dead-letters is an investigation
  nobody sees. Open questions: (1) Is the reconciler bash under `~/.local/libexec` or a fifth cargo
  workspace? The design's remaining work after `posture evidence` is a SQLite read, two command
  invocations and a six-field validation, which argues bash, but the validator is the security-critical
  component. (2) Does `posture evidence` belong in posture? The document reads producing evidence as a
  producer feature rather than agent orchestration; if that reading is wrong the collector moves into the
  reconciler and the bash-versus-Rust question stops being optional. (3) Is a gateway notify subscription
  wanted as a belt for the failure half? It would survive a dead reconciler but needs
  `gateway.strict: true` machine-wide, which changes media delivery for every other profile, and it still
  publishes the worker's completion text. (4) What does the gateway's own environment contain? The worker
  inherits it wholesale and the container cannot close that surface. (5) Who watches the reconciler? It
  is the one remaining silent failure in the table; posture's watchdog already probes gateway health, so
  extending it is the cheap answer but needs saying. (6) Should `kanban.max_in_progress_per_profile` be
  set for the investigator profile? It is `null` today, so concurrency is unbounded and a burst of
  Critical alerts starts one container per card. (7) Does the alert wire gain `severity` and an evidence
  reference in its existing `extensions` map now or later? It is additive to one workspace plus golden
  fixtures and is not required for the design to work. (8) Is the advisory's Discord destination the same
  channel as the page? A named route selects one destination rather than broadcasting, so this is a route
  decision. (9) Should `run_after_59-hermes-config-migrate` cover profile configs? The investigator
  profile's `config.yaml` carries the whole execution boundary and nothing migrates it across a pin bump.
  (10) Is the investigator profile named for its function or given a character name, matching `butters`,
  `concerned`, `elaine` and `nicodemus`?

- [ ] Define the required alert/evidence metadata through the existing delivery path. The current pns
  webhook forwards `agent`, `state`, `project`, `detail` and `request_id`; it does not forward the
  producer's security class or a structured artifact reference. Verify classification and evidence
  references before attaching an investigator, rather than inferring them from rendered prose. Also
  reconcile route names: native posture names `posture`, while the tracked route checker covers `pns` and
  `unattended-upgrades` as delivery-only. The encrypted route configuration was not inspected in this
  review. Any producer/transport changes should carry data; Hermes owns the investigation. On 2026-09-14
  a design was written to `docs/superpowers/specs/2026-09-14-security-alert-metadata-contract-design.md`
  covering both halves of this task, and the operator half was answered by inspection rather than
  deferred: the age identity was already on disk at `~/.config/chezmoi/key.txt`, so the encrypted route
  configuration was decrypted and read without a vault unlock. It declares exactly `priority`, `pns` and
  `unattended-upgrades`, in both the source and the deployed `~/.hermes/config.yaml`, all three
  `deliver_only`. The mismatch is already losing pages: native posture hard-codes the route `posture` at
  six call sites, the live gateway answers 404 for it, and `ledger_legs` holds eight dead-lettered hermes
  legs on that route (`deadletter_reason = permanent`, `http_status = 404`) whose banner legs all
  delivered, so eight daily digests reached the operator locally and were never written to Discord.
  `pns doctor` and `pns failures` already name both that route and `pns-recap` as missing, but
  `RouteVerdict::Missing` maps to `Mark::Warn`, so the run closes with `✓ nothing to act on` over two
  routes that drop every page. The recommendation: carry one additive optional `security` object on
  `pns.request/1` (`severity`, `finding_count`, and an `evidence` array of `{kind, locator, detector}`),
  added to both wire copies with `skip_serializing_if` so the golden fixtures stay byte-identical,
  mirrored into the hermes body off the `producer_request` string every destination and every retry
  already receives, with delivery failing open and the investigator trigger failing closed on malformed
  metadata; declare a `posture` route and extend
  `.chezmoiscripts/run_after_68-hermes-log-route-status.sh.tmpl` to all five names (`priority` carries
  today's only CRITICAL page and is checked by nothing); collapse posture's six hard-coded route sites to
  one overridable default; and fall back to the default route once, with a line, on a permanent refusal,
  the way `post_return_recap` already does. Repointing posture at `priority` was rejected for now: no
  single route prompt can serve both the pns body shape and the Bash alerter's `{alert.title}` shape, so
  that move belongs after the alert cutover, not before it. This is a blocker on
  `feat/posture-alert-cutover`: the moment it merges and is applied, the CRITICAL security page moves to
  the 404 route and the eight lost digests become lost security pages. Seven open questions wait on the
  operator, chiefly which Discord channel security pages land in, whether a security page in the general
  channel beats no security page, and per-leg versus per-retry delivery-attempt identifiers. Full
  document: `docs/superpowers/specs/2026-09-14-security-alert-metadata-contract-design.md`. Operator
  steps: (1) Read the design at
  docs/superpowers/specs/2026-09-14-security-alert-metadata-contract-design.md and answer the seven open
  questions, in particular: which Discord channel security pages land in, whether a page in the general
  `pns` channel beats no page at all, and per-leg versus per-retry delivery-attempt identifiers. (2)
  Decide the merge order: this design treats declaring the `posture` route as a blocker on
  `feat/posture-alert-cutover`, because that cutover moves the CRITICAL security page onto the 404 route.
  (3) Edit the age-encrypted hermes config to add a `posture` route: `deliver_only: true`, its own 64-hex
  secret matching the pns signing key, `deliver: discord`, a `deliver_extra.chat_id` for the chosen
  channel, and a prompt that names only body keys the pns body actually carries (a key the body lacks
  renders as the literal `{key}` in Discord). (4) Run a full `chezmoi apply` (no by-name apply, no
  `--exclude=templates`) so the decrypted config reaches `~/.hermes/config.yaml`, then
  `hermes gateway restart` so the gateway loads the new route, since a route in the file that the gateway
  has not loaded still answers 404. (5) Confirm with
  `curl -s -o /dev/null -w '%{http_code}' -X POST http://127.0.0.1:8644/webhooks/posture -H 'Content-Type: application/json' -d '{}'`
  that the answer is 401 (route exists, unsigned body refused) rather than 404. (6) Confirm with
  `pns doctor` that the Delivery section no longer reports
  `route posture: THE GATEWAY HAS NO SUCH ROUTE`, and decide whether `pns-recap` should be declared too
  or left degrading to the default route. (7) Once a real posture digest or page lands in the chosen
  channel, confirm `pns failures` stops growing on route `posture`. Open questions: (1) Which Discord
  channel do security pages land in: a new one for the `posture` route, or `#priority` reused with its
  own secret and a prompt that serves the pns body shape? (2) Is a security page in the general `pns`
  channel better than no security page (the recommended loud-ward fallback), or must a security page
  appear only in its own channel? (3) Per-leg or per-retry delivery-attempt identifier, that is,
  lost-page risk or duplicate-page risk? Hermes at the installed revision ignores `Idempotency-Key` and
  reads `X-Request-ID`, so this is a live choice. (4) Should the `posture` route be declared and the
  checker extended before `feat/posture-alert-cutover` merges? The design says yes and treats it as a
  blocker; the operator owns the merge order. (5) Is `security` the right field name next to an existing
  `class` whose value is already the literal string `security`, or is `finding` clearer? (6) Does the
  evidence vocabulary need a fourth kind now for the file-integrity path (a manifest entry), or does
  `file` plus `detector` cover it until that reader exists? (7) `pns doctor` closing with
  `✓ nothing to act on` over two route warnings is a one-line severity change (`RouteVerdict::Missing`
  from `Mark::Warn` to `Mark::Bad`). Fold it into this work, or file it separately?

- [x] 2026-09-14: the hermes gateway gained the `posture` and `pns-recap` webhook routes, and posture
  learned to pick its route from a finding's tier, in
  [PR #607](https://github.com/webdavis/dotfiles/pull/607)
  (`feat(hermes): add the posture and pns-recap gateway routes`, merged), a piece of the missing-route
  finding above but not a numbered task. Each new route copies the `pns` route (same signing secret,
  `deliver_only`, same prompt template); their `deliver_extra.chat_id` ships empty because the gateway
  does no environment expansion, so two new `DISCORD_*_CHANNEL` lines render into `~/.hermes/.env` for
  the operator to paste, and the apply-time route check reports the gap until they do. `severity_route`
  in posture-domain makes the route decision: Critical names `priority`, Notice and Info name `posture`,
  and a submission with no tier names no route at all. The route check now compares every route's secret
  against `pns`'s, which surfaced a real finding: `priority` still carries the retired Bash alerter's key
  rather than pns's, so a CRIT page routed there answers 401 and is silently lost, since posture advances
  its cursor once pns reports the submission accepted regardless of what the destination did with it. The
  four route behaviors are pinned by tests verified against two hand mutants; the change is inert until
  applied. Operator steps left: run a full `chezmoi apply` (KeePassXC unlocked); fill the two channel ids
  from `~/.hermes/.env` into `~/.hermes/config.yaml`'s route `deliver_extra.chat_id` fields via
  `chezmoi edit`, then apply again; decide the `priority` secret question (open question below); restart
  the gateway with `hermes gateway restart` so the new routes load (they answer 404 until then); confirm
  the routes are loaded with a signed-route 401 check documented in the PR. Open questions: (1) which
  side gives way on the `priority` route's secret, held over from the retired Bash alerter, since giving
  it the pns secret costs the alert drainer its ability to deliver what that alerter left queued, while
  leaving it means posture's CRIT pages stay refused; a third option is retiring the drainer outright,
  since posture's own agents already cover its job; (2) the `priority` route's prompt template is still
  the Bash alerter's `{alert.title}`/`{alert.detail}` shape, so even with the secret reconciled a
  pns-shaped CRIT body delivers literal placeholders until the stale-escalation PR's prompt-template
  change lands too. Closed 2026-09-15. The routes moved into a chezmoi modify template carrying one
  KeePassXC secret and one channel id per route in
  [PR #617](https://github.com/webdavis/dotfiles/pull/617); pns gained per-route keys in
  [PR #624](https://github.com/webdavis/dotfiles/pull/624); posture became engine-agnostic and sends its
  critical pages to `priority` in [PR #623](https://github.com/webdavis/dotfiles/pull/623); the
  `pns-recap` route was dropped in [PR #628](https://github.com/webdavis/dotfiles/pull/628); and the
  survivors were renamed to match their Discord channels in
  [PR #626](https://github.com/webdavis/dotfiles/pull/626).
  [PR #625](https://github.com/webdavis/dotfiles/pull/625) took the `.tmpl` suffix off that modify
  template, which chezmoi had been rendering as a template and then executing (`exec format error`), and
  pinned the basename with a test that drives `chezmoi diff` over a scratch source. The full
  `chezmoi apply` and a `hermes gateway restart` both ran on 2026-09-15; the gateway config now lists
  `general`, `pns-events`, `posture-pages`, `priority` and `uu-runs`, the same five names the source
  template builds, and live posts reached `pns-events`, `priority` and `posture-pages` that day. Both
  open questions are answered: every route carries its own key, so the `priority` secret question is
  moot, and that route now renders the shared three-line layout.

- [ ] 2026-09-14: pns learned to say who an event is from, a structured sender header on every hermes
  message, merged in [PR #612](https://github.com/webdavis/dotfiles/pull/612)
  (`feat(pns): structured sender header on every hermes message`, merged). A harness payload now carries
  the prompt and the harness's own session title, flattened on the way in; schema migration 9 added a
  sessions table whose row the prompt hook writes, naming the session from its first prompt (cut to sixty
  characters) and never relabeling it on a later prompt, with the stale-block escalation's two columns
  created unwritten. One bounded
  `git rev-parse --path-format=absolute --git-common-dir --show-toplevel --abbrev-ref HEAD` per event
  replaced the working directory's last segment as the project, so a linked worktree reports the
  repository instead of its branch slug, the worktree's own name fills the branch slot on a detached
  head, and the directory name is the fallback outside a repo. Every hermes post gained `header`,
  `subheader`, `body` and an always-empty `thread_id` beside its five existing keys, composed by two
  total functions that drop an empty part with its separator, and the `pns` route's prompt in the
  encrypted hermes config became the three-line layout A template (bold header, Discord subtext
  subheader, bare body); the phone card's title stayed byte for byte the same, because an iOS
  notification title truncates before a header would reach the state word. Sixteen behaviors are pinned
  test-first and the whole pns workspace stayed green at 2261 tests. Operator steps: run `chezmoi apply`
  (rebuilds and installs `pns`, no dependency change, kickstarts `com.webdavis.pns-daemon` onto the new
  binary, writes the new `~/.hermes/config.yaml`; schema migration 9 runs on the first invocation after
  that); run `hermes gateway restart` so the pns route's new prompt loads, since the running gateway
  otherwise keeps rendering the old two-line template; fire one live event from a worktree and confirm
  `#pns` shows a bold `repository · branch · state` line, a dim `claude · <four characters> · <title>`
  line, and the body underneath; expect a session already running when the apply landed to show no title
  on that dim line until its next prompt names it. Open questions: which upstream hermes ask to file
  first for a real Discord thread per session, `deliver_extra.thread_name` or a returned `raw_response`
  on a `deliver_only` response; whether a retried hermes post's dim line, which names only the agent
  because the delivery ledger keeps no session, needs a session column on `ledger_events` if that reads
  wrong in the channel; and whether the sixty-character title cap and the state's position at the end of
  the header's first line hold up once the operator sees them in front of them. The full `chezmoi apply`
  and a `hermes gateway restart` both ran on 2026-09-15. Still owed: one live event confirming the
  header, subheader and body layout in Discord.

- [ ] 2026-09-14: the stale-block escalation shipped, the second pull request of the pns
  session-attribution design, merged in [PR #613](https://github.com/webdavis/dotfiles/pull/613)
  (`feat(pns): escalate a session blocked for an hour`, merged). An event that starts a session's wait
  (any state in `pulse::LAMP_BLOCKED`) now stamps `blocked_since` on that session's row at the same seam
  that writes the blocked marker, ungated by the lamps, and registers one leased job on the existing
  pns-daemon clock; a later non-blocking event, and the prompt and resolved hook arms, clear the row. A
  new `pns stale` subcommand selects the rows waiting since now minus the window with no escalation
  stamped, stamps each one under `escalated_at IS NULL` so the database arbitrates two fires woken in one
  tick, and raises one ordinary event per stuck session on the priority route, carrying the sender header
  and subheader plus a body reading how many minutes the block has stood. The gate reads one surface
  reading before any claim: away is silent, a screen locked for the whole window is silent, a screen
  locked for part of it still pages, and a suppressed fire stamps nothing so the block is escalated the
  first time the operator is reachable. The window is `[nag] stale_after_secs`, 60 to 86400 seconds with
  zero the feature off, shipped uncommented at its 3600 default in the regenerated config template. The
  priority route's prompt in the encrypted hermes capture was retemplated from the retired Bash alerter's
  placeholders to the same three-line layout the pns route carries. Decision made for the operator on
  2026-09-14 (commit d17ce40a in PR #613): the `priority` hermes route now carries the pns key instead of
  the retired Bash alerter's, because every escalation page answered 401 under the old key; the alert
  drainer's store was measured empty (pending_alerts 0) and nothing writes to it any more, so nothing
  queued was lost; revert that commit if you want the old key back. Operator steps: run a full
  `chezmoi apply` with KeePassXC unlocked (rebuilds and installs `pns` with the escalation and the
  `stale` subcommand, rewrites `~/.config/pns/config.toml` with `[nag] stale_after_secs = 3600`, decrypts
  the new `~/.hermes/config.yaml` carrying the priority route's layout A prompt); run
  `hermes gateway restart` so the priority route stops rendering the retired alerter's template (the
  restart drains in-flight runs for up to 180 seconds); no restart is needed for `pns-daemon` itself,
  since each job spawn re-executes `std::env::current_exe`; before this PR, the drainer
  (`~/.local/libexec/osquery/drain-undelivered-alerts.sh`, on `com.webdavis.osquery-alert-drainer`) still
  posted alerter-shaped bodies to `priority`, but the store held zero rows and all three Bash monitors'
  LaunchAgents already ran `posture` subcommands, so no live page path was affected; observe one
  escalation on purpose, either by staying blocked past the hour or by backdating a session's
  `blocked_since` in `~/.local/state/pns/pns.db` and running `pns stale` by hand, which prints
  `1 stuck session(s); one page attempted each` or a held-back count if the gate suppressed it, and
  `nothing is stuck` on a second run; leave `graphify-out/graph.json`, rebuilt by the post-commit hook,
  out of any related PR. Open questions: whether the default of escalating after 60 minutes, with no
  explicit opt-in, is the right default given the nag beside it defaults off; whether the recap section
  from the design's second pull-request bullet, deferred here, should be scheduled to surface a
  suppressed escalation later or left as an unmentioned self-resolving block; whether a fire suppressed
  while away should re-arm itself one window out instead of staying a one-shot, if missed pages while
  away matter more than the extra spawns; and whether the escalation's `blocked <n> minutes, no answer`
  wording should share one renderer with the nag's more compact `<n>m` form. The full `chezmoi apply` and
  a `hermes gateway restart` both ran on 2026-09-15. Still owed: one deliberate escalation, either by
  staying blocked past the hour or by backdating a session's `blocked_since` and running `pns stale` by
  hand.

- [x] 2026-09-14: the hermes unattended-upgrades route was renamed to `uu`, merged in
  [PR #592](https://github.com/webdavis/dotfiles/pull/592)
  (`chore(hermes): rename the unattended-upgrades route to uu`, merged), moving the shipped `[records]`
  URL, the commented-out `failure_webhook` example and the apply-time route check to `/webhooks/uu`
  together with the Discord channel rename. Closed 2026-09-15:
  [PR #592](https://github.com/webdavis/dotfiles/pull/592) merged on 2026-09-14 and
  [PR #626](https://github.com/webdavis/dotfiles/pull/626) renamed the route again to `uu-runs` on
  2026-09-15, matching the Discord channel.

- [x] 2026-09-14: the pns session-attribution, threads and stale-block escalation design was written and
  merged as [PR #593](https://github.com/webdavis/dotfiles/pull/593)
  (`docs(specs): pns session attribution, threads and stale-block escalation design`, merged), the spec
  behind PR #612 and PR #613, at
  `docs/superpowers/specs/2026-09-14-pns-session-attribution-and-threads-design.md`; one Discord thread
  per session was verified unbuildable on hermes 0.17.0 webhooks and stays an upstream ask. Closed
  2026-09-15: [PR #593](https://github.com/webdavis/dotfiles/pull/593) merged, and the two pull requests
  the spec describes, #612 and #613, are both merged too.

- [ ] 81. Settle the Discord channel model in configuration (operator rulings 2026-09-14 and 2026-09-15).
  Every channel is `#<project>-<stream>` and never a bare project name, and each project gets
  `#<project>-dev` for continuous integration, pull requests, GitHub notifications and agent session
  threads, covering dotfiles, pns, uu, posture, homelab, justdavis-ansible, essential-feed-case-study,
  scalebar, netpulse, plantpulse and casually-concerned. `#github-notifications` is the catch-all, and
  `#priority` is the severity channel for anything critical from any source, which means rare, actionable
  and worse if ignored, posted once with the subject in its header. The notification channels are
  `#pns-events`, `#uu-runs`, `#posture-pages` and `#general`, while `#pns-recap` and `#uu-failures` were
  deleted. Routing is two-axis: the subject picks the channel and severity overrides it to `#priority`.
  The Discord category is `Projects`, and one `repo -> channel entry` map in the pns config is shared by
  the GitHub source and by session events.

- [ ] 82. Give pns its own Discord destination. A `pns` Discord bot exists, with View Channels, Send
  Messages, Create Public Threads, Send Messages in Threads, Embed Links and Read Message History, no
  privileged intents, not public and guild-install only, and it stays offline until that destination
  ships: one thread per agent session, recaps into the project channels, and the slash commands
  `/pns pending`, `/pns approve` and `/pns reject`. Repository channels are delivered by this bot and
  never by a hermes route. Its secrets are the KeePassXC entries `Discord (Uriel) :: Bot Token (pns)`,
  `Discord (Uriel) :: Public Key (pns)`, `Discord (Uriel) :: Application/User ID (pns)`, and one
  `Discord (Uriel) :: Channel ID (#<project>-dev)` per project plus `(#github-notifications)`.

- [ ] 83. Route a failed upgrade to `priority`. The producer event carries a kind, health or agent, and
  pns maps that kind to a route, so `uu` never names a route itself; the agent triage then lands under
  that page in the same channel. The `#uu-failures` channel was dropped on 2026-09-15. Operator step:
  delete the two vault entries `Hermes :: Webhook Secret (#uu-failures)` and
  `Discord (Uriel) :: Channel ID (#uu-failures)`.

- [ ] 84. Build the posture critical-page explainer, three pull requests, on the design merged in
  [PR #618](https://github.com/webdavis/dotfiles/pull/618). The explanation posts in the same channel as
  the page it explains and directly under it, moving into the page's own thread once the pns Discord bot
  of task 82 exists. The accepted defaults are that the explainer runs with zero tools
  (`platform_toolsets.webhook: ["no_mcp"]`), refuses after six explanations in an hour, never puts a
  command in its text, and that pns sends a request id on every hermes post.

- [ ] 85. Build the pns GitHub source, four pull requests, on the design merged in
  [PR #620](https://github.com/webdavis/dotfiles/pull/620), whose channel names `#github-<repo>` and
  `#github` are superseded by `#<project>-dev` and `#github-notifications` under task 81. The baseline
  polls the notifications API with the classic token
  `GitHub (Webdavis) :: Personal Access Token (pns notifications)`, which carries the `notifications`
  scope only and no expiry, and push arrives later through the existing Cloudflare tunnel with a separate
  receiver process. The GitHub colours are configurable in the pns config, defaulting to purple for a
  pass and orange for a failure, and three dedicated lamps carry them, `3F - Studio - HCL2`,
  `3F - MBedroom - HCL2` and `2F - Kitchen - HCD5`, each with `shows = ["github"]` and nothing else. On
  GitHub itself, Actions notifications are set to On GitHub with failed-only off, and Dependabot alerts
  to On GitHub plus CLI.

- [ ] 86. Finish the live coverage of the five hermes routes. The 2026-09-15 check covered `pns-events`,
  `priority` and `posture-pages` with real posts. `uu-runs` gets its first live post at the next weekly
  `uu` run, and `general` has no producer in this repository, so it stays unproven until something posts
  to it.

- [ ] 88. Give a storm one combined explanation instead of one per finding. Approved by the operator
  2026-09-15, alongside the answers recorded in
  `docs/superpowers/specs/2026-09-15-posture-explainer-amendment.md`. Task 84's per-finding cap cannot
  solve spam by itself: any number low enough to avoid spam is low enough to hide findings, and twenty
  distinct failures already means the machine is in trouble. The useful message at that point is one that
  says so and lists them, not twenty separate explanations. posture's own pages already arrive uncapped
  today, so a storm already reaches the operator on that leg, and a combined message would improve it
  too. Not yet started.

- [ ] Revalidate the old Docker/profile, trigger, network and artifact-copy assumptions against supported
  Hermes interfaces. Preserve restricted host access and outbound connectivity, no host secrets, and
  untrusted evidence handling. The old plan includes unverified flags and prompt-based output checks;
  those do not establish sandbox isolation or enforce the promised output limits. Review the actual
  controls and their acceptance checks before building. If supported integration cannot provide them,
  report that gap instead of patching Hermes. Use the existing research at
  `~/Documents/Sandboxed_Agent_Prompt_Injection_Research_20260603/report.md` as a review input; it is not
  a missing research assignment. Revalidation written 2026-09-14 against installed Hermes v0.17.0
  (upstream a4091e49, committed 2026-06-25) and Docker 29.7.2; it lives at
  `docs/research/2026-09-hermes-sandbox-revalidation.md`. Verdict: reject the old plan's containment
  story as written. Hermes's own `SECURITY.md` calls terminal-backend isolation the posture for an
  otherwise-trusted operator and whole-process wrapping (its own container image and Compose setup, or
  NVIDIA OpenShell for layer-7 outbound policy) the supported posture for content the operator does not
  control, which an osquery artifact is by definition; it also states that no approval gate, output
  redaction, pattern scanner or tool allowlist is containment. Seventeen controls each carry a
  supported-or-gap verdict and an acceptance check, and no Hermes file was modified. Supported:
  capability reduction (`--cap-drop ALL` then `DAC_OVERRIDE`, `CHOWN`, `FOWNER`, plus `SETUID`/`SETGID`
  unless `docker_run_as_host_user`), `no-new-privileges`, `--pids-limit 256`, no implicit host
  environment in the container, per-profile `HERMES_HOME` with backend pinning through kanban
  `--assignee`, a dispatch-time `--toolsets` pin taken from the assignee profile's
  `platform_toolsets.cli`, and five distinct kanban terminal events that already make timeout and failure
  distinguishable from an all-clear. Gaps: three automatic read-only mounts (the profile's skills
  directory, five cache directories, and skill-declared credential files); `container_persistent` and
  `docker_persist_across_processes` both default true, and `cleanup` in persist mode is a no-op, so the
  container is left running and reused per `(task-id, profile)` with the task id collapsed to `default`,
  making the box neither ephemeral nor session-isolated; measured full outbound egress plus reachability
  of the home gateway and `host.docker.internal` from Hermes's own flag set, with `tools/url_safety.py`
  proven to be a pre-flight check on Hermes-owned URL tools rather than any container control; tirith
  enabled in the live config with `fail_open: true` while the binary is not installed; and dangerous
  commands auto-approved in the unattended worker context, the opposite of the old plan's assumption. Two
  corrections to the recorded review: the old "two routes fire on one incoming webhook" recipe is false,
  because `_handle_webhook` resolves one route from the path and runs one handler; and upstream's attach
  capability is the agent-callable `kanban_attach` and `kanban_attach_url` tools, not a `kanban attach`
  subcommand, and neither exists at the installed revision, where uploading is a dashboard action. Two
  useful discoveries: the container needs no outbound access at all, because the model call is made by
  the host controller process, so `terminal.docker_extra_args: ["--network=none"]` is free and was
  measured to leave `docker exec` and a writable `/workspace` intact; and the kanban notifier never
  publishes raw completion text, only a one-line summary truncated to 200 characters, while
  `kanban complete --metadata` plus `kanban show --json` / `kanban runs --json` hand a trusted publisher
  owned by this repository a structured contract it can validate outside the prompt. One live unguarded
  path was found independent of this workflow: `_deliver_kanban_artifacts` uploads any host file a
  completion summary names, filtered only by a credential and system-path denylist that leaves
  `~/workspaces`, `~/Documents` and `~/.claude.json` deliverable, and `gateway.strict: true` closes it.
  Also found: pns sends `Idempotency-Key`, which the Hermes webhook adapter ignores, so its deduplication
  falls through to a millisecond timestamp, and reusing the alert correlation key as `X-Request-ID` would
  silently drop the second post for an hour on any route; and posture pins the route name `posture`,
  which the live config does not define and the tracked route checker does not cover. Recommended trigger
  shape: the existing `deliver_only` alert post plus a local
  `hermes kanban create --assignee --workspace dir: --max-runtime --max-retries --idempotency-key --json`,
  not a second webhook route, whose default tool surface is four read-only tools and cannot inspect an
  artifact. Nine open questions are listed for the operator; the task is not closed. Full document:
  `docs/research/2026-09-hermes-sandbox-revalidation.md`. Operator steps: (1) Decide the isolation tier
  (open question 1): terminal-backend isolation with `terminal.docker_extra_args: ["--network=none"]` (a
  handful of config keys on this host) or whole-process wrapping under Hermes's own container image or
  NVIDIA OpenShell (the posture upstream names for untrusted input, and the only reviewed option that
  constrains the host-side leg). (2) Decide the permitted-evidence scope (open question 2): whether the
  referenced binary or script and narrowly scoped logs join the plist named by the finding's structured
  `path` field, and the per-investigation byte ceiling. (3) Decide model-provider disclosure (open
  question 3). The live config routes to `provider: openai-codex` at
  `https://chatgpt.com/backend-api/codex`, so an investigator would send attacker-controlled artifact
  text to that endpoint under your account. (4) Decide the credential policy for a dedicated investigator
  profile (open question 4): its own `auth.json` credential through KeePassXC, or a shared one. (5)
  Consider setting `gateway.strict: true` now, independent of this workflow. `_deliver_kanban_artifacts`
  currently uploads any host file a completion summary names, and the denylist leaves `~/workspaces`,
  `~/Documents` and `~/.claude.json` deliverable. Setting it also needs an allowlist root
  (`HERMES_MEDIA_ALLOW_DIRS`) decided for anything you do want delivered. (6) Resolve the tirith
  contradiction: either install it (`brew install sheeki03/tap/tirith`, then declare it in
  `.chezmoidata/system_packages_autoinstall.yaml`) or set `security.tirith_fail_open: false`, because the
  config currently claims a scanner that is not installed and fails open. (7) Approve the route
  reconciliation: add a `posture` route to the encrypted hermes config and extend `expected_routes` in
  `.chezmoiscripts/run_after_68-hermes-log-route-status.sh.tmpl`, and decide whether any future agent
  route gets a documented carve-out from that checker's `deliver_only` invariant. (8) Confirm or revise
  the fixed remediation vocabulary from the old design's section 6 (quarantine the file, disable the
  launch item by label, revert the setting in System Settings), since the publisher can only enforce a
  vocabulary that has been written down. (9) Approve the pns transport change: a per-delivery-attempt
  value in `X-Request-ID` and the alert correlation key in the body. This also fixes the fact that pns's
  current `Idempotency-Key` header is ignored by Hermes entirely. (10) Decide whether to upgrade Hermes
  past the reviewed upstream revision b6b53c69 to get the `kanban_attach` and `kanban_attach_url` tools,
  which would change the evidence-copy-in mechanism, and note that
  `.chezmoiscripts/run_after_59-hermes-config-migrate.sh.tmpl` would carry the config migration. Open
  questions: (1) Execution and network boundary: terminal-backend isolation with `--network=none`, or
  whole-process wrapping? Upstream's own policy names the second for content the operator does not
  control. The first is a handful of config keys on this host; the second is a project. Which tier is
  this workflow held to? (2) Permitted evidence: what may be copied into the box, and who resolves it?
  The old design says only the artifact at the finding's structured `path` field, resolved
  deterministically and never chosen by the agent. Is the referenced binary or script in scope, are
  narrowly scoped logs in scope, and what is the byte ceiling per investigation? (3) Model-provider
  disclosure: the live config routes to `provider: openai-codex` at
  `https://chatgpt.com/backend-api/codex`, so an investigator sends attacker-controlled artifact text to
  that endpoint under your account. Acceptable, a different provider for the investigator profile, or
  wait for a local model? (4) Credential policy for the investigator profile: a profile carries its own
  `auth.json`. Does the investigator get its own credential (blast radius bounded, one more secret in
  KeePassXC) or share an existing one? (5) Evidence-upload posture: turn on `gateway.strict` now? The
  unguarded artifact-upload path is live today for every agent route on this gateway, independent of this
  workflow. It is one key plus an allowlist root decided for anything you do want delivered. (6) Tirith:
  install it or stop claiming it? The config enables a scanner that is not installed, with
  `fail_open: true`, so every command passes unscanned and silently. (7) Route reconciliation: add a
  `posture` route, and extend the tracked checker? Posture pins the name `posture`, the config has no
  such route, and the checker covers only `pns` and `unattended-upgrades`. Separately, if an agent route
  is ever added, the checker's `deliver_only` invariant needs a documented carve-out for it. (8) Advisory
  scope: does the fixed response vocabulary from the old design's section 6 stand (quarantine the file,
  disable the launch item by label, revert the setting in System Settings)? The publisher can only
  enforce a vocabulary that has been written down. (9) Producer transport identifiers: confirm the
  intended split of a per-delivery-attempt value in `X-Request-ID` and the alert correlation key in the
  body. Small pns change, and it also fixes pns's currently-ignored `Idempotency-Key` header.

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
  been approved, and scripts remain Bash. Evaluated 2026-09-14, verdict NO-GO, written up for
  `docs/research/2026-09-xonsh-evaluation.md`. Measured on dresden against xonsh 0.24.2 with
  prompt_toolkit 3.0.53 from a throwaway virtual environment (nothing installed through chezmoi, no
  manifest edited). Fully configured xonsh reaches its first prompt at 1.89x and 1.95x the deployed
  `~/.bashrc` across two interleaved passes, an empty xonsh already costs what all of Bash's
  configuration costs, the first start after any rc edit pays a 2.5 to 3.6 s parse of a 1,064-line
  binding file, and atuin's xonsh hook blocks the prompt on `atuin history end` for 19 to 57 ms after
  every command because xonsh issue 5224 was closed as not planned. Upstream's own install guidance
  advises against xonsh as a login shell and wants a Python isolated from system changes, which the
  Homebrew formula on `python@3.14` plus uu's unattended weekly brew lane is not. The chord surface does
  port (a `Ctrl-g d r` chord that types and runs a command was verified in a live session; 353 bindings
  register in 3 ms), except the 25 escape-prefixed bindings, which prompt_toolkit's vi mode claims.
  atuin, starship, zoxide and carapace are first-class for xonsh; direnv and fnm are not and need a
  hand-rolled hook or an unmaintained 2024 xontrib. Two spillover findings for SP4: bash-completion@2 is
  about 220 ms of Bash's roughly 400 ms startup (the 353 bindings are about 18 ms), and building SP4's
  binding table as shell-agnostic data with a renderer is the one change that would make any future shell
  evaluation cheap. Awaiting the operator's ratification and the pipeline review step; no migration
  specification was written because the verdict makes it moot. Full document:
  `docs/research/2026-09-xonsh-evaluation.md`. Operator steps: (1) Read
  /private/tmp/claude-501/-Users-stephen-workspaces-Ivy-webdavis-dotfiles/1bf0ef19-e242-4ea5-8746-60cb679ebafc/scratchpad/docs-wave/sp5-xonsh.md
  and ratify or reject the NO-GO. The file is already mdformat-clean under .mdformat.toml and idempotent,
  so it can be copied straight to docs/research/2026-09-xonsh-evaluation.md. (2) Append the ledger
  sentences under the SP5 bullet in docs/remaining-work.md and leave the box unticked; the bullet's
  operator_part (decide on a migration) is still open. (3) Send the document through the pipeline review
  step if the 'reviewed spec' half of done_means is to be satisfied by this task rather than a follow-up.
  (4) Decide whether a migration specification is still owed despite the no-go, or whether the evaluation
  discharges it. (5) Optional cleanup of measurement traces, gated on you: trash ~/.local/share/xonsh
  ~/.cache/xonsh (created by xonsh's own first runs: one history JSON file per measured session plus 13
  compiled-code cache entries; inert with xonsh not installed). Nothing was added to Homebrew,
  ~/.local/bin, any chezmoi source file or any managed manifest, and the repository working tree is
  unchanged. (6) Optional: re-run the three-minute reproduction recipe in the document on an idle machine
  if you want absolute numbers rather than ratios; the machine carried a load average of 18 to 102
  throughout this session. Open questions: (1) Ratify the no-go, filing this document beside
  docs/research/2026-07-09-sp4-nushell-evaluation.md so neither shell question is re-litigated? (2) Is
  the migration specification still owed? done_means asks for 'a reviewed spec and a go/no-go verdict';
  this document is the evaluation, not a migration plan. (3) Does SP4 build its binding table as
  shell-agnostic data with a renderer? That is the single change that would make any future shell
  evaluation cheap, and it stands on its own merits. (4) Should SP4 take the bash-completion@2 startup
  win (about 220 ms of roughly 400 ms, measured)? It needs an inventory of completions carapace does not
  cover first; lazy-loading is the conservative middle path. (5) Do you want the comparison re-measured
  on an idle machine before ratifying? The ratios held across two passes at different loads, so the
  verdict does not hang on it, but every absolute millisecond figure is inflated. (6) Is there a workload
  reason to want Python in the shell that this evaluation missed? The value side was judged from the
  repository's contents (Rust tooling, Bash scripts, Python only as formatters), which is the weakest
  evidence in the document. (7) If a migration ever proceeds, is keeping SHELL pointed at Bash agreed?
  Upstream advises it, fzf runs its preview and execute commands with $SHELL -c so the 64 fzf bindings
  require it, and it keeps every non-interactive door untouched.
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
  2026-09-14: checked, recorded in `docs/research/2026-09-babysitter-defect-release-check.md`. Fix #1777
  merged `2026-08-23T19:13:06Z` and is current on `main` (its post-merge blobs for
  `adapter-claude/src/renderer.ts` and `core/src/merge-engine/merge.ts` are the objects `main` holds),
  but no artifact carries it: the newest publish of `@a5c-ai/babysitter-sdk`,
  `@a5c-ai/hooks-adapter-claude` and `@a5c-ai/hooks-adapter-core` is `2026-08-10`, across the `latest`
  tag (still `6.0.0`), the released `6.0.3` the source pins, and the newest `staging` prerelease; the
  newest GitHub release is `v0.0.188` from `2026-06-26` and the `babysitter-claude` plugin release source
  has not moved since `2026-08-10`. The recorded A/B reproducer was rerun against installed `6.0.3` and
  still disagrees (A returns `continue: true` with an empty reason, B returns `decision: "block"` with
  the continuation reason), and the installed `renderStopOutput` and merge engine are pre-fix by direct
  inspection. The execute-bit reproducer was rerun too and also still reproduces (a mode-644
  `.a5c/hooks/on-run-start/` script ran); that defect has no upstream report, so no release can fix it.
  The pin is durable: the apply path reinstalls the exact spec and the weekly uu npm lane has run since
  `f89a9dae` without moving it. Verdict: defer, both holds stay, no duplicate of #1761 filed, no merged
  source fix treated as a release, and neither template changed. Operator half outstanding: both
  templates require both defects resolved, only the Stop defect is reported upstream, and a released
  #1777 would not lift the Codex hold (that adapter drops `decision` on Stop by a deliberate field
  allowlist, not by the omission #1777 fixes). Six open questions are in the document, the first being
  whether the two holds are decoupled. Full document:
  `docs/research/2026-09-babysitter-defect-release-check.md`. Operator steps: (1) Read
  `docs/research/2026-09-babysitter-defect-release-check.md` and rule on its first open question: does
  the enablement condition stay "both defects resolved", or does it decouple so each hold names the one
  defect it waits on? As written it cannot be satisfied, because the execute-bit defect has no upstream
  report. (2) Decide whether to release the 2026-09-06 deferral on
  `~/.claude/pipeline/babysitter-issues/babysitter-issue-execute-bit-v2.md` so that defect can be filed
  upstream. Without a report there is no path to "both resolved" at any date. (3) Decide whether the
  Codex Stop allowlist omission gets its own upstream report, and whether the Codex hold's comment names
  it explicitly instead of citing the same reason as the Claude comment. (4) Agree any edit to the two
  hold comments before it is made. `private_dot_claude/modify_settings.json` still keeps
  `babysitter@a5c.ai` in `$defaultDisabledPlugins` and `private_dot_codex/modify_private_config.toml`
  still sets `plugins."babysitter@babysitter".enabled = false`; nothing was changed. (5) Decide the
  recheck mechanism and cadence: the manual three-command `npm view` recheck in the document, or a uu
  probe comparing published versions against the pin, and whether it is tracked in the ledger entry or in
  a Todoist task. (6) Decide whether a `staging` dist-tag is ever acceptable as the pin. It changes
  nothing today (the newest staging predates the merge) but it sets what a future check may accept. Open
  questions: (1) Does the enablement condition stay "both defects resolved"? As written it cannot be
  satisfied: the execute-bit defect has no upstream report, so no release can fix it. Recommendation:
  decouple, so each hold names the defect it waits on. (2) Is the execute-bit issue draft filed now,
  releasing its 2026-09-06 deferral? Without that, "both resolved" has no path forward at any date. (3)
  Is the Codex Stop allowlist omission reported upstream, and does it join the Codex hold's condition
  explicitly? Today the Codex comment cites the same "adapters both drop Stop's decision" reason as the
  Claude comment, but a released #1777 fixes only the Claude side. (4) When a release lands that fixes
  only the Stop defect, does babysitter get enabled in Claude Code while the execute-bit behavior stands?
  The two defects differ in kind: one is a broken feature, the other runs repository-supplied scripts
  under an unrelated approval. (5) Does the repository get a release watcher, or does this stay a manual
  recheck? If manual, on what cadence, and does it live in the ledger entry or in a Todoist task? (6) Is
  a `staging` dist-tag ever acceptable as the pin? The answer decides what a future check may accept,
  even though it changes nothing today. (7) Once the fix is installable, does the Codex Stop path get
  retested? #1777's core half will start sending `continueSession: false` to Codex on a block, and
  whether Codex reads that as holding the turn or ending the session is untested.

### SP7 scope recovered from the roadmap and Todoist

- [ ] Reconcile the package-manager audit (#11), npm/uv cleanup drift (#20), Homebrew rollback (#12),
  remaining macOS settings (#17), and optional shell command generation (#91). Review the existing
  installers and uu producers before proposing additions. Keep package removal and rollback decisions
  explicit; installation tracking does not authorize removal of all undeclared packages. Reconciled
  2026-09-14 against the current installers, with nothing installed, removed or applied.
  [#11](https://github.com/webdavis/dotfiles/issues/11) CLOSED as completed: every package manager in use
  has a declaration and a runner, `.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl`
  consuming all of `packages.macos` (17 taps, 132 formulae, 55 casks, 5 mas apps, 5 uv tools, 15 npm
  packages on node 24), and measurement found 15 of 15 declared npm packages and 5 of 5 uv tools
  installed. pipx is superseded by uv (`5036f485`, 2026-03-28) and the issue's `trash-cli` example is a
  declared formula at 0.24.5.26; volta is gone (`a902c341`, 2026-08-10); cargo was excluded from that
  issue by operator instruction, leaving only the rustup install, which
  `run_once_before_20-install-rustup.sh.tmpl` does, plus weekly `[lanes.cargo]` and `[lanes.rustup]`
  reporting (`f073bfce`, 2026-09-09). `~/go/bin` does not exist and `gem list --user-install` is empty,
  so both guessed managers had nothing to declare. Todoist
  [6gfVJCVHWvqJ8Jpv](https://app.todoist.com/app/task/6gfVJCVHWvqJ8Jpv) was completed with that evidence.
  The other four stayed OPEN because each one's remainder is an operator decision.
  [#20](https://github.com/webdavis/dotfiles/issues/20) keeps a real gap (no removal pass in the uv or
  fnm stages) but loses its volta bullet and its line numbers, and the measured drift argues against the
  cleanup pass it proposes: uv drift is zero, and all 3 undeclared npm globals would have been destroyed
  by a diff-and-uninstall tonight, since `corepack` ships with the runtime while
  `@oliverames/mcp-server-for-ynab` 5.2.0 and `gnhf` 0.1.49 were installed 2026-09-13 and are already
  scheduled for declaration by lines 1919 and 2161 of this file. A report of undeclared globals on the
  existing weekly lanes was recommended instead of a remover.
  [#12](https://github.com/webdavis/dotfiles/issues/12) should not be built as written: on Homebrew
  7.0.1-3-g67f689a `brew pin` accepts `--cask`, so the local-tap workaround is obsolete for pinning,
  `brew list --pinned` is empty, `bash-completion@2` and `postgresql@17` already cover the versioned
  formula case, keepassxc is a cask at 2.7.12 past the 2.7.11 that motivated the ask, and the version
  history a rollback would read already exists as the brew lane's `name<TAB>state<TAB>before<TAB>after`
  rows (`uu/crates/uu-adapters/src/lanes/changes.rs:124`), whose TSV has never been published on dresden.
  Only a downgrade is genuinely missing. [#17](https://github.com/webdavis/dotfiles/issues/17) is
  delivered for `defaults write` (`dd04c215` and `409dd2a6`, 2026-05-05, 15 records plus 4 killall
  targets, with `just D` and `just defaults-capture` over helpers deployed 2026-08-10) and unstarted for
  pmset, which appears in no data file; its three values are already live on AC (`sleep 0`,
  `tcpkeepalive 1`, `lowpowermode 0`) but undeclared, so a fresh machine gets none of them. Three
  `sudo: true` `enforce` records for `.chezmoidata/macos_system_setup.yaml` were drafted on the issue
  rather than committed, because they need sudo and an apply.
  [#91](https://github.com/webdavis/dotfiles/issues/91) stays parked with every premise intact: atuin
  18.22.0 still ships `atuin ai inline`, `[ai] enabled = false` matches in source and deployed config,
  and ollama plus `qwen3.5:4b` keep the self-hosting test one config file away. Its two Hub-free
  neighbors, `atuin hook install` and `atuin mcp`, are unadopted in every managed harness config and were
  recommended for a separate ruling ahead of the assistant question. One new finding, operator-gated:
  `~/.cargo/bin/fd` 8.4.0 shadows the declared Homebrew `fd` 10.5.0 because `~/.cargo/bin` precedes
  `/opt/homebrew/bin` on the managed shell's PATH, so the correct declared binary is unreachable; `nu`
  0.44.0 and `selene` 0.26.1 sit beside it, all three already reported by the cargo lane on 2026-09-09.
  Nothing was deleted. Full measurement:
  `~/workspaces/backups/2026-09-14T04-34-55.sp7-package-audit.backup.txt`. Owed from the operator: (1)
  Rule on #20: document npm and uv as install-only (the issue's third option, which is what happens
  today) versus building a cleanup pass. Recommendation is install-only plus an undeclared-globals report
  on the existing uu npm and uv lanes, because a diff-and-uninstall pass tonight would have removed all 3
  undeclared npm globals wrongly (corepack is node-bundled; the YNAB server and gnhf are deliberate
  2026-09-13 installs the ledger already routes into the fnm declaration).; (2) Approve or deny the one
  removal candidate on #20: `cargo uninstall fd-find` to stop ~/.cargo/bin/fd 8.4.0 shadowing the
  declared Homebrew fd 10.5.0. Same decision for the two other stale undeclared crates,
  `cargo uninstall nu` and `cargo uninstall selene`. Nothing was removed.; (3) Decide #12 in two parts:
  (a) approve or deny a `pinned:` key in .chezmoidata/system_packages_autoinstall.yaml that the runner
  turns into `brew pin` calls and the uu brew lane honors, now buildable on upstream's cask-capable
  `brew pin` rather than a local tap; (b) say whether an actual downgrade mechanism is wanted at all, or
  whether reading ~/.local/state/homebrew-weekly-upgrade/last-upgrade-changes.tsv by hand is enough until
  a second incident.; (4) Approve the three pmset records drafted in the #17 comment for
  .chezmoidata/macos_system_setup.yaml (`/usr/bin/pmset -c sleep 0`, `-c tcpkeepalive 1`,
  `-c lowpowermode 0`, each `sudo: true` and `tier: enforce`). They were not committed because they need
  sudo and an apply, which the operator owns; the live machine already holds all three values, so nothing
  is urgent.; (5) Rule separately on the two Hub-free atuin features in #91, ahead of the parked
  assistant question: whether to adopt `atuin hook install claude-code` (records an agent's commands into
  atuin history) and `atuin mcp` (serves history search over MCP). Keep `[ai] enabled = false` either
  way.; (6) Run `just lint-check` before committing the ledger paragraph above, since mdformat wraps at
  105 columns and rewrites in place.
- [ ] Reconcile existing Nix installer maintenance (#10) only as needed to preserve optional Nix package
  and project-flake use. This is separate from the rejected nix-darwin macOS-management transition.
  Reconciled on 2026-09-14 and [#10](https://github.com/webdavis/dotfiles/issues/10) closed as completed.
  The migration it asked for had already shipped on 2026-05-15 in `58bbb7d2`, which replaced
  `DeterminateSystems/nix-installer-action@main` with `NixOS/nix-installer-action@main` in
  `.github/workflows/lint.yml` and updated the `README.md` install command and the `dot_bashrc.tmpl`
  comment. Its message read "Solves #10", which GitHub does not accept as a closing keyword, so the issue
  outlived the work by four months. Same-day `6a3da6fb` added the repair LaunchDaemon and `3426adcf`
  tracked the user `nix.conf` that re-enables `nix-command` and `flakes`, which the upstream installer
  leaves off. `2550e8be` (2026-08-05) later deleted `flake.nix`, `flake.lock` and `treefmt.nix`, removing
  the continuous-integration Nix install step with them, and `c34fc502` (2026-08-09) restored the repair
  hook into `libexec` as `systems.nixos.nix-installer.nix-hook`, wired through
  `.chezmoidata/macos_system_setup.yaml` at tier `enforce`. Live verification passed: `/etc/nix/nix.conf`
  carries the NixOS installer header, `nix-installer 2.34.6` and `nix 2.34.6` match `/nix/receipt.json`,
  `/nix/nix-installer self-test` exited 0 across `sh`, `bash` and `zsh`, `nix eval --expr '1 + 1'`
  returned 2, and a throwaway single-output flake evaluated to `ok`, so optional packages and per-project
  flakes both still work. nix-darwin and sops-nix were not reopened. Todoist
  [6gfVJ9rXQ85xr7qM](https://app.todoist.com/app/task/6gfVJ9rXQ85xr7qM) was already complete and took the
  evidence comment. One leftover needs the operator:
  `/Library/LaunchDaemons/systems.determinate.nix-installer.nix-hook.plist` is still on disk and still
  registered, identical to the repo-managed plist apart from its label domain and a missing trailing
  newline, and booting it out needs sudo. Owed from the operator: (1) Decide whether to retire the stale
  Determinate LaunchDaemon left over from the pre-2026-05-15 installer. It is a byte-equivalent duplicate
  of the repo-managed hook, so leaving it is harmless; retiring it is two sudo commands the repo will
  never issue itself: `sudo launchctl bootout system/systems.determinate.nix-installer.nix-hook` then
  `sudo trash /Library/LaunchDaemons/systems.determinate.nix-installer.nix-hook.plist` (or `sudo rm` if
  trash cannot reach /Library). Confirm with
  `launchctl print system/systems.determinate.nix-installer.nix-hook` returning not-found afterwards; the
  repo-managed `systems.nixos.nix-installer.nix-hook` must stay loaded.; (2) No other action is owed.
  Nothing here needs a chezmoi apply: `chezmoi status` reports no pending nix targets, and the deployed
  repair hook and installed plist already match source.
- [ ] Revisit the roadmap's bandwhich/doggo/ouch evaluation, remaining shell quick wins and optional Tart
  clean-machine environment. `MANPAGER` and Git's `autocorrect = prompt` already exist. VM creation
  remains operator-gated. Re-rule the old documentation/archive tasks S1/S2/S4 against current files.
  Researched 2026-09-14, written to `docs/research/2026-09-sp7-tool-and-quickwin-verdicts.md`. **doggo:
  adopt**, but on a reason the filed task did not have. From 1.4.0 (`MatchDomainNameservers`, commit
  `3111835`, in the Homebrew version) it parses `scutil --dns` instead of `/etc/resolv.conf`, so
  `doggo dresden.tail2f2430.ts.net` routes to Tailscale's `100.100.100.100` and names the resolver that
  answered, where `dig +short` on the same name returns empty with exit 0. That is the
  MagicDNS-versus-hosts-pin question this repo owns. `dscacheutil -q host` stays the tool for the
  hosts-pin half. **bandwhich: decline.** Upstream requires elevated privileges and its `setcap` escape
  is Linux-only, while `/usr/bin/nettop` already reports per-process and per-connection bytes with remote
  addresses, unprivileged, with a machine-readable mode. It would also need a pns change:
  `shell_is_interactive` is a hardcoded prefix list that `sudo bandwhich` defeats. **ouch: decline on
  taste**, since bsdtar/libarchive 3.7.4, `7zz` (Rar and Rar5), zstd, lz4, brotli, xz, zip and bzip2
  already open all 14 of its formats. **Quick wins: two left.** `dot_fzf_bindings:53` sets
  `FZF_DEFAULT_COMMAND` with `--no-ignore` but no `--hidden`, so Ctrl-T reaches none of the 53 files
  under `.chezmoiscripts/` (8,712 of 116,165 files missing). The fix is `--hidden` on the existing
  ripgrep line, not the filed fd swap: fd measured 1.17 ± 0.32 times faster (inside the noise), and PATH
  resolves an undeclared `~/.cargo/bin/fd` 8.4.0 over the declared Homebrew 10.5.0. Deleting the override
  for fzf's built-in walker is semantically correct but measured at 0.9 to 5.2 s against ripgrep's 0.65
  to 0.83. `batman` as `MANPAGER` is declined: `nvim +Man!` costs 209.8 ms ± 123.5 ms and keeps the
  editor's keymaps and `:Man` navigation. Every other item on the 2026-04-14 list is shipped, dead with
  tmux and the flake, or taste; `git maintenance start` gets an explicit decline for scheduling a job
  chezmoi does not declare. **Tart: retire P13 as written, defer the rehearsal.** The `sequoia-runner`
  image name and macOS version are both stale (`macos-tahoe-*` is current), and the act-runner rationale
  closed when actionlint was adopted, which that research doc itself recommended. Licensing permits it
  (personal workstations royalty-free; macOS agreement section 2.B(iii) allows two instances). A
  rehearsal cannot cover the privacy grants, LuLu system-extension approval, OverSight consent or Touch
  ID that dominate the fresh-machine runbook, while `chezmoi apply --dry-run --destination <scratch>`
  ("In dry-run mode, scripts are not executed") plus the 2026-09-08 render-and-run ruling already answer
  the will-the-apply-fail question. Noted: `tart` and `act` are both declared, installed and unused.
  **S1: decline as written** (no `docs/archive` or `.gitkeep` was ever created, and the move would break
  `docs/research/` paths in eleven files under `docs/`). **S2: moot** without S1; no `docs/archive` rule
  exists in `CLAUDE.md`. **S4: superseded** (`409dd2a` is on `origin/main`, nothing unpushed on `main`,
  no `Closes #17` trailer anywhere), and its `gh issue close 17` fallback must not be taken, because #17
  is live work in this section's first bullet. Operator decisions remain open on the doggo install, the
  picker fix's timing against SP4, Tart's disposition, and whether `tart` and `act` leave the package
  declaration. Full document: `docs/research/2026-09-sp7-tool-and-quickwin-verdicts.md`. Operator steps:
  (1) Read docs/research/2026-09-sp7-tool-and-quickwin-verdicts.md and rule on the four adopt/decline
  calls. (2) If doggo is approved: run `brew install doggo`, then add `doggo` to
  .chezmoidata/system_packages_autoinstall.yaml under packages.macos.homebrew.formulae between `direnv`
  and `dust`, then a full `chezmoi apply` (no by-name apply, the known-good manifests need the full run).
  Confirm with `doggo dresden.tail2f2430.ts.net`, which must return 100.77.192.92 and name
  100.100.100.100 as the resolver. (3) Decide the fzf picker fix: add `--hidden` to the ripgrep branch of
  dot_fzf_bindings line 53 now, or hold it for SP4, which is chartered to rewrite that file. (4) Decide
  Tart: retire P13 as written (recommended), or charter the clean-machine rehearsal as its own project
  with the stated interactive ceiling. VM creation stays operator-gated and nothing was pulled. (5) If
  P13 is retired, decide whether `tart` (line 157) and `act` (line 48) come out of the formulae
  declaration; both are installed and unused, and act's own research doc recommended against it. (6)
  Confirm issue #17 stays open. Do not run the S4 fallback `gh issue close 17`; the SP7 section's first
  bullet lists remaining macOS settings (#17) as live work. (7) Decide the four taste items if wanted:
  Catppuccin for bat, Ghostty `quick-terminal-size`, Ghostty background blur, AeroSpace
  workspace-to-monitor assignment. (8) File the two incidental findings as their own items rather than
  folding them into a quick-wins bullet: Server Message Block file sharing is listening on port 445
  (`com.apple.smbd` enabled, untracked in the ledger and in macos_posture_controls.yaml), and
  `~/.cargo/bin` holds an undeclared fd 8.4.0 shadowing the declared Homebrew 10.5.0 plus `nu` and
  `nu_plugin_core_match` despite the ratified nushell no-go. Open questions: (1) doggo: adopt? It is the
  one recommended addition, one line between `direnv` and `dust`, install first then declare per the
  Homebrew agent workflow. (2) bandwhich and ouch: accept the declines, or overrule either on taste? If
  bandwhich is wanted, the pns skip-list question needs answering first: extend `shell_is_interactive`
  and teach it about `sudo`, or accept a notification after every session? (3) The picker fix: apply
  `--hidden` now, or hold it for SP4, which will rewrite dot_fzf_bindings anyway? (4) Should `dot_fzf*`
  be added to treefmt's shellcheck and shfmt includes? The file is currently linted by neither, despite
  carrying its own `# shellcheck shell=bash` directive. (5) Tart: retire P13, or charter the
  clean-machine rehearsal as its own project? If retired, should `tart` and `act` come out of the
  formulae declaration? (6) Issue #17: confirm it stays open. This record recommends against the S4
  fallback that would close it, contrary to the 2026-05-15 spec's own instruction. (7) The four taste
  items: Catppuccin for bat, Ghostty `quick-terminal-size`, Ghostty background blur, AeroSpace
  workspace-to-monitor assignment. Each is a small diff with no correctness argument. (8) Out of scope
  but untracked anywhere: Server Message Block file sharing is listening on port 445 (`nettop` shows
  `tcp4 *:445 Listen`, `launchctl print-disabled system` shows `com.apple.smbd => enabled`). Should it be
  off, or should a posture control assert its state? The ledger's existing exposure section covers SSH
  only. (9) Out of scope: `~/.cargo/bin/fd` 8.4.0 shadows the declared Homebrew fd 10.5.0, and `nu` plus
  `nu_plugin_core_match` are installed there despite the ratified nushell no-go. Clean up, or leave?
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
  its old evaluation-only scope to the requested setup work. A second tooling wave 1 attempt (2026-09-14)
  got further: the `feat/openspec-config` worktree folded the stray `graphify-out/graph.json` (commit
  `c33ff50e`), merged `origin/main` clean, passed `just ship` on the documented one-rerun retry (the
  first run's failure was an unrelated `uu` signal-timing test with several sibling worktrees running
  `just ship`/`just test-rust` concurrently on the same machine), pushed, and opened
  [PR #586](https://github.com/webdavis/dotfiles/pull/586)
  (`feat(openspec): track the global OpenSpec configuration`, branch `feat/openspec-config`, not merged).
  CI never ran: the check suite sat at 0 check-runs for about 45 minutes across the PR's open, a
  close/reopen, and a fresh empty commit (new head `a454043d`), while sibling PRs' `lint` runs started
  and finished normally in the same window, and a direct check-suite rerequest returned 404 (insufficient
  permission over a suite owned by another app). This is not the pre-authorized `Could not resolve host`
  retry case, so the branch was left open rather than merged without a real result. `origin/main` has
  since advanced past this branch's merge point, and a fresh merge-tree check now shows real conflicts in
  `docs/remaining-work.md` and `.chezmoidata/macos_posture_controls.yaml`, outside the pre-approved
  conflict scope, so no further merge was attempted. Gated on GitHub Actions/Blacksmith CI recovering
  (verify with `gh-axi pr checks 586`), then merging `origin/main` again, rerunning `just ship`, pushing,
  and resuming.
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
  configuration; nothing was installed in the audit. A second tooling wave 1 attempt (2026-09-14) got
  further: the `feat/ynab-mcp` worktree merged `origin/main` clean, passed `just ship` on the documented
  one-rerun retry (the first run's failure was three unrelated `pns` dispatch test panics in a workspace
  this branch's diff never touches), pushed, and opened
  [PR #585](https://github.com/webdavis/dotfiles/pull/585)
  (`feat(mcp): declare the read-only YNAB server for Claude Code and Codex`, branch `feat/ynab-mcp`, not
  merged). CI's `lint` check then failed for real: GitHub Actions run `34811977849` failed a
  `pns-adapters` timing-budget test
  (`persistence::sqlite::ledger::tests::failures::busy_ledger_writes_refuse_within_the_budget_without_recording_sensitive_content`,
  `assertion failed: started.elapsed() < Duration::from_millis(100)`) in a crate this PR's diff (limited
  to `.chezmoidata/system_packages_autoinstall.yaml`, `CLAUDE.md`, `modify_private_dot_claude.json`,
  `private_dot_codex/modify_private_config.toml`) never touches. This is not the pre-authorized
  `Could not resolve host` retry case, so the branch was left open rather than rerun or merged. Gated on
  confirming whether the failure is CI-runner load or a real `pns-adapters` regression, then rerunning
  the failed workflow once confirmed as flake and resuming from the poll step. On the operator's ruling
  of 2026-09-14, the YNAB MCP server was opened to full write access across all three clients in
  [PR #606](https://github.com/webdavis/dotfiles/pull/606)
  (`feat(mcp): turn YNAB writes on for Claude Code, Codex and Claude Desktop`, merged):
  `YNAB_ALLOW_WRITES` moved from `"0"` to `"1"` in the Claude Code and Codex declarations, and the same
  server was added to the Claude Desktop config template, which had carried no YNAB entry at all; all
  three name the fnm-lane binary and read the token from the KeePassXC entry "YNAB :: Personal Access
  Token". A stdio tool listing confirmed 38 tools with writes off and 62 with them on, and exactly nine
  tool schemas require `confirmed: true`, which an agent fills in itself rather than stopping to ask per
  call. The client-side rate limiter stays at its upstream defaults (190 requests/hour, burst 10).
  `YNAB_BUDGET_ID` was deliberately left unset. All three templates rendered headless and validated, and
  `just lint-check` and `just test-unit` were green. Operator steps left: unlock KeePassXC and run a full
  `chezmoi apply`; quit and reopen Claude Desktop; in Claude Code, list the YNAB tools and confirm 62.
  Open question: whether the YNAB account holds more than one budget, since `YNAB_BUDGET_ID` unset
  resolves each call against the most recently accessed budget, which is a footgun with more than one
  budget; `list_budgets` answers this in the first session after the apply. The full `chezmoi apply` ran
  and passed on 2026-09-15 and [PR #585](https://github.com/webdavis/dotfiles/pull/585) merged. Still
  owed: quitting and reopening Claude Desktop, and confirming the 62-tool listing.
- [ ] Reconcile [Backpass](https://github.com/kunchenguid/backpass) configuration and finish any missing
  integration, requested 2026-09-12. It is installed, declared in npm, and
  `dot_config/backpass/config.json` matches the deployed copy, directing user instruction edits to
  `.chezmoitemplates/global-agent-rules.md`. Preserve that source ownership and the interactive review of
  proposed edits. Decide how accepted skill extractions enter the managed store and provenance lock,
  instead of leaving undeclared directories. CLI upgrades already use uu's npm lane; do not reinstall
  merely to complete the old evaluation task. Reconciled on 2026-09-14. `dot_config/backpass/config.json`
  is byte-identical to the deployed `~/.config/backpass/config.json` and still points `user.memoryFiles`
  at `.chezmoitemplates/global-agent-rules.md`; `backpass status --scope user` confirms it live,
  reporting this checkout as no write target, that partial at 14,884 tokens against the default
  5,000-token budget across 83 instructions, and an empty cache (0 transcripts fingerprinted, 0 evidence,
  no proposal), so backpass has never run here and no extraction existed to reconcile. backpass 0.1.22
  and acpx 0.15.1 are installed on the fnm npm lane and declared; nothing was reinstalled.
  Skill-extraction ownership is settled on the existing vendored lane, with no new provenance kind, no
  `skillsDir` redirect and no automation: the write target already is the managed store
  (`USER_CONFIG_DEFAULTS.skillsDir` is `.agents/skills` under `$HOME`), a configured `skillsDir` is
  honored only when that directory already exists and otherwise falls back to the store silently,
  backpass plants `~/.claude/skills` only when the path is missing and warns on a real directory (which
  is what it is here, 77 declared symlinks), and an unpromoted extraction survives everything because
  uu's `absorb_store_entries` walks `roster.tracked_names()` only and `live-reconcile.sh` prunes hermes
  profile links but never a real store directory. The gap was reach and version control rather than
  survival, and `backpass apply` is interactive and the only writer, so the promotion belongs to the same
  sitting as the acceptance: the recipe now lives in `docs/runbooks/agent-skills-store.md` as a "Locally
  extracted (`backpass`)" provenance section plus a pointer in its "Adding a skill" step 1, with the
  CLAUDE.md paragraph corrected to match. Both files are source-only, so no apply is involved, and the
  two edits wait uncommitted on `main` for review. The audit also found four live store entries in no
  lock table (`composio-cli`, `hyperframes-media`, `website-to-hyperframes`, `website-to-video`), each
  carrying an undeclared `~/.claude/skills` symlink and alive through every weekly publication since
  2026-07-03; they are now tracked in
  [6hW4MHMmqR4pgM8v](https://app.todoist.com/app/task/6hW4MHMmqR4pgM8v) and nothing was changed. All four
  are now resolved: [PR #595](https://github.com/webdavis/dotfiles/pull/595)
  (`feat(skills): declare composio-cli and hyperframes-audio`, merged 2026-09-14) declares `composio-cli`
  (tier `core`) and adds `hyperframes-audio` (tier `on-demand`, the thirteenth upstream HyperFrames core
  skill the roster lacked) with their lock rows and chezmoi symlinks, and trashes the other three
  undeclared directories (`hyperframes-media`, `website-to-hyperframes`, `website-to-video`), each
  confirmed retired or renamed upstream rather than left undeclared. Evidence recorded on
  [6hPV483GJgGHX95M](https://app.todoist.com/app/task/6hPV483GJgGHX95M), which stays open for no-mistakes
  and firstmate. Owed from the operator: (1) Review and commit the two doc edits as their own commit,
  separate from the ledger commit:
  `git -C /Users/stephen/workspaces/Ivy/webdavis/dotfiles diff CLAUDE.md docs/runbooks/agent-skills-store.md`,
  then commit those two paths (docs scope). No `chezmoi apply` is involved, both files are in
  `.chezmoiignore`. mdformat was already verified idempotent on both.; (2) At the next
  `backpass apply --scope user` that accepts a skill extraction, promote it in the same sitting per the
  new runbook section: copy `~/.agents/skills/<name>/` into `dot_agents/skills/<name>/`, add the `tiers`,
  `hermesProfiles` and (if withheld) `claudeDelivery` rows, declare
  `private_dot_claude/skills/symlink_<name>`, add the `skillOverrides` entry and committed
  `agents/openai.yaml` overlay when on-demand, run `just test`, then apply.; (3) Rule on the four
  undeclared store entries in Todoist 6hW4MHMmqR4pgM8v: declare each (its real lane plus lock rows plus
  the Claude declaration) or record in the lock comment which app owns it. Nothing was deleted or
  declared overnight.; (4) Optional decision, needs a full apply if taken: whether to raise
  `budgetTokens` in `dot_config/backpass/config.json` above the 5,000 default, since the shared partial
  already measures 14,884 tokens and a first run will otherwise propose deletions to fit.
- [ ] Plan installation and configuration of [no-mistakes](https://github.com/kunchenguid/no-mistakes)
  and [firstmate](https://github.com/kunchenguid/firstmate), requested 2026-09-12. Neither has a managed
  declaration in the reviewed source. The operator confirmed that Backpass, no-mistakes and firstmate all
  refer to `kunchenguid`'s repositories. Verify any existing checkout before installing. Firstmate is a
  repository-based agent distribution, not a standalone CLI; configure its Herdr backend and review its
  treehouse dependency alongside the existing worktree workflow. Select harnesses, project roots,
  validation commands and review/merge authority explicitly. Preserve existing hooks and per-invocation
  destructive-action approvals when configuring no-mistakes' Git proxy and repair behavior. Track tools,
  upstream skills and their update paths through existing uu lanes or a producer where needed. Reuse
  [6hPV483GJgGHX95M](https://app.todoist.com/app/task/6hPV483GJgGHX95M) for these and Backpass. Planned
  2026-09-14 in `docs/superpowers/specs/2026-09-14-no-mistakes-firstmate-installation-design.md`.
  Verified first that nothing is installed: no binary, no checkout, no declaration. Recommendation is a
  three-phase adoption, dotfiles only, with firstmate held behind its own gate because it requires
  no-mistakes 1.46.0 or newer and four npm packages that are missing here. treehouse is a bottled
  homebrew-core formula, so it needs only a `formulae` line and the existing uu brew lane; no-mistakes
  installs from its own installer and gets one uu `command` lane running `update -y`; firstmate is a
  clone at `~/workspaces/firstmate` updated by `/updatefirstmate`, with no uu lane because that skill
  restarts live mates. The plan records seven measured intersections with systems this repository already
  owns, four of which need action: the no-mistakes daemon writes an unmanaged LaunchAgent that the
  launchd persistence detector default-denies until `posture allowlist add` pins it (and re-pins after
  every update that rewrites the plist); `no-mistakes init` drops undeclared real directories into both
  skill stores, which uu's fanout leaves untouched, so they should be recorded as a third app-owned case
  beside `cua-driver` and `graphify`; the gate's bare repository inherits the user-wide `core.hooksPath`
  and its receive hooks never fire unless upstream's `config --worktree` isolation runs, which was proven
  to work on git 2.55.0 here and is the first acceptance check; and every crewmate commit would fire the
  `prepare-commit-msg` hook's nested `claude -p`, so `SKIP_AI_COMMIT=1` belongs in firstmate's
  `config/launch-env-allowlist`. Automatic pull requests conflict with the `~/.claude/commands/pr.md`
  body contract, since `pr.template` appends a protected evidence appendix by design, so the plan pushes
  with `-o no-mistakes.skip=pr,ci` and keeps `/pr`. Two measured side-findings: `treehouse --root .`
  would put worktrees inside the chezmoi source tree and poison every render through nested
  `.chezmoidata`, and the `herdr` call at the bottom of `dot_bashrc.tmpl` runs during no-mistakes'
  `$SHELL -l -i -c 'env -0'` daemon probe, prefixing 28 bytes of terminal escapes onto the first
  environment record (PATH survives intact, so this is benign today; a `[[ -t 1 ]] || return` guard is
  proposed as a separate one-line change). Eleven assumptions and seven open questions are listed; Open
  Question 5 still gates adoption and nothing is installed. Full document:
  `docs/superpowers/specs/2026-09-14-no-mistakes-firstmate-installation-design.md`. Operator steps: (1)
  Read the design at docs/superpowers/specs/2026-09-14-no-mistakes-firstmate-installation-design.md and
  answer Open Question 5: whether firstmate is adopted at all, and which harness runs its primary
  session. Phases 1 and 2 stand alone if it is declined. (2) Decide the four operator-only items in the
  Open questions section: bypass or auto for config/claude-permission-mode, whether the recurring posture
  allowlist page after each no-mistakes update is acceptable, whether the reviewed pull-request body
  contract stays with /pr, and whether the no-mistakes skill is recorded in
  docs/runbooks/agent-skills-store.md as a third app-owned case alongside the Backpass extraction
  question. (3) Approve or reject the eleven assumptions, in particular the curl installer over
  `go install`, leaving ~/.no-mistakes/config.yaml untracked and hand-maintained, ci.revalidate_repairs
  true, and commands.test being `just test-unit` with the heavy suites declared as gates. (4) If phase 1
  and 2 are approved, authorize the agent-side work: `brew install treehouse` plus its alphabetical line
  in .chezmoidata/system_packages_autoinstall.yaml, the `[lanes.no-mistakes]` command lane in
  dot_config/uu/private_config.toml.tmpl, the two NO_MISTAKES\_\* exports in dot_bashrc.tmpl, and a
  committed .no-mistakes.yaml on main. Each is a separate commit; nothing is applied by an agent. (5)
  Decide the separate one-line `[[ -t 1 ]] || return` guard before the herdr auto-attach in
  dot_bashrc.tmpl. It is its own change, not part of the tool install. (6) Run the no-mistakes installer
  yourself and then, in order: `no-mistakes doctor`, `no-mistakes daemon status`,
  `git -C ~/.no-mistakes/repos/<id>.git config --get core.hooksPath` (must not answer
  ~/.config/git/hooks), and `posture allowlist add com.kunchenguid.no-mistakes.daemon.<suffix>`. The
  hooksPath check is the one most likely to fail. (7) Reuse Todoist task 6hPV483GJgGHX95M for these and
  Backpass, per the ledger entry, and record the phase split on it. Open questions: (1) Which harnesses
  and Hermes profiles are the audience for no-mistakes and firstmate? The /no-mistakes skill reaches
  Claude Code and every harness reading ~/.agents/skills automatically with no per-harness choice, so the
  live part of the question is whether firstmate is adopted and which harness runs its primary session
  (Claude Code and Codex are the only installed candidates; upstream's co-primaries are Claude Code, Grok
  and Pi). (2) Is firstmate adopted, or is phase 3 declined? Adopting it costs four new global npm
  packages (chrome-devtools-axi, gh-axi, quota-axi, tasks-axi), a fleet of unattended agents under
  --dangerously-skip-permissions, a reserved herdr workspace label `firstmate`, and a second Stop hook on
  the primary session. (3) bypass or auto for firstmate's config/claude-permission-mode? bypass matches
  this machine's existing global bypassPermissions mode; auto is the stricter posture upstream added for
  a captain who refuses unattended bypass. (4) Is the recurring launchd allowlist page acceptable? Every
  no-mistakes update that rewrites its daemon plist invalidates the pinned SHA-256 and pages once until
  `posture allowlist add` re-pins it, because the writer refuses to store an unpinned tuple. (5) Which
  repositories beyond webdavis/dotfiles should the gate eventually cover? Each needs its own
  .no-mistakes.yaml on its own default branch, its own worktree_roots placement, and its own validation
  commands. The Obsidian vault is excluded on evidence because Obsidian Git auto-commits it on a timer.
  (6) Does the reviewed pull-request body contract stay with /pr? pr.template cannot satisfy it: upstream
  states the protected evidence appendix is appended in addition to the template and that this does not
  satisfy a policy requiring only the template's headings. Keeping /pr means pushing with -o
  no-mistakes.skip=pr,ci; changing the answer means amending ~/.claude/commands/pr.md. (7) Should the
  undeclared no-mistakes skill directories in ~/.claude/skills and ~/.agents/skills be recorded in
  docs/runbooks/agent-skills-store.md as a third app-owned case? They work with no lock table row, but
  leaving them unrecorded means the lock stops describing the store. This is the same decision the
  Backpass entry already carries and the two should be answered together.
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
  [6hVpPJWQq79vMM3v](https://app.todoist.com/app/task/6hVpPJWQq79vMM3v). Shipped 2026-09-14 in
  [PR #578](https://github.com/webdavis/dotfiles/pull/578) (`feat/dockerfile-roast`, merged
  `bd88f70d5e3bc4396f947e412d2c8b75fe375e23`). Assumptions made in the operator's place: droast is a
  homebrew-core formula rather than a third-party tap, so it is declared as a bare `- droast` in
  `formulae:` instead of the briefed tap/trusted_taps entries; hadolint stays beside droast.nvim because
  droast does not lint `RUN` bodies as shell; the lazy.nvim spec carries no `opts` (upstream self-calls
  `setup()` with sane defaults) and no commit pin (the pin lives in `lazy-lock.json` only); and
  `graphify-out/graph.json` was restored after committing to keep the diff to the three intended files.
  Review found one SEV-2 finding, that the plugin comment's stated reason for keeping hadolint was false
  since droast can lint `RUN` bodies as shell through its own `--shellcheck` flag; fixed by rewriting the
  comment with measured droast/hadolint overlap facts in commit `17c9ce7a`. Gated on more than the
  routine apply: after `chezmoi apply`, open Neovim and run `:Lazy restore` (or `:Lazy sync`) to actually
  install `droast.nvim` at its locked commit, since editing `lazy-lock.json` alone installs nothing.
  Optional follow-ups the implementer left open: a smoke check (open a Dockerfile, `:w`, expect droast
  and hadolint diagnostics together, `:DroastQuickfix` fills the quickfix list) and removing the
  verification harness at `/private/tmp/dr` when no longer wanted. The full `chezmoi apply` ran and
  passed on 2026-09-15. Still owed: `:Lazy restore` in Neovim, without which `droast.nvim` is not
  installed.
- [x] Review native Neovim language-server configuration, requested 2026-09-12. Installed Neovim is
  `0.12.5`; `dot_config/nvim/lua/plugins/lsp.lua` already uses `vim.lsp.config()` and `vim.lsp.enable()`,
  with no legacy `require("lspconfig").SERVER.setup()` calls. Keep nvim-lspconfig for maintained server
  definitions and Mason/mason-lspconfig for installation and native activation.
  [Upstream guidance](https://github.com/neovim/nvim-lspconfig#readme) deprecates the old setup
  framework, not the definitions plugin. No migration is needed. none-ls and lsp-format still provide
  external linting/formatting; their separately deferred replacement is not a prerequisite for droast.
- [ ] Reconcile the remaining tool evaluations for strix, apple/container, minutes and gnhf. Check prior
  removals and rejections before proposing adoption. Herdr remains the selected multiplexer. The rejected
  git-absorb and deferred gh-dash/companion tools are not additions to #530. Verdicts recorded 2026-09-14
  in `docs/research/2026-09-sp7-tool-evaluations.md`, one per tool, each citing the decision it respects.
  `minutes`: ADOPT, already adopted, so the evaluation item is superseded. It was declared 2026-07-27 in
  `ad7124f3` with `silverstein/tap` in both `taps` and `trusted_taps`, the cask is installed at 0.26.1
  (0.26.2 available, uu's brew lane owns it), and its vault link is made by its own
  `minutes vault setup --subdir`. The one gap: `~/.local/bin/minutes` is an unmanaged symlink into
  `/Applications/Minutes.app`, created before the cask, so a fresh machine gets the app and no command on
  PATH; the cask's own caveat names the fix (`brew install silverstein/tap/minutes`), and because
  `dot_bashrc.tmpl` prepends `~/.local/bin` the stray symlink must go with it. There is nothing to manage
  in configuration: `~/.config/minutes/config.toml` does not exist. `gnhf`: ADOPT, already decided; the
  install work stays in its own entry above. It is installed at 0.1.49 and absent from the `fnm` packages
  list, and the two lanes are asymmetric: uu's npm lane runs `npm update -g` with no roster so it is
  upgraded weekly, while the apply-time lane installs only declared packages into a new node prefix, so a
  node bump would drop it. Three corrections to that entry: "(pinned, like the other npm tools there)"
  does not describe the list (one of fifteen entries is pinned, with a comment explaining why it is the
  exception), upstream rolls a failed iteration back with `git reset --hard`, which makes `--worktree`
  and the `gnhf/` branch prefix constraints rather than options, and the license is MIT on GitHub with no
  license field in the npm registry record. `apple/container`: DECLINE as an addition. Homebrew core
  carries it at 1.4.1 (the old note's 1.3.0 is stale) and macOS 26.2 clears its minimum, but it has no
  declared consumer here, Docker Desktop 29.7.2 is installed, running and declared, and `act` is the only
  declared Docker consumer. The recorded coupling "strix via uv+container as one decision" is void:
  `strix-agent` 1.6.2 depends on the Docker software development kit for Python and its installer checks
  a live daemon, while the `container` README documents Open Container Initiative images and no Docker
  application programming interface; `socktainer` 1.2.1 is the bridge if Docker Desktop is ever retired,
  which is a separate decision this record does not open. `strix`: DEFER, with the adoption path
  pre-cleared. Apache-2.0, release 1.6.2 of 2026-09-05, and nothing technical blocks it:
  `uv tool install strix-agent` joins the existing `uv` list (same package-name-versus-command split as
  `graphifyy`/`graphify`) and uu's `[lanes.uv]` upgrades it weekly, the sandbox runs on the Docker
  Desktop already here, and the `OPENROUTER_API_KEY` already in KeePassXC matches upstream's own example
  provider. The `strix.ai/install` script is ruled out by the same reasoning as plannotator's installer:
  read without running, it appends `export PATH` to the chezmoi-managed `~/.bashrc`. What is missing is
  operator input: a target, a decision about unattended credit spend, and where `LLM_API_KEY` is exposed
  (a per-repository `.envrc` through direnv or a vault-reading wrapper, not the managed shell). Also
  recorded: `gh-dash`'s deferral has a tracked source
  (`docs/superpowers/plans/2026-04-19-dotfiles-improvements-v2.md:3963`, "cut from v2 scope"), while
  `git-absorb`'s rejection has no record in this repository beyond this ledger line. Full document:
  `docs/research/2026-09-sp7-tool-evaluations.md`. Operator steps: (1) Read
  `docs/research/2026-09-sp7-tool-evaluations.md` and confirm or override the four assumptions it states
  in your place: the minutes formula over a declared symlink, apple/container declined as an addition
  rather than evaluated as a Docker Desktop replacement, strix deferred rather than declined, and the
  gnhf npm entry left unpinned. (2) Answer the strix question first, since it is the only one that gates
  work: is there an asset to point it at, and is unattended OpenRouter spend on it acceptable? A no
  closes the item permanently for one line of edit. (3) If the minutes formula path is taken, a follow-up
  task adds `silverstein/tap/minutes` to the formulae list in
  `.chezmoidata/system_packages_autoinstall.yaml`; you then run the full `chezmoi apply` (agents do not
  apply) and remove the stray `~/.local/bin/minutes` symlink by hand with `trash`, since this repository
  builds no removal mechanisms. (4) Decide whether `gnhf` gets declared in the `fnm` list now, ahead of
  the rest of its install task: it is installed today and a node bump would silently drop it. Open
  questions: (1) minutes command-line interface: declare the `silverstein/tap/minutes` formula and remove
  the `~/.local/bin/minutes` symlink by hand, or declare that symlink in chezmoi instead? (Assumption
  taken: the formula, because `~/.local/bin` is prepended in `dot_bashrc.tmpl` and would otherwise shadow
  the formula's copy forever.) (2) apple/container: is declining the addition the whole answer, or was
  the original intent to retire Docker Desktop? The second reading is a separate design task costing
  `container` plus its kernel install plus `socktainer`, proven against `act` first. (3) strix target: is
  there an asset to point it at (a repository of yours, a homelab service, a deployed application), and
  is unattended OpenRouter credit spend acceptable on it? (4) strix key exposure: per-repository `.envrc`
  through direnv, a wrapper that reads KeePassXC at call time, or an export in the managed shell? The
  third widens a vault secret to every interactive shell, where hermes keeps the same key scoped to one
  `.env` file. (5) gnhf pin: pin the npm entry (safer for a 0.x tool that runs unattended and calls
  `git reset --hard`) or leave it unpinned (matches fourteen of the fifteen entries in that list)? (6)
  git-absorb: its rejection has no record in this repository beyond the ledger sentence. Should that
  sentence stand as the record, or is a one-line reason worth adding beside it so the question stops
  recurring?
- [ ] Review the still-open bqf, dadbod/dadbod-ui and dblab/database-workflow tasks, including the older
  PostgreSQL shell/client configuration task and SSH `Host *` client-hardening task. Source declares
  PostgreSQL, but that alone does not supply the requested client configuration. bqf and dadbod are
  absent from the current source/live plugin locks. Avoid duplicating completed xcodebuild, dap and
  neotest infrastructure; the language-specific gaps above remain separate. Adjudicated 2026-09-14 in
  `docs/research/2026-09-quickfix-database-ssh-backlog.md`. The five tracker items are `6ggcw5hwM9Gp7XQv`
  (bqf), `6ggcw5v939gHcFvv` plus `6ggcw63vFWchgRjv` (dadbod and its UI), `6ggf2WjFmgQqRqRM` (dblab),
  `6gfVJFgXvG9mJ96M` (PostgreSQL workstation setup) and `6ggcXcJ6jMcm25HM` (SSH `Host *`); no separate
  database-workflow task exists, so that phrase names the dblab item. bqf, dadbod, dadbod-ui and
  dadbod-completion are absent from both the source and the deployed `lazy-lock.json`, which are
  byte-identical at 93 plugins. Verdicts: **adopt bqf**, because `:ReviewLedger` in
  `dot_config/nvim/lua/config/keymaps.lua` ends in a native `:copen` and every prerequisite is already
  installed (Neovim 0.12.5, fzf 0.74.4, nvim-treesitter, nvim-hlslens by the same author); **defer
  dadbod**, a four-plugin cluster plus a missing `sql` treesitter parser serving a two-row database, and
  blocked on choosing a credential form (the function-valued `vim.g.dbs` is the only one compatible with
  the KeePassXC model, and `:DBUIAddConnection` writes a plaintext URL to `~/.local/share/db_ui` outside
  any gitleaks gate); **defer the dblab declaration**, though it is in homebrew-core at 0.50.0 needing no
  tap, and its `--save-as` Keychain profiles, `--readonly` mode and native SSH-tunnel flags fit this
  repository better than dadbod's inline URLs. PostgreSQL: **adopt** the `path_prepend` line
  (postgresql@17 is keg-only and `/opt/homebrew/bin/psql` does not exist, so `psql` is not invocable
  today, four months past the stated daily-use date) and the `dot_psqlrc`; **close** `PSQL_EDITOR` as a
  second copy of the existing `EDITOR` export, and **close** the `~/.pgpass` item, because every line of
  `pg_hba.conf` is `trust` and a password file has nothing to authenticate with. **Close the SSH task as
  written**: both examples it names, modern ciphers/key exchange and no agent forwarding, are already
  OpenSSH 10.0p2 defaults measured with `ssh -G`, and of the four settings that would be real changes
  only `HashKnownHosts yes` is safe, since `RequiredRSASize 3072` would ignore a 2048-bit key in the
  running agent, `IdentitiesOnly yes` would stop the agent offering the non-default GitHub and homelab
  key names no block declares, and `PasswordAuthentication no` would remove the factory-fresh Pi
  fallback; a `Host *` block is safe only at the end of the file, verified by experiment with `ssh -F`.
  Recorded separately, outside every task: an undeclared `postgresql@17` server starts at every login
  from an untracked `brew services` LaunchAgent with `RunAtLoad` and `KeepAlive`, listening on loopback
  with `trust` for every role including superuser, holding `open_brain` (one `thoughts` table, 2 rows,
  pgvector 0.8.2) whose owning project was not found in this repository or the memory index;
  `homebrew.mxcl.ollama.plist` is in the same position. Five operator questions remain open, so this
  entry stays unticked. Full document: `docs/research/2026-09-quickfix-database-ssh-backlog.md`. Operator
  steps: (1) Read
  /private/tmp/claude-501/-Users-stephen-workspaces-Ivy-webdavis-dotfiles/1bf0ef19-e242-4ea5-8746-60cb679ebafc/scratchpad/docs-wave/sp7-db-workflow.md
  and answer the five open questions; four of the five verdicts are recommendations that name their
  alternative rather than closed decisions. (2) Answer open question 5 first, because it is the only one
  with a security dimension: an undeclared postgresql@17 server runs at every login on loopback with
  `trust` authentication for every role including superuser, so any agent session on this machine can
  read and write `open_brain` without passing a permission gate. Three dispositions are laid out in the
  document. If the choice is to pause it, `brew services stop postgresql@17` is the whole step, and the
  same question applies to `homebrew.mxcl.ollama.plist`. (3) Identify what created `open_brain` (one
  `thoughts` table, 2 rows, `vector(384)` embeddings, pgvector 0.8.2, 7990 kB). It appears nowhere in
  this repository or in the agent memory index, and a grep across ~/workspaces did not finish inside a
  60-second budget, so this needs your knowledge rather than another search. (4) Answer open question 1,
  which decides whether nvim-bqf lands at all: should `:ReviewLedger` keep opening the native quickfix
  window, or be rerouted to Trouble with a one-word change to dot_config/nvim/lua/config/keymaps.lua? (5)
  If you want the free dblab trial that closes both the dblab and dadbod items: `brew install dblab`,
  point it at `open_brain`, look at it once. Nothing is declared and nothing is committed by the trial.
  (6) Before typing any `dblab --pass ... --save-as ...` command, check that the HISTIGNORE regex in the
  KeePassXC entry `Dotfiles (bashrc) :: HISTIGNORE Regex` covers a `--pass` argument. That pattern feeds
  atuin's `history_filter` and is not readable from the repository, so only you can check it. (7) Nothing
  here needs a chezmoi apply. No code was written, nothing was installed, and every probe in the document
  is read-only. Open questions: (1) Should `:ReviewLedger` keep opening the native quickfix window? If
  yes, nvim-bqf is the adopt recommended here. If you would rather it opened Trouble, nvim-bqf closes
  instead and the change is one word in dot_config/nvim/lua/config/keymaps.lua. This is the assumption
  the bqf verdict rests on. (2) Did the 2026-05-16 PostgreSQL daily-use date in task 6gfVJFgXvG9mJ96M
  actually arrive? There is no ~/.psql_history on this machine and `psql` is not on PATH, which reads as
  no. If the work happened somewhere else, the dadbod and dblab verdicts flip from defer to a real
  evaluation. (3) Trial dblab before declaring it, or declare it now? The trial is one `brew install` and
  commits nothing. Declaring now is one alphabetical line in
  .chezmoidata/system_packages_autoinstall.yaml and keeps the weekly bundle current, at the cost of
  maintaining a formula that may go unused. (4) Is a two-setting `Host *` block worth adding to
  private_dot_ssh/config, or should task 6ggcXcJ6jMcm25HM just close? The honest content is
  `HashKnownHosts yes`, optionally a `MACs` list without the SHA-1 entries, and a comment recording why
  `RequiredRSASize`, `IdentitiesOnly` and `PasswordAuthentication` are deliberately absent. Separately:
  do you want the 168 plaintext entries already in ~/.ssh/known_hosts retro-hashed with `ssh-keygen -H`,
  which makes that file permanently unreadable to you? (5) What is `open_brain`, and should its server
  keep running? Two rows behind `trust` authentication on loopback, restarted at every login by an
  untracked `brew services` LaunchAgent, reachable by any agent session on this machine without a
  permission gate. Keep it and record the agent plus the `trust` decision in the repository; keep the
  formula but stop the service until there is a workload; or retire it, which means stopping the service,
  then removing the declaration and the data directory in that order, because removing the declaration
  first puts the formula in scope for `brew bundle cleanup --force` while /opt/homebrew/var/postgresql@17
  stays on disk. The same question applies to homebrew.mxcl.ollama.plist. (6) Should the psqlrc be a
  plain `dot_psqlrc` or a chezmoi template? Plain is recommended because it carries no secret, but a
  template would let it branch on host or pull a value from the vault later. Stated here because the
  document chose plain in your place.
- [x] Reconcile completed or superseded Todoist review items with evidence: Neotest parser findings
  (`6hR5vFgFXgjHVGMv`, `6hRHmWWqFjr5RmVv`), Atlas (`6hR59h6FQWpv33p8`), Overseer (`6hR5Gg6XXRFVhfpg`) and
  the pns builder-input fix (`6hV7jMW3jMGWcCMv`, commit `0b55db04`). Current source contains their fixes
  or recorded replacement decisions. Preserve Atlas's deliberate omission of checkout mappings. Update
  the remaining stale acceptance claims rather than reinstalling. B111's temporary-repository fsmonitor
  exclusions and fixture fix already shipped; the proposed worktree reaper was rejected. Likewise, the
  old graphify exclusion-removal proposal predates the current committed `graphify-out/graph.json` and
  post-commit rebuilding policy; do not delete that map or its exclusions as unfinished cleanup.
  Reconciled 2026-09-14: all five items are closed in Todoist, each carrying a comment with the commit
  that fixed or superseded it. The three findings on
  [6hRHmWWqFjr5RmVv](https://app.todoist.com/app/task/6hRHmWWqFjr5RmVv) are fixed:
  `webdavis/neotest-bashunit` `ddd53e6`, `36854cb` and `f2f085b` discover the parenthesis-free
  `function test_name { ... }` spelling, `ddd53e6` and `82dd3de` narrowed the passing-test output claim
  to what bashunit actually reports (no message), and `4b6e8d70` removed the obsolete local
  `just test-unit` version-gate claim from `CLAUDE.md`. The adapter pin this repository runs,
  `4c07ce1c94a53be4a5e8c959fd759432c81039f6` set by `ffea4072`, contains all four adapter commits. Its
  parent [6hR5vFgFXgjHVGMv](https://app.todoist.com/app/task/6hR5vFgFXgjHVGMv) held that one subtask and
  no others, so it closed on the same evidence.
  [6hR59h6FQWpv33p8](https://app.todoist.com/app/task/6hR59h6FQWpv33p8) closed with the checkout-mapping
  finding recorded as deliberately rejected: `847500ff` dropped `pulls.repo_config.paths` as a protection
  policy and corrected the worktree rationale in both `dot_config/nvim/lua/plugins/atlas.lua` and
  `docs/research/2026-09-atlas-nvim-evaluation.md`, while the atlas treesitter exclusion was superseded
  by the general `is_installable` rule in `852466ed`, made per-call in `86ee65f5` and pinned by
  `dot_config/nvim/tests/plugins_treesitter_spec.lua`.
  [6hR5Gg6XXRFVhfpg](https://app.todoist.com/app/task/6hR5Gg6XXRFVhfpg) closed with one landed commit per
  finding: `e036fea5`, `a495cdd1`, `4b114b13`, `66d4daf0`, `007653ae`, `e8e7a4a9`, `f709cd47` and
  `b892a438`. [6hV7jMW3jMGWcCMv](https://app.todoist.com/app/task/6hV7jMW3jMGWcCMv) was already complete
  (2026-09-13) and took the evidence comment for `0b55db04`, which widened all four Rust builders to
  every extension and scoped them to `crates/` so cargo's own `target/` stays out of the hash. Nothing
  was reinstated or deleted: `graphify-out/graph.json` remains committed behind `.gitignore` lines 30 to
  33, `treefmt.toml` line 22 and `.chezmoiignore` line 33 with the `.githooks/post-commit` rebuild
  intact, the temporary-repository fsmonitor exclusions shipped in `96ce432a` and `d80dcfd3`, and no
  worktree reaper exists anywhere in source. No open GitHub issue matched any of the five items, so none
  was touched.
- [x] Reconcile [6ggcw4qfqfqjxP3v](https://app.todoist.com/app/task/6ggcw4qfqfqjxP3v), the old tiling
  window-manager task. Issue #14 is already closed as completed (2025-12-28); AeroSpace is declared and
  configured. This needs task-list closure with that evidence, not another installation or issue closure.
  Reconciled on 2026-09-14. Issue #14 was verified closed as completed at 2025-12-28T22:11:00Z with
  `state_reason` `completed`, so no issue closure was needed. AeroSpace is declared as the cask
  `nikitabobko/tap/aerospace` at line 214 of `.chezmoidata/system_packages_autoinstall.yaml` (beside
  `mediosz/tap/swipeaerospace` at line 209), `dot_aerospace.toml` carries 363 lines and 70 main-mode
  keybindings plus service and resize modes, and it is deployed byte-identical to `~/.aerospace.toml`
  (14831 bytes, `diff` clean), with `brew list --cask` showing both casks and
  `/Applications/AeroSpace.app` present.
  [6ggcw4qfqfqjxP3v](https://app.todoist.com/app/task/6ggcw4qfqfqjxP3v) was completed after that evidence
  was posted as comment `6hW4J768XmMCRJvM`, and the parent audit task
  [6hVp9mV63cg2J8PM](https://app.todoist.com/app/task/6hVp9mV63cg2J8PM) got the same evidence in comment
  `6hW4JGmgcQJCmFGM` for its matching line item and stays open for its other gaps. Nothing remains for
  the operator.
- [x] Reconcile stale GitHub issues #8 (Kulala-LS is declared), #9 (gh-notify was superseded), #13
  (Zellij predates the Herdr decision), and #18 (fixed), then align the surviving issues and Todoist
  items with this file. The earlier migration's cutover ledger has all five completion markers dated
  2026-08-10; do not confuse those completed gates with the new posture cutovers. Reconciled on
  2026-09-14. [#8](https://github.com/webdavis/dotfiles/issues/8) closed as completed:
  `@mistweaverco/kulala-ls` is declared at `.chezmoidata/system_packages_autoinstall.yaml` line 275 in
  the single fnm node group, has been on `main` since `fc6d3327` and `7b32adc6` (both 2026-05-05,
  first-parent, no PR to cite), is installed at 1.11.1, and is made idempotent by the `npm ls -g` guard
  at line 293 of `.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl`. Its third checklist
  item was unachievable rather than unmet: the package ships no help mode, and its transport error is
  itself the proof the binary resolves. [#13](https://github.com/webdavis/dotfiles/issues/13) closed as
  not planned: Zellij was never installed or declared, tmux and sesh are both uninstalled, `~/.tmux.conf`
  does not exist, and herdr 0.9.0 carries the issue's one requirement through ten
  `herdr-workspace-jump.jump_*` bindings plus `last_workspace` in `dot_config/herdr/config.toml`. The
  five markers `gate1.done` through `gate5.done` in `~/.local/state/cutover` are all stamped 2026-08-10
  and belong to that migration, not to the posture cutovers.
  [#18](https://github.com/webdavis/dotfiles/issues/18) needed no write: it was already closed earlier
  the same day with its own reconciliation comment. [#9](https://github.com/webdavis/dotfiles/issues/9)
  STAYED OPEN because this line's premise was wrong. gh-notify was not superseded as a tool: it is
  installed at the exact pinned commit `556df2ee`, registered with `gh`, returned five live rows when
  tested, and has a live consumer at `dot_config/nvim/lua/plugins/snacks.lua` lines 44 to 52. What died
  twice was its role as a notification SOURCE, first in
  `docs/research/2026-05-18-github-workflow-notification-trigger.md` and then in pns, which absorbed both
  halves of that redesign: `~/.local/bin/hue-pulse.sh` is gone and the colours are five operator-locked
  constants in `pns/crates/pns-domain/src/pulse.rs`, and Discord is the compiled-in `[plugins.hermes]`
  destination. The undelivered half is the declaration alone: `grep -rn "gh extension"` over the source
  tree returns nothing, so gh extensions are the one install surface with no declaration and no runner,
  and the choice between declaring the pinned install and dropping the dashboard section is the
  operator's. [6gfVJ9P5vpX64JhM](https://app.todoist.com/app/task/6gfVJ9P5vpX64JhM) was commented and
  narrowed to that single question, its hue-pulse-blue and Bob-Discord parts recorded as superseded, and
  [6gj9Pwj6PR5HJQ8v](https://app.todoist.com/app/task/6gj9Pwj6PR5HJQ8v), the `~/.tmux.conf` task made
  stale by the same herdr decision, was completed as superseded. Nine open issues survive (#9, #12, #17,
  #20, #91, #192, #193, #194, #195) and this file already carries every one. Owed from the operator: (1)
  Decide issue #9, one of two one-line answers. (1) DECLARE IT: add a gh-extension step to the package
  runner, `gh extension install meiji163/gh-notify --pin 556df2ee` guarded by
  `gh extension list | grep -q meiji163/gh-notify`. The `--pin` flag is confirmed by
  `gh extension install --help` to take a commit for script extensions, and gh-notify is a script
  extension (upstream cuts no releases). Cost: one new declaration surface for one extension. (2) DROP
  THE DEPENDENCY: change `cmd = "gh notify -s -a -n5"` at dot_config/nvim/lua/plugins/snacks.lua line 46
  to a plain `gh api notifications` call, or delete that dashboard section, then close #9 as not planned.
  Either way gh-notify stays installed on dresden; only the fresh-machine path changes.; (2) Optional,
  cosmetic: retitle Todoist task 6gfVJ9P5vpX64JhM. Its title still names all three parts ("Automate
  gh-notify install + hue-pulse blue + Bob Discord notification") while two are recorded superseded in
  comment 6hW4MwR49rgwm29v. No title or description was edited by the agent.; (3) For information, not a
  step: Todoist 6gfVJ7VwcFQvg7xM (P10, "Notify via Bob on long-running shell command completion") is
  still open although dot_bashrc.tmpl now delivers that through pns and [plugins.hermes]. It sits outside
  this ledger line's four issues and was left untouched; it belongs to whichever reconcile task owns the
  pns notification tasks. Closed 2026-09-15: all four issues are closed, the operator took option 2 on #9
  and [PR #619](https://github.com/webdavis/dotfiles/pull/619) dropped gh-notify from the snacks
  dashboard in favour of a plain `gh api notifications` call, and Todoist task `6gfVJ9P5vpX64JhM` was
  closed. The replacement command ran through Neovim the same day and returned five rows at exit 0.
- [x] Resolve scope for the older warden import/quarterly-cleanup tasks and obsolete agent-session
  restoration tasks. The restic script is the operator's learning exercise; keep its later LaunchAgent
  dependent on that work and do not take over writing it without a new instruction. These older tasks
  require disposition, not automatic inclusion in the active implementation queue. Dispositioned
  2026-09-14 with nothing entering the implementation queue. The orphan-doc cleanup
  [6gVRJmHQ3rWJpCcX](https://app.todoist.com/app/task/6gVRJmHQ3rWJpCcX) was already done and is now
  closed: commit `2ce35dd9` deleted `docs/superpowers/plans/2026-04-27-macos-disk-cleanup-plan.md` and
  `docs/superpowers/specs/2026-04-27-macos-disk-cleanup-design.md`, the third listed file was untracked
  and is absent from the checkout, and the canonical copies live in the warden repo at
  `/Users/stephen/workspaces/Ivy/webdavis/warden`. The quarterly cleanup LaunchAgent
  [6gVRJjqWc69XqV75](https://app.todoist.com/app/task/6gVRJjqWc69XqV75) stayed open and blocked rather
  than queued: no reminder script and no `com.webdavis.disk-cleanup-reminder.plist` exist in source,
  `warden` is neither installed nor declared, and its checkout carries 0 commits, so neither
  `warden scan --json` nor the warden database this task relies on exists; its recorded `~/.local/bin`
  target also conflicts with the rule putting launchd-invoked scripts under `~/.local/libexec`. Both
  agent-session restoration tasks were completed as superseded,
  [6ghRWpCV3j3XjxPM](https://app.todoist.com/app/task/6ghRWpCV3j3XjxPM) by the Neovim config import in
  commit `13feac58` and [6ghRWpcXMfW87hrM](https://app.todoist.com/app/task/6ghRWpcXMfW87hrM) by the
  relay-to-pns split in commit `6e214d36`; the 39 transcripts in
  `~/.claude/projects/-Users-stephen--local-share-chezmoi` were left untouched, and the documented `mv`
  would now nest that directory inside the live project directory holding 196 sessions. The restic pair
  was left alone by instruction: [6ggcXXG83fHpM3Gv](https://app.todoist.com/app/task/6ggcXXG83fHpM3Gv)
  remains the operator's learning exercise and
  [6ggcXXPPxm99FRpv](https://app.todoist.com/app/task/6ggcXXPPxm99FRpv) stays dependent on it, with
  `restic` declared and installed at `/opt/homebrew/bin/restic`. The warden-project reading-order task
  [6gVRJg8xGvH4pmR5](https://app.todoist.com/app/task/6gVRJg8xGvH4pmR5) kept its own scope and received
  the corrected document paths.

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
  Researched 2026-09-14; the verdict lands as `docs/research/2026-09-keepassxc-local-boundary.md`.
  Rejected: no supported KeePassXC integration moves this boundary, because every one of them ends in a
  process running as the operator, and the unit of authorization in all three `keepassxc.mode` values is
  the whole database. Measured on dresden: a key-file-only database with `prompt = false` renders
  unattended and silently, which is handing over the vault rather than an approved operation; `builtin`
  returned an empty map for every entry; `open` and `cache-password` both serve the title and attribute
  lookups here; whole-tree `chezmoi status` fails without the vault while `chezmoi managed` and
  path-scoped `status`/`diff` on non-vault targets succeed; and `chezmoi cat` decrypts the five
  age-encrypted targets with no vault at all. The Agent Proxy brokers outbound web requests only and
  writes no files, so it cannot reach file rendering, which matches homelab A6's own note. Two findings
  reframe the boundary: all fifteen rendered targets sit at 0600 under the operator's user id after any
  apply, so the unlock gate buys render freshness and not secrecy; and the source tree, including the
  `hooks.read-source-state.pre` script, is agent-writable, so the instruction stream reaching the
  unlocked vault is not itself gated. Deferred candidate: `keepassxc.mode = "open"` with a YubiKey
  challenge-response factor, the only supported configuration whose unlock software cannot supply,
  pending hardware and a recovery design. `kpxc-cli` is recommended for closure as rejected on the same
  evidence. Seven open questions for the operator, four of them decisions, are listed in the document.
  Full document: `docs/research/2026-09-keepassxc-local-boundary.md`. Operator steps: (1) Read the
  document, then answer the seven open questions at its end. Four are decisions (the reading of
  "supported" that governs the kpxc-cli task, whether to price the YubiKey path, whether the
  deployed-secret exposure is the primary defect and reorders the Infisical work, and whether the source
  tree is a trust boundary); three are cheap checks. (2) Run one command with the vault unlocked to
  settle the cheapest blocker:
  `chezmoi execute-template '{{ keepassxcAttribute "GitHub (Webdavis) :: GPG :: Signing key" "Public Signing Subkey ID" }}'`
  with `keepassxc.mode = "open"` set temporarily. A real custom attribute under `open` mode was not
  verified (keepassxc-cli 2.7.12 cannot create one from the command line), and if it fails the deferred
  YubiKey candidate dies with it, because `dot_gitconfig.tmpl` and
  `dot_composio/private_user_data.json.tmpl` both read custom attributes and `open` is the mode YubiKey
  requires. (3) Delete the dummy probe directory when you no longer want the evidence:
  `trash /private/tmp/claude-501/-Users-stephen-workspaces-Ivy-webdavis-dotfiles/1bf0ef19-e242-4ea5-8746-60cb679ebafc/scratchpad/kpxc-probe`.
  It holds a throwaway KDBX 3.1 database, a key file and three throwaway chezmoi configs, containing only
  the strings `dummy-master-pw`, `dummy-entry-secret` and `simple-secret`. Nothing in the repository
  references it. (4) Approve or reject the one adopt-now item, which is a documentation fix and grants
  nothing: `private_dot_claude/agents/chezmoi-apply.md` tells that agent to run whole-tree
  `chezmoi status` and `chezmoi diff`, and both fail without the vault (measured, exit 1 at the first
  vault read). Path-scoped `status <path>` and `diff <path>` on non-vault targets do work, as does
  `chezmoi managed`. The agent's process should say so. (5) Decide whether to file the two side tasks the
  document surfaced but does not propose: the Moshi pairing token's render-time exposure (half fixable;
  `moshi-hook pair` takes `--token` only, verified, so argv cannot be cleaned), and a test for
  `renders_unsafe` in `scripts/treefmt/shellcheck-rendered-template.sh`, whose
  `test/fixtures/render-coverage/` fixtures are referenced by nothing. Open questions: (1) Which reading
  of "supported" governs the kpxc-cli task at ledger line 2056? If a shim behind `keepassxc.command`
  counts as supported, that task's scope changes and this document's rejection of the browser protocol
  weakens from "not possible" to "possible at the cost of a title-to-URL migration across the vault, for
  no security gain". Recommendation: it does not count, and kpxc-cli should be closed as rejected on this
  evidence. (2) Is the YubiKey path worth pricing? `keepassxc.mode = "open"` with a challenge-response
  factor is the only supported configuration whose unlock cannot be produced by software running as the
  operator, and it works with this repository's existing template calls unchanged. Costs: hardware, an
  experimental chezmoi mode, a recovery design, and the touch authorizes a whole run rather than one
  secret. Recommendation: yes, as its own task, after the exposure question below. (3) Should the
  deployed-secret exposure be fixed before any broker work? Fourteen rendered targets sit at 0600 owned
  by `stephen` and the five age-encrypted targets need no vault at all, so an agent on this machine can
  already read nearly every credential. Recommendation: yes, and it reorders the Infisical tasks behind
  it, because brokering one credential off the laptop while fourteen others sit readable in $HOME buys
  little. (4) Should the source tree be treated as a trust boundary? An agent that adds a `keepassxc`
  call to any template, or edits `.install-password-manager.sh` (wired as `hooks.read-source-state.pre`),
  reaches the operator's unlocked apply, and neither gitleaks nor any test catches it. Recommendation:
  yes, and it belongs in the boundary-definition task, because it needs a mechanism decision rather than
  more research. (5) Is the half fix to the Moshi pairing token worth making? `moshi-hook pair` takes the
  token only as `--token` (verified), so the argument-list exposure cannot be removed without patching a
  third-party tool. The render-time exposure can be removed by fetching at execution time the way
  `run_before_05` does. Recommendation: yes, as a small standalone change, because the token currently
  sits in the source state rather than only in a momentary argument list. (6) Should `keepassxcAttribute`
  be verified against a real custom attribute under `mode = "open"`? The probe used the built-in `Title`
  field because keepassxc-cli 2.7.12 cannot create a custom attribute from the command line. One unlocked
  command settles whether the deferred YubiKey candidate is viable at all. (7) Should the orphaned
  `test/fixtures/render-coverage/` fixtures get a test, or be deleted? The vault-exclusion logic
  (`renders_unsafe`, a transitive includeTemplate walk with a cycle guard) decides which shell templates
  get linted at all, and nothing exercises it. Recommendation: one small test, because a silent
  regression there is a lint gap nobody would notice.
- [ ] Evaluate [kpxc-cli](https://github.com/mietzen/keepassxc-cli) as an unadopted third-party
  candidate: it uses KeePassXC's browser protocol and macOS biometric unlock. Verify entry approval,
  association key protection, revocation and locked-database behavior. It is not a drop-in replacement
  for the current title/attribute-based chezmoi integration: its documented lookups use URLs, and custom
  fields have additional requirements. Do not treat Touch ID support or masked output as proof of
  isolation. Evaluated 2026-09-14 and written up in `docs/research/2026-09-kpxc-cli-evaluation.md`. All
  four behaviors verified from upstream sources (kpxc-cli and `keepassxc-browser-api` at `main`,
  KeePassXC at tag `2.7.12`, chezmoi at tag `v2.72.1`), not from a live install: entry approval is a
  one-time remembered grant per entry and site host, silent on every later read, with a global "Never ask
  before accessing credentials" escape hatch that is not scoped to one client; the association key is two
  secret keys in plaintext in a 0600 `~/.keepassxc/browser-api.json`, shared with keepassxc-ssh-agent,
  authorizing writes as well as reads; revocation exists at Database Settings, Browser Integration
  ("Remove selected key", "Disconnect all browsers", "Forget all site-specific settings on entries") but
  is GUI-only, is a database edit that syncs with the `.kdbx`, and leaves the client's keys on disk; a
  locked database raises the unlock dialog and times out after 30 seconds with stable exit codes 2, 3 and
  4\. Verdict: it cannot replace the current integration. chezmoi's argument vector for
  `keepassxc.command` is fixed and passes a database path and a title, kpxc-cli takes neither; lookup is
  by URL host only, and zero of the 23 entry titles across 16 files and 43 template actions is a
  hostname; re-keying to one shared host would collapse approval to all-or-nothing; and the 3
  `keepassxcAttribute` calls would need `KPH: ` renames plus a global KeePassXC setting. Touch ID gates
  the unlock, not the read, and masked output is a print-time choice over an already-fetched secret, so
  neither is isolation. The age-key script's direct `keepassxc-cli` call and the moshi token in an
  argument vector are untouched by any template-function swap. Not ticked: closure waits on two operator
  decisions recorded in the document. Full document: `docs/research/2026-09-kpxc-cli-evaluation.md`.
  Operator steps: (1) Read
  /private/tmp/claude-501/-Users-stephen-workspaces-Ivy-webdavis-dotfiles/1bf0ef19-e242-4ea5-8746-60cb679ebafc/scratchpad/docs-wave/kpxc-cli-eval.md
  and confirm it lands at
  /Users/stephen/workspaces/Ivy/webdavis/dotfiles/docs/research/2026-09-kpxc-cli-evaluation.md (it is
  already mdformat-clean and idempotent under .mdformat.toml, so it will pass `just lint-check` as
  written). (2) Answer open question 1: state whether kpxc-cli was under consideration for Touch ID
  convenience during applies or for agent credential isolation. The verdict is reject either way, but the
  follow-up differs completely. (3) Answer open question 2: decide whether you want a supplementary
  kpxc-cli path through chezmoi's `secret.command` for new agent-scoped secrets only. My recommendation
  is no, on blast radius (a durable on-disk read-write vault credential plus a second unlock path per
  apply). (4) Optional, only if you want findings 1 through 4 measured rather than source-read: create a
  throwaway .kdbx (never keepass.kdbx), `uv tool install keepassxc-cli`, then run the seven-line sequence
  in the document's "What would change the verdict" section. Do not run `kpxc-cli setup` against the live
  vault; it writes a permanent association key into the database your iPhone syncs. (5) Decide whether
  this ledger item is ticked closed on the reject verdict or kept open as a watch on the unreleased
  `get-database-entries` protocol action (absent from KeePassXC 2.7.12, present only on `develop`). Open
  questions: (1) What problem was kpxc-cli under consideration for: Touch ID convenience during applies,
  or agent credential isolation? The verdict is reject either way, but the first leads to the
  `secret.command` bridge question and the second leads nowhere, with the item closing against the A6
  Infisical direction instead. (2) Do you want a supplementary kpxc-cli path at all, through chezmoi's
  `secret.command`, for new agent-scoped secrets only? This is the one decision deliberately left for
  you. It leaves the existing 43 call sites untouched and costs a durable on-disk read-write vault
  credential plus a second unlock path per apply. Recommendation: no. (3) Should this item be ticked
  closed, or kept open as a watch on `get-database-entries` (the unreleased protocol action that would
  make title filtering possible client side)? (4) Do the two non-template exposure paths get their own
  ledger items now? `run_before_05-restore-age-key.sh.tmpl` calls `keepassxc-cli` directly at execution
  time, and `run_once_after_60-moshi-hook-setup.sh.tmpl` renders the moshi device token into
  `moshi-hook pair --token <token>`, visible in `ps` and in the rendered script body. Neither is affected
  by any decision about kpxc-cli, and both are real exposure today. (5) Is "no audit record of a
  remembered credential read" a finding you want pursued? The setting "Show a notification when
  credentials are requested" exists and is stored, but no browser-path source read consumes it and its
  declaration carries a `// TODO!!` comment. That is four files read, not an exhaustive proof, and it
  applies to the browser extension you already use, not only to a hypothetical kpxc-cli. It bears
  directly on the section's "audit records without secret values" requirement.
- [ ] Define and verify the actual access boundary: permitted secrets and operations, approval duration,
  revocation, audit records without secret values, and denial outside the permitted set. Account for
  arbitrary shell access, readable rendered configuration and agent-editable templates/scripts. Hiding a
  value from chat or putting it in a child environment does not keep it inaccessible to an unrestricted
  process running as the same user. Establish the isolation needed for the claimed protection. Designed
  2026-09-14 in `docs/superpowers/specs/2026-09-14-credential-access-boundary-design.md`. Measured on
  dresden (macOS 25.2, chezmoi 2.72.1, keepassxc-cli 2.7.12): `ps eww -p` and `ps -E -p` print another
  same-user process's full environment and `ps -o args=` its command line, so a child environment is not
  protection; all fifteen rendered targets exist and are same-user readable, and the mode audit is clean
  but irrelevant because the agent runs as the owner; `hooks.read-source-state.pre` points at an
  agent-editable file inside the source tree and fires on `chezmoi diff`, `status`, `managed`,
  `execute-template` and `apply --dry-run`; `keepassxc.command` is unqualified with `$HOME/.local/bin` at
  PATH position 2 ahead of `/opt/homebrew/bin` at 9, and `/opt/homebrew/bin` is mode 0775 owned by the
  operator, so the vault client is replaceable both on PATH and at its absolute path; and chezmoi's
  `cache-password` mode, the effective mode here, writes the master password to that client's standard
  input. Those chain into a pre-review path from an agent-written file to the whole KeePass database,
  opened by the operator running `just d`. Verdict: no claimed protection survives while the agent shares
  the operator's user id, so the design records two tiers. Tier 1 is brokered through the homelab Agent
  Proxy, and its isolation mechanism is a different machine across a network boundary plus Infisical's
  server-side refusal to start when the agent identity can read a brokered secret. Tier 2 is rendered
  locally with no isolation at all, governed instead by the master password never existing in a file,
  operator review of source changes, and rotation. It recommends `keepassxc.mode = "builtin"` and
  removing the source-tree hook as the immediate hygiene floor, `connect` over `run` so the proxy's
  certificate authority private key stays on the home server, and `--unmatched-host block`. It records
  nine denial probes, D1 to D9, of which D7 and D8 can be built now and the rest are gated on homelab A6.
  It also records that Infisical gates audit logs and role-based access control above the Free tier,
  which turns the audit-record and use-without-read requirements into a licensing decision. Eleven
  assumptions and nine open questions are listed; the design is not approved, nothing was built, and it
  waits on the operator's read. Full document:
  `docs/superpowers/specs/2026-09-14-credential-access-boundary-design.md`. Operator steps: (1) Read the
  design at docs/superpowers/specs/2026-09-14-credential-access-boundary-design.md, sections 1 to 6 and
  "The chain these five facts form" first; they are the reason the rest of the document is shaped the way
  it is. (2) Decide the one urgent item ahead of everything else: whether to set
  `keepassxc.mode = "builtin"` in `.chezmoi.toml.tmpl` and remove or repoint
  `hooks.read-source-state.pre`. Together they break the measured chain from an agent-written file to the
  master password, and both are one-line changes. (3) Answer the nine open questions in the design, one
  at a time, and record each answer beside the affected ledger bullet. (4) Confirm the KeePass database
  format and key derivation parameters with
  `keepassxc-cli db-info '/Users/stephen/Library/Mobile Documents/iCloud~com~strongbox/Documents/keepass.kdbx'`.
  This needs the master password, so this could not be measured directly; the design's claim that the
  vault's resistance to an agent reduces to offline passphrase strength depends on it. (5) Decide the
  Infisical licensing question before any homelab A6 work starts: audit logs and role-based access
  control both appear to sit above the Free tier, and the boundary's audit and use-without-read
  requirements both need them. (6) When A6 is deployed, run denial probes D1 through D6 and D9 from the
  design's probe table with dummy credentials, and record the evidence column. D7 and D8 can be run on
  the laptop before A6 exists. (7) Decide whether the twenty-three secret-bearing entries listed in the
  appendix are rotated now on the standing assumption that they are already agent-exposed, or only on
  evidence of a specific compromise. Open questions: (1) Do you approve the isolation claim as stated:
  Tier 1 isolated by a different machine across a network boundary, Tier 2 not isolated at all and
  governed by review and rotation? The ledger asks you to approve the boundary and the isolation claim it
  rests on, and this is that claim. (2) Paid Infisical edition, or accept no audit trail and no granular
  role-based access control? Both appear to sit above the Free tier and both are load-bearing for this
  boundary. (3) Approach C, a separate user id or container for agents: pursue, defer, or reject? It is
  the only route to a true Tier 2 isolation claim, at the cost of the whole single-home-directory
  toolchain. Recommendation is defer and shrink Tier 2 instead, recorded as your choice. (4) Which of the
  twenty-four registered entries move to Tier 1? Reading offered in the design: the Anthropic,
  OpenRouter, Tavily, ElevenLabs, Composio, Discord bot token and Google OAuth entries are brokerable and
  the rest are not. (5) Root-own the Homebrew binary directory to close the last substituted-client path,
  at the cost of breaking the unattended weekly brew lane uu runs? Recommendation is to leave it
  writable, because `builtin` mode removes what the shim was for. (6) Remove the `read-source-state` pre
  hook, or repoint it outside the source tree? Removing loses a best-effort fresh-machine KeePassXC
  install. (7) Any appetite for per-request approval on specific high-value operations? The proxy model
  brokers by destination rather than by request, so naming them may require a different mechanism
  entirely. (8) Rotate all twenty-three secret-bearing entries now on the assumption they are already
  exposed, or only on evidence? Rotation cost is real and spread across many providers. (9) Build the
  boundary checker, or keep the credential register as documentation reviewed by eye? The checker is a
  tool this repository owns, so its behavior is testable and inside the 2026-08-05 test scope ruling, but
  it is still a new mechanism to maintain.
- [ ] Include fresh-machine recovery and current secret exposure paths in that design. The age-key
  restoration script calls `keepassxc-cli` directly, outside template lookup. Moshi's pairing script
  currently places its token in command arguments. Review supported alternatives without printing the
  values or modifying upstream tools; replacing the template function alone would leave both paths
  unaddressed. Design written 2026-09-14 to
  `docs/superpowers/specs/2026-09-14-fresh-machine-secret-paths-design.md`, continuing the credential
  access boundary design. It recommends three changes that need no vault reorganization and no homelab
  dependency: the age identity becomes `dot_config/chezmoi/create_private_key.txt.tmpl` and
  `.chezmoiscripts/run_before_05-restore-age-key.sh.tmpl` is deleted, moshi pairing leaves the apply for
  a fresh-machine runbook step passing the token through `MOSHI_PAIRING_TOKEN` after a silent `read -rs`,
  and the `read-source-state` pre hook with `.install-password-manager.sh` and its unit test are removed
  because the KeePassXC cask already installs before the first vault read. Fresh-machine recovery is
  covered step by step for the local tier, including the iCloud-synced vault database as its own
  prerequisite, and for the brokered tier as re-issue rather than restore. It corrects the earlier
  document's `keepassxc.mode = "builtin"` one-liner: builtin mode keys entries by `<group>/<title>` and
  never visits the root group, so the switch is a 43-call-site rewrite across 16 files, three of them
  `modify_` templates, plus a vault reorganization; verified against chezmoi v2.72.1 source. Two exposure
  findings were added: a year-old 1.8 MB `keepass.yKLDVO` artifact sits beside the live database, and
  both `moshi-hook pair` and pns read the one entry `moshi-hook :: Device Token` whose two comments
  contradict each other, which makes rotation a coupled two-step. No code was written, no value was
  printed and no upstream tool was patched. Awaiting operator approval of the replacement paths. Full
  document: `docs/superpowers/specs/2026-09-14-fresh-machine-secret-paths-design.md`. Operator steps: (1)
  Read the design and approve or reject the three floor changes as written: the age identity as a
  `create_` target with `run_before_05-restore-age-key.sh.tmpl` deleted, moshi pairing moved out of the
  apply into the fresh-machine runbook, and removal of the `read-source-state` pre hook with
  `.install-password-manager.sh` and `test/unit/install-password-manager-hook.sh`. (2) Settle whether
  `moshi-hook :: Device Token` is one value with two roles: re-pair one host, then fire one test push and
  see whether it lands. The design assumes one value and writes rotation as a coupled two-step on that
  basis; the answer decides which of the two contradictory comments is a documentation fix and which is a
  correctness fix. (3) Decide whether to rotate the moshi device token now. It is measured as already
  present in roughly a dozen local atuin history rows. Rotate at the app first, re-pair, let the next
  apply carry the new value into `~/.config/pns/config.toml`, then delete the rows with
  `atuin search --delete "moshi-hook pair --token"`. (4) Identify `keepass.yKLDVO` in
  `~/Library/Mobile Documents/iCloud~com~strongbox/Documents/` (1834507 bytes, mode 0600, 2025-06-26) and
  either keep it deliberately or move it to `~/workspaces/backups/` under the dated naming convention. It
  was not opened by this work, and deleting a possible vault copy is an operator action. (5) Before any
  builtin-mode work, list the real entry paths with `keepassxc-cli ls -R -f <database>` (prints paths, no
  values) so the 43 call sites can be rewritten to `<group>/<title>`, and move any root-level entries
  into a group, since builtin mode never visits the root group. (6) Decide where the builtin switch sits
  in the queue relative to the three floor changes; the design recommends it be its own sequenced change
  and not combined with any of them. (7) Choose the pairing placement if you disagree with the
  recommendation: option P1 keeps pairing automatic with one line in the script and leaves the value in
  the rendered body, option P2 moves it to the runbook, option P3 keeps it automatic by reading the
  already-rendered pns configuration. Open questions: (1) Do you approve the three floor changes (age
  identity as a `create_` target with the script deleted, pairing out of the apply, `read-source-state`
  hook removed)? Each is independent of the homelab and of the builtin switch and small enough to be its
  own pull request. (2) Pairing in the runbook, or one line in the script? P2 is the recommendation; P1
  keeps a fresh machine at one command. (3) Rotate the moshi device token now? The history rows are
  measured, not hypothetical, and rotation touches the vault entry, the pns configuration on the next
  apply, and the pairing itself. (4) Is `moshi-hook :: Device Token` one credential or two? Both
  consumers read that one entry and both paths work today, so the likely answer is one value with two
  roles and the setup script's "separate entry" comment is stale. Confirm before rotating. (5) Where does
  the builtin switch sit in the queue? It closes the substituted-client path and needs a vault
  reorganization done by hand. (6) Which process is wrapped by Infisical's `connect`: the harness itself,
  so everything inherits proxy routing and certificate trust, or individual tools at their call sites?
  (7) Do you want the encrypted-target ordering invariant enforced by a check, or documented? Documented
  is the recommendation while all encrypted targets live under `private_dot_hermes/`. (8) What is
  `keepass.yKLDVO`, and does it stay? Keeping it deliberately is a fine answer; not knowing what a
  year-old 1.8 MB file beside the live vault is, is not. (9) Should the fresh-machine quickstart carry a
  copy-the-database-by-hand fallback? It removes an iCloud sync wait from the critical path at the cost
  of one more way to end up running on a stale vault copy.
- [ ] Demonstrate any proposed chezmoi flow with dummy credentials first, including locked/denied access,
  secret-free output and a complete render/deployment/manifest cycle. The current operator-only apply
  rule remains in force until a reviewed replacement is approved. `--exclude=templates` is a retired
  workaround, not the current agent apply procedure, and must not be revived.

### Homelab plan coordination

- [x] Keep the requested server deployments in `webdavis/homelab`: Infisical (A6), NetBird (F4), Dozzle
  (F5) and Open Notebook (L6) are recorded in `docs/plans/PLAN-v12.md` and its service matrix. Dotfiles
  owns their needed laptop configuration and managed client updates. Record actual cross-project
  dependencies without importing the full homelab deployment backlog into this modernization. Reconciled
  2026-09-14. Both sides now state the same split: PLAN-v12.md section 9 gives homelab Infisical's
  deployment, identities, policies, networking and recovery while dotfiles owns laptop clients,
  non-secret harness configuration and uu upgrades, and F4 repeats it for NetBird. The laptop half of all
  four was measured in this checkout and none of it exists yet: grepping for infisical, netbird, dozzle
  and Open Notebook finds only these coordination lines plus one 2026-05 research appendix quoting a
  nix-darwin directory listing, with no package declaration, no rendered configuration and no uu lane for
  any of them. A6 needs no new laptop machinery, because `brew info --json=v2 infisical` reports formula
  infisical 0.43.132 in Homebrew core, so the client is one entry in
  `.chezmoidata/system_packages_autoinstall.yaml` whose weekly upgrade rides the existing `[lanes.brew]`
  block of `dot_config/uu/private_config.toml.tmpl`; its non-secret configuration is `INFISICAL_DOMAIN`
  and `INFISICAL_AGENT_PROXY_ADDRESS` plus certificate trust for the private hostname, and the launch
  path is `infisical secrets agent-proxy connect -- <agent command>`, all three confirmed against the
  [upstream agent-proxy page](https://infisical.com/docs/cli/commands/agent-proxy). That work stays in
  the Infisical section above and in
  [task 6hPCF99XjRP87GmM](https://app.todoist.com/app/task/6hPCF99XjRP87GmM), not here. F4 costs more:
  `brew search netbird` returns only `netris`, and the
  [upstream macOS page](https://docs.netbird.io/get-started/install/macos) documents
  `brew install netbirdio/tap/netbird` with the `netbirdio/tap/netbird-ui` cask, so adoption adds that
  tap to both the `taps:` and `trusted_taps:` lists, and enrollment through
  `netbird up --setup-key <key> --management-url <url>` makes the setup key a vault-backed template value
  rather than a declaration. The Tailscale surface a cutover would keep or retire was inventoried so it
  is not rediscovered later: the `tailscale` formula, the `tailnet-pin` workspace built by
  `.chezmoiscripts/run_onchange_after_40-build-tailnet-pin.sh.tmpl` over the single `tailnet_pins` entry
  in `.chezmoidata/macos_system_setup.yaml`, the three osquery monitor files
  (`dot_local/libexec/osquery/executable_tailscale-monitor.sh`,
  `Library/LaunchAgents/com.webdavis.osquery-tailscale-monitor.plist.tmpl` and
  `.chezmoiscripts/run_onchange_after_60-load-osquery-tailscale-monitor-launchagent.sh.tmpl`),
  `.chezmoiscripts/run_onchange_after_66-tailscaled-status.sh.tmpl` with
  `test/unit/tailscaled-status.sh`, the `lulu_rule_tailscaled` verify control in
  `.chezmoidata/macos_posture_controls.yaml`, and the `tailscaled` repair key in the brew lane. F5 and L6
  own nothing on the laptop at all: `brew search dozzle` reports no formula or cask and
  `brew search open-notebook` matches nothing, because both are authenticated web interfaces reached over
  the tailnet, so their only laptop-side dependency is the private name resolution F4 may later move.
  L6's one real cross-project dependency is the optional vpp handoff, which stays in the item below and
  keeps canonical originals in the vault layout that already exists on disk
  (`agent-processing-pipeline/raw/`, `transcripts/` and `analysis/`). The boundary was written back as
  Todoist comments on the four homelab tasks and the dotfiles credential task, comment 6hW4Mc2w4Mf9PcrV
  on [A6](https://app.todoist.com/app/task/6hVpX4M8hmV4Gmr3), 6hW4MfWF2XGjmH63 on
  [F4](https://app.todoist.com/app/task/6hVpfWghCjQ66GG3), 6hW4MgVfXrQgpcXV on
  [F5](https://app.todoist.com/app/task/6hVpfWmPgm7qQHMV), 6hW4MhgQ5r44GVqV on
  [L6](https://app.todoist.com/app/task/6hVpfWrX8W4jrQcV) and 6hW4MmF39R6mpj5M on the laptop task. No
  homelab backlog item was imported, no task was completed because all four remain planning-stage behind
  F1 and F2, and no file in this repository was changed.
- [ ] Coordinate vpp's optional Open Notebook handoff with L6. Preserve one capture/transcription
  pipeline and canonical originals; decide the handoff format during integration design. vpp and Bob must
  not require Open Notebook merely to read or produce ordinary notes. Designed 2026-09-14 as
  `docs/superpowers/specs/2026-09-14-vpp-open-notebook-handoff-design.md`, the seventh document in the
  vpp chain, built on the boundaries design's four-homes table. The finding that decides it is in Open
  Notebook's own user guide, read today rather than remembered: "Audio/video is transcribed to text
  automatically", over MP3, WAV, M4A, OGG and FLAC, where M4A is what Apple Voice Memos writes. So
  handing it a recording starts a second transcription on a second engine with no flags, no alternatives
  and no `known-terms.txt`, which is exactly what L6's own bullet forbids and is one drag onto a web
  page. The recommendation makes the refusal structural instead of advisory: one pure command,
  `vpp handoff <id> [--stage transcript|analysis|brief|draft]`, prints one `vpp.handoff/1` document on
  standard output and does nothing else, and that document has no field that can hold audio or a path to
  it, so the supported path cannot produce the forbidden outcome. vpp performs no request, holds no
  credential and knows no address, which preserves the redacted-draft design's rule that vpp never
  transmits; the Open Notebook vocabulary lives in a mapping table in the document (`content`, `title`
  from vpp, `type: "text"`, `notebook_id` and `embed` from the pusher, everything else carried inside
  `content` because the source model has no metadata field) and in a four-line `jq` plus `curl` recipe
  owned by whoever runs it. That recipe was verified end to end against a throwaway local listener with
  an invented password: `curl -H @file` on the installed 8.22.0 delivered `Authorization: Bearer` with
  the password in no process argument list, the body arrived with `type: "text"`, and `shellcheck` passes
  clean; nothing left the machine, and no instance exists to send to since L6 is queued behind F1 and F2.
  Canonical originals hold because the emitted document is a pure function of the record and the note, so
  a lost notebook is regenerated by re-running one command, and because vpp has no importer and never
  reads anything back. The done-means is written as four absence checks (service absent, configuration
  absent, a grep over vpp's tree for `open.notebook`, `notebook_id`, `5055`, `8502` and `surreal` finding
  nothing, and `vpp.brief/1` unchanged for Bob), the third of which is the cheapest guard against a
  convenience push creeping in later. The header inside `content` carries the identity, the
  unresolved-flag count and a derived-copy sentence, keeps `[unverified]` markers in place because a
  model summarizing a source drops a footer and keeps the sentence, drops the note's frontmatter
  (`vppRecording` is a to-the-second capture timestamp wearing an identifier's clothes), and is
  deliberately declarative, since a source that instructs a model is indistinguishable from an injected
  one in a notebook that also holds web pages. Two upstream configuration findings are handed to L6
  rather than solved here, both in upstream's own words: one shared password sent in plain text with no
  rate limiting or audit log, so encrypted transport is mandatory, and an unrestricted `CORS_ORIGINS`
  means "any website the user visits can issue authenticated cross-origin requests to your API". Named
  ceiling: vpp never learns the remote source identifier, so a corrected note handed off twice makes two
  sources rather than replacing one; the upgrade path is written in the source instead of built. Waiting
  on the operator: whether a private note may cross or only a released redacted draft, whether the
  no-push rule survives costing three lines at every handoff, which artifacts may be handed off at all
  (the brief names the people who will be in a room), who holds the shared password when L6 exists and
  whether an agent gets write access through the `uvx open-notebook-mcp` server, who owns the return path
  for a note authored in Open Notebook, and where the transport recipe eventually lives. No code written.
  Full document: `docs/superpowers/specs/2026-09-14-vpp-open-notebook-handoff-design.md`. Operator steps:
  (1) Answer the private-versus-redacted question, because it decides what the feature is for: the
  proposal lets a private note cross with the document recording that it did, and the alternative is that
  only a released redacted draft may cross, which makes the notebook safe and much less useful. (2)
  Confirm the no-push rule: three lines of jq and curl at every handoff, forever, versus one
  `vpp handoff --push`. If three lines are too heavy for how this will actually be used, say so before
  the rule is written into tests rather than after a convenience flag is added around it. (3) Decide who
  holds Open Notebook's shared password when L6 exists: the laptop through KeePassXC, an agent through
  `uvx open-notebook-mcp` (uvx is already installed), or nobody, with the handoff done by hand in the web
  interface. This is the smallest concrete instance of the A6 credential question already in the ledger.
  (4) Hand L6 the two upstream configuration findings, which are not vpp's to fix: encrypted transport is
  mandatory because the shared password is sent in plain text with no rate limiting or audit log, and
  `CORS_ORIGINS` must be restricted or any site the operator's browser visits can reach the interface
  with the operator's session. (5) Decide who owns the return path for a note authored in Open Notebook
  that should become durable. L6's bullet says canonical locations including Obsidian; this design says
  vpp has no importer, which leaves the act unassigned until it is assigned deliberately. (6) This ledger
  bullet's own line number is stale in the earlier triage: the bullet is at docs/remaining-work.md under
  "### Homelab plan coordination". Nothing needs an apply, a package, a grant or a deployment for this
  item. Open questions: (1) May a private note cross into Open Notebook, or only a released redacted
  draft? The design supports both and defaults to allowing private notes, on the reasoning that a
  redacted research workspace answers redacted questions; the counter-argument is that once a hosted
  model provider is configured, a private transcript in a notebook has left the machine as surely as an
  email would. (2) Is the no-push rule right? It is the load-bearing assumption, it costs three lines at
  every handoff, and it is the reason the share gate in the previous document still means something. (3)
  Which artifacts may be handed off at all: transcripts, analysis notes, briefs, released drafts, or a
  subset? The brief is the same caution the share design raised, since it names the people who will be in
  a room. (4) Should an unreviewed artifact be refusable rather than labelled? Today it is emitted with
  the flag count and the in-line markers; a refusal would be stricter and would probably be routed around
  by hand. (5) Who holds the shared password, and does an agent get write access to the notebook? The
  `uvx open-notebook-mcp` server makes agent-driven filing available today with nothing new to install.
  (6) Does the handoff need to say it came from vpp? The proposed header names vpp and the record
  identity, which is good provenance and also tells anyone with notebook access that a recording exists.
  (7) What happens to a copy in the notebook when the note it came from is corrected? Today nothing: a
  second handoff makes a second source, because vpp never learns the remote identifier. The upgrade path
  is named; whether it is needed depends on how often a transcript is corrected after filing. (8) Where
  does the transport recipe eventually live? Nowhere today. A dotfiles `libexec` script, a `just` recipe,
  a homelab-side ingester and "the operator types it" are four answers with four different maintenance
  costs, and a scheduled one would need a separate argument against "no second automatic workflow". (9)
  Where does vpp's code live, and what is it called? Carried forward unresolved from the boundaries
  design, because the chain should not stay in disagreement with itself.

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
  for Apple Voice Memos synced to macOS. Reconciled 2026-09-14 in
  `docs/research/2026-09-vpp-source-reconciliation.md`. The three named sources reconcile and the claim
  holds: L-R5 is the vpp specification itself (its `pns submit --json` contract reverified against
  `pns-protocol` source, `producer` and `signal.kind: needs_attention` both correct, though `submit` is
  missing from `pns --help`); the vault's `agent-processing-pipeline/` tree is a filing convention that
  has never run (all three directories empty since 2026-07-22); and `PLAN-v11` Phase 6 is a homelab
  hermes skill that transcribes URLs on `lash` through n8n, never a local-recording watcher, contributing
  only its engine decision (ElevenLabs Scribe v2 with whisply fallback) and its three output formats.
  None specifies a Voice Memos watcher. The reconciliation also found a FOURTH source the task does not
  name: the `minutes` 0.26.1 cask, declared and installed 2026-09-08, whose 60 subcommands already cover
  six of the seven vpp feature bullets (folder watcher plus launchd service, first-class `memo` type,
  `transcribe --json --diarize`, vault sync by symlink, voiceprints, retention policy, templates,
  insights, commitments, draft-only delivery). Only three vpp items are absent from all four sources:
  Voice Memos discovery, redundant transcription with disagreement comparison, and the pns
  needs-attention notification. **Verdict: defer the vpp ingestion design.** vpp's scope is blocked on
  the still-open `minutes` disposition (task `6hPV483GJgGHX95M`), which decides between an adapter around
  `minutes` and a full replacement; `PLAN-v12` L6 already forbids a second automatic Voice Memos workflow
  for Open Notebook and nobody has applied that rule here. Measured inputs for the next task, not
  decisions: recordings are ALAC 48 kHz stereo in `.m4a` (not AAC), about 1 GB for 28 files with single
  files at 188 MB; titles, durations and stable identifiers live only in `CloudRecordings.db`, a Core
  Data store whose write-ahead log must be copied with it or a reader sees a nine-day-stale snapshot;
  `ZEVICTIONDATE` does not mean the audio is gone (both evicted rows still have local files); and
  `minutes storage` already classes 30-day-old originals as delete-candidates, which collides with vpp's
  preserve-originals rule. Separate live drift: the vault `CLAUDE.md` claims the
  `agent-processing-pipeline/minutes` symlink is managed by `minutes vault setup --subdir`, but no
  `minutes` config file exists and `minutes vault status` reports `Vault: not configured`, so the
  committed link is an orphan. Full document: `docs/research/2026-09-vpp-source-reconciliation.md`.
  Operator steps: (1) Read docs/research/2026-09-vpp-source-reconciliation.md. It is already
  mdformat-clean against the repo .mdformat.toml, so no reformat is needed. (2) Rule on the `minutes`
  disposition: keep it, or replace it. This is the blocking decision. Everything about vpp's scope
  follows from it, and it is still open in Todoist task 6hPV483GJgGHX95M. Do not let the next vpp ledger
  task (the ingestion design) start before this ruling lands. (3) Rule on whether PLAN-v12 L6's existing
  constraint ("must not create a second automatic Voice Memos capture/transcription workflow", written
  for Open Notebook) binds `minutes` against vpp. If it is a general rule rather than a per-service one,
  it settles the adapter-versus-replacement question on its own. (4) Decide the redundant engine pair and
  whether Voice Memos audio may leave the machine. All the candidates are already installed here:
  ElevenLabs CLI 1.2.0 (fnm node 24), whisply 0.14.2 (uv), openai-whisper (Homebrew), plus the `minutes`
  on-device pipeline. PLAN-v11 Phase 6's cloud-first Scribe v2 choice carries a metered cost line; a
  local-only rule for personal voice memos would retire both that line and one candidate engine. (5)
  Repair or explicitly defer the vault `minutes` link. The repair is one operator command,
  `minutes vault setup --strategy symlink --subdir agent-processing-pipeline/minutes`, which also
  recreates the missing ~/.config/minutes/config.toml. Note the default `--subdir` is `areas/meetings`,
  so omitting the flag would create a second meetings location in the vault. Either way the vault
  CLAUDE.md sentence claiming the link is managed needs correcting, because it is false as written today.
  (6) Decide whether to upgrade `minutes` 0.26.1 to 0.26.2 before ruling on its disposition, since its
  feature set is the input to that ruling. It is a declared cask, so the weekly uu Homebrew lane would
  take it; an unattended upgrade before the decision is fine, but the decision should be made against
  whatever version is then installed. (7) Optional pns follow-up, independent of vpp: `pns submit` works
  (pns/crates/pns/src/invocation.rs:119) but is absent from `pns --help`. Whoever implements a producer
  against it will not find it from the CLI. Open questions: (1) Is `minutes` kept or replaced? This is
  the blocking question; vpp's whole scope follows from it, and the ledger has carried it as an open
  evaluation since before vpp existed. (2) If `minutes` is kept, does vpp call it or run beside it?
  Calling `minutes transcribe --json` makes it one of vpp's two engines and reuses its summarization,
  vault sync and speaker work. Running beside it means two tools writing notes about recordings, which is
  what PLAN-v12 L6 forbids for Open Notebook. (3) Does the PLAN-v12 L6 rule ("must not create a second
  automatic Voice Memos capture/transcription workflow") bind `minutes` against vpp, or was it only ever
  about Open Notebook? (4) Which two engines are the redundant pair? Phase 6 already chose ElevenLabs
  Scribe v2 with whisply on faster-whisper as the fallback, and Scribe, whisply, openai-whisper and the
  minutes on-device pipeline are all present here. Cloud-plus-local and two-local differ in cost, in what
  audio leaves the machine, and in whether a disagreement means anything. (5) Do Voice Memos originals
  leave the machine at all? L-R5 inherits Phase 6's "local processing for sensitive audio", but everyday
  personal voice memos may all qualify, which would remove the metered cost line and one candidate engine
  together. (6) Repair the vault `minutes` symlink now or fold it into the vpp work? Folding it in leaves
  the vault CLAUDE.md claim false until vpp ships. (7) Should the four unwired transcription installs
  stay declared? whisply, openai-whisper, @elevenlabs/cli and the minutes cask are all in
  .chezmoidata/system_packages_autoinstall.yaml and no script, recipe or LaunchAgent reaches any of them.
  They are either vpp's future inputs or removable weight, and which depends on the engine decision. (8)
  Does `minutes` get upgraded to 0.26.2 before the disposition decision, given that its feature set is
  the input to that decision? (9) Deferred to the next ledger task, not answered here: whether a
  launchd-run service can read ~/Library/Group Containers/group.com.apple.VoiceMemos.shared at all under
  its own macOS privacy-permission identity (every read in this record ran from a terminal that already
  holds broad disk access), and whether reading Apple's private CloudRecordings.db Core Data schema is
  acceptable at all. A no to either changes what vpp is, from a watcher to an export-path integration.
- [ ] Design automatic discovery of fully synced recordings, preserving original audio and capture
  metadata without modifying Apple's source recordings. Verify the supported macOS access/export path and
  actual audio format before choosing an ingestion mechanism. Handle interrupted sync, retries and
  repeated discovery without duplicate notes or lost audio. Keep original recordings, transcripts and
  agent analysis separately linked using the existing vault layout. Designed 2026-09-14 in
  `docs/superpowers/specs/2026-09-14-vpp-recording-discovery-design.md`, written against the second
  bullet only and scoped so the still-open `minutes` disposition changes the consumer rather than this
  producer. The access-path verification is the finding: Voice Memos ships no scripting dictionary, its
  App Intents expose only title, creation date and duration with no action that outputs a file, and
  Spotlight holds no content metadata, so no supported programmatic path yields the audio and the real
  choice is a read-only read of the undocumented group container or a human export through the share
  sheet. The format is Apple Lossless Audio Codec at 48000 Hz in an `.m4a` container, 988 MB across 28
  recordings. The capture timestamp was measured inside the audio file, matching the database to the
  second, so the private schema is needed only for the human title and its loss degrades to untitled
  rather than broken. Recommended: an idempotent `vpp ingest` sweep on a `StartCalendarInterval` rather
  than a watcher or a daemon; a wholeness gate of MPEG-4 top-level box lengths summing to file size with
  `moov` present, plus an mtime quiet period; content-derived identity; and `clonefile(2)` into
  `agent-processing-pipeline/raw/audio/`, whose `EEXIST` is the duplicate guard and whose copy-on-write
  clone preserved a 187.9 MB recording in 0.00 s for 16 KB. Not approved and not built: it carries twelve
  assumptions and eight open questions, and one measurement is unresolved, whether a LaunchAgent that
  launchd starts at login can read the group container, since every read in the session inherited
  Ghostty's Full Disk Access grant. Full document:
  `docs/superpowers/specs/2026-09-14-vpp-recording-discovery-design.md`. Operator steps: (1) Read the
  design and answer open question 1 first: is reading Apple's undocumented Voice Memos group container
  acceptable at all? The measurements closed every supported programmatic path, so a "no" turns vpp from
  a watcher into a manual filing tool and the design needs replacing rather than editing. (2) Settle the
  LaunchAgent permission question: install a plist whose program reads one byte of ~/Library/Application
  Support/com.apple.TCC/TCC.db and one byte of a recording, load it, LOG OUT AND BACK IN so launchd
  starts it with no submitting session, then read the result file. The logout is load-bearing: a job
  submitted from a terminal may inherit that terminal's responsible-process attribution, which is what
  the test exists to rule out. (3) Confirm or reject the clone decision (assumption 5): it puts a
  copy-on-write clone of every personal recording inside
  ~/workspaces/Ivy/agent-processing-pipeline/raw/audio/, gitignored and invisible to Obsidian mobile
  sync, at measured zero storage cost. (4) Remove the scratch copy of the personal Voice Memos database
  this session made, which is a destructive action needing your confirmation: trash
  /private/tmp/claude-501/-Users-stephen-workspaces-Ivy-webdavis-dotfiles/1bf0ef19-e242-4ea5-8746-60cb679ebafc/scratchpad/vm-probe
  (5) Carried forward from the reconciliation, unrelated to this design: repair or retire the orphaned
  vault minutes link with
  `minutes vault setup --strategy symlink --subdir agent-processing-pipeline/minutes` (the default
  --subdir lands in areas/meetings and creates a second meetings location), and correct the vault
  CLAUDE.md claim that the link is already managed. Open questions: (1) Is reading Apple's undocumented
  Voice Memos store acceptable at all? This is the gating question: no supported programmatic path yields
  the audio, so a "no" makes vpp a manual filing tool driven by the share sheet. (2) Can a LaunchAgent
  that launchd starts at login read the group container? Unresolved. Every read in this session inherited
  Ghostty's Full Disk Access grant, a launchctl-submitted job may inherit the same attribution, and
  launchctl procinfo needs root. (3) Does the minutes ruling change this boundary? The design assumes
  discovery is a producer and transcription a consumer. If vpp is meant to be a thin front end on
  `minutes watch`, the clone destination changes and most of the design is replaced by configuring a
  third-party tool. (4) Clones, or references in place? The design chose clonefile(2) on measured cost,
  which puts a second copy of every personal recording inside the vault directory, gitignored but
  present. (5) Fifteen minutes as the sweep interval, or something else? And is near-immediate discovery
  worth taking WatchPaths despite launchd.plist(5) discouraging it in its own words? (6) What happens to
  a recording deleted in Voice Memos after vpp has cloned it? The clone survives, which is the point of
  cloning. Should vpp notice the disappearance and mark the sidecar, or keep the clone silently? (7) Do
  the eight .waveform sidecars matter? They belong to 2022-era recordings only and are Apple's rendering
  cache; the design ignores them. (8) Should `vpp ingest` emit its per-recording record on stdout as JSON
  for a caller to pipe, or only write sidecars? The design does both on the assumption the next stage
  wants a stream; if nothing will consume it, that is unneeded surface.
- [ ] Use redundant transcription and compare disagreements; flag uncertain text and unsupported notes
  for review, notifying through pns's producer application programming interface (API). Preserve the
  alternatives and source references. Multiple engines agreeing does not prove correctness. The proposed
  feature for playing audio from a summary sentence was rejected; original audio preservation remains.
  Designed 2026-09-14 as `docs/superpowers/specs/2026-09-14-vpp-redundant-transcription-design.md`. Three
  engine runs against a synthesized clip with known ground truth settled the shape. whisply on Apple's
  MLX framework and `openai-whisper` with `turbo` produced normalized transcripts that were identical,
  because they are the same `large-v3-turbo` weights on two runtimes, so that pairing is not redundancy
  at all; the design makes a same-model-family pair a startup refusal. Word confidence proved a weak
  signal even on the strong model, ranking correct common words below actual errors, and the one error
  every engine shared ("Muthakrishnan" for Muthukrishnan) was invisible to both disagreement and
  confidence. So the recommendation is three signals rather than two: disagreement between different
  model families, low confidence where an engine reports any, and a risk-class flag on agreed names,
  numbers and dates, aggregated by surface form and suppressed by a `known-terms.txt` the operator grows.
  Note checking is a separate `vpp verify-note` over a timecode source-reference convention, with four
  flag classes, the strongest being a claim built on already-flagged text; it annotates and never gates.
  Notification is one `pns submit --json` per recording carrying identity, counts and paths and never any
  flagged text, on a route the hermes gateway actually declares, with an aggregate form so a 28-recording
  backlog does not fire 28 pages. Open Question 8 is now priced rather than open-ended: the whole
  existing back catalogue is 4.607 hours, $1.01 once through ElevenLabs Scribe v2 at $0.22 per hour, and
  about $0.50 a month at the hypothesized rate, against roughly 5 minutes of laptop compute per 10-minute
  recording for one local engine and 15 for the tempting but non-redundant local pair. Apple's
  SpeechAnalyzer is confirmed available on this Mac and is the free different-family option, at the cost
  of a Swift helper in a Rust project. `minutes transcribe` cannot run here at all today: its model file
  is absent. `audit_engine` ships empty so nothing costs money or leaves the machine until the operator
  names a second engine. Waiting on the operator: the engine pairing, whether transcripts may be
  committed to the vault and synced to a phone, and whether a confirmed correction may rewrite later
  transcripts. No code written. Full document:
  `docs/superpowers/specs/2026-09-14-vpp-redundant-transcription-design.md`. Operator steps: (1) Answer
  Open Question 8 now that it is priced. The four pairings and their measured costs per 10-minute
  recording: whisply on MLX with large-v3-turbo alone, about 5 minutes of laptop compute, no auditor, no
  confidence; whisply MLX plus openai-whisper base on the central processing unit, about 15 minutes of
  compute and NOT a different model family, which is the trap; whisply MLX plus ElevenLabs Scribe v2,
  about 5 minutes plus $0.037, different families, confidence on both sides; whisply MLX plus Apple
  SpeechAnalyzer, about 5 minutes plus on-device time, different families, confidence unknown. The whole
  existing 28-recording back catalogue through Scribe v2 is $1.01 once, and the hypothesized three
  recordings a week is about $0.50 a month. (2) Decide whether `minutes transcribe` is a candidate
  adapter, and if so run its setup. It fails today in 0.04 s with "Transcription model not found.
  Expected model file "ggml-small.bin" in /Users/stephen/.minutes/models". The fix is
  `minutes setup --model small`, which is a download and therefore an operator step. Note it is a local
  Whisper, so it is the same model family as whisply and cannot be that engine's auditor. (3) Add a `vpp`
  route to the hermes gateway, or accept that vpp posts on the existing `pns` route. The gateway declares
  exactly `priority`, `pns` and `unattended-upgrades`; posture names a `posture` route that does not
  exist and eight of its Discord legs are dead-lettered with HTTP 404 right now. Settle this before vpp
  sends its first notification rather than after. Open questions: (1) Which engine pairing, from the
  priced table? This is Open Question 8 and everything else in the design is a configuration value once
  it is answered. (2) Is the Apple SpeechAnalyzer route worth a Swift helper inside a Rust project? It is
  the only free different-family option on this Mac and it is confirmed available here (macOS 26.2,
  SpeechTranscriber asset supported, en_US installed), but its confidence reporting is unknown and it
  would put a second language in the build. (3) May a transcript be committed to the vault, and therefore
  synced to a phone? The audio is gitignored and the transcript would not be. The design assumes yes
  because that is what the vault's `transcripts/` directory is for, but it is the decision that puts
  searchable text of every voice memo into a git history. (4) Should a confirmed correction rewrite
  future transcripts? Recording that "Muthakrishnan" should be "Muthukrishnan" is cheap; applying it
  automatically changes the transcript of record with no human reading the result. The design records and
  does not apply. (5) How loud should the `agreed-unverified` class be? It is the class that catches the
  error every engine shared and also the largest class. The design ranks it last and aggregates it by
  surface form. Should it appear in the pns notification at all, or only in the file? (6) One
  notification per recording with aggregation past three, or one summary per run always? A weekly
  reviewer might prefer the latter. (7) Does `verify-note` belong in vpp or in whatever writes the note?
  It is a separate command precisely because the writer is undecided. If the writer turns out to be
  `minutes`, the check still works but would be checking a third-party tool's output against a timecode
  convention that tool does not follow, which needs the convention enforced somewhere else. (8) Where
  does vpp's code live? The sibling boundaries design recommends its own repository; the sibling
  discovery design assumed a fifth cargo workspace in dotfiles. Nothing in this document depends on the
  answer, but the two designs should not stay in disagreement.
- [ ] Support agent-suggested tags and relationships between recordings, with a defined metadata schema
  and configurable output paths. Use explicit links and deterministic filing rules for automatic routing.
  Markdown output can live in an Obsidian vault and use its existing mobile sync, but Obsidian is
  optional. Designed 2026-09-14 as
  `docs/superpowers/specs/2026-09-14-vpp-metadata-schema-and-filing-design.md`. Three schema homes were
  compared (adopt the vault's own eight-key note schema, adopt `minutes`' published frontmatter JSON
  Schema, or keep a versioned record in vpp's state and render the note from it) and the third is
  recommended, with `minutes`' vocabulary borrowed where it already named a concept. Measured in the
  vault while writing: 753 Markdown files, 738 with frontmatter, 722 note names and 246 aliases,
  scannable in 0.245 s, so the link and vocabulary index is rebuilt every run rather than cached; 122
  distinct tags over 1,109 uses, serialized three different ways and named three different ways (23
  camelCase, 10 kebab, the rest single words), which is why a new tag has no derivable shape and is held
  rather than written; `status`, `hub` and `description` appear in none of the 87 entries of
  `.obsidian/types.json`, so vpp's own keys need no registry edit. Obsidian's documentation settles the
  tag character set, the two link formats and the URL encoding rule, and says a nested property is
  source-mode only, which is why every vpp key is flat and prefixed and why `minutes`' nested `entities`
  was not copied. `yq` confirms an unquoted `[[Note Name]]` in YAML parses as a nested sequence, so wiki
  links in frontmatter are always quoted. Filing is defined as five testable properties (total, pure,
  stable, explainable, collision-free), keyed only on confirmed metadata, pinned at first write, and
  exposed as `vpp path <id> --stage <stage>` so the later note generator files correctly without
  embedding the rules; names are `{date}-{slug}-{hash8}` with the slug sanitized as a trust boundary.
  Links live in a marked managed block that vpp rewrites and never touches prose outside, and a note the
  operator renames in Obsidian is found again by its `vppRecording` key. Retention is a report and not a
  reaper: the whole back catalogue's transcripts are about 187 KB and both engines' raw outputs about 13
  MB, while the audio is nearly free until Apple deletes the original, so the real question is whether
  vpp's copy is the backup. Also corrected while measuring: `relationship_map` is a `minutes` capability
  flag for feature detection, not a subcommand, and the command-line surface is `minutes people`. Waiting
  on the operator: the output layout, the retention answer, whether new tags are held or accepted, and
  the shape of a new tag. No code written. Full document:
  `docs/superpowers/specs/2026-09-14-vpp-metadata-schema-and-filing-design.md`. Operator steps: (1)
  Answer the output-layout half of Open Question 8: audio cloned into the vault's raw/audio/ (the
  discovery design), an archive directory outside the vault with an optional symlink (the boundaries
  design, matching what minutes already does with ~/meetings), or a folder per recording (not
  recommended, it costs a folder note per recording under the vault's folder-note rule). (2) Answer the
  retention half, which is smaller than it looks: transcripts are about 187 KB for the whole back
  catalogue and about 1.5 MB a year, both engines' raw outputs together about 13 MB, and the audio is
  983.6 MiB logically but nearly free physically until Apple's original is deleted. The real decision is
  whether vpp's copy is the backup, and no machine backup exists yet. (3) If the vault layout is adopted,
  write the three missing folder notes for agent-processing-pipeline/, transcripts/ and analysis/
  carrying the reference DataviewJS query from the vault's CLAUDE.md. None exists today, so nothing vpp
  writes would appear in any listing. (4) Decide the shape of a tag that does not exist yet: kebab-case
  (the document's default) or camelCase. The vault corpus splits 10 to 23 the other way, so there is no
  majority to derive it from. One configuration value either way. Open questions: (1) Which output
  layout, and is vpp's audio copy the backup? Everything else in the design is a configuration value once
  that is answered. (2) New tags: held as suggestions (the design's default, because the vault's 122-tag
  vocabulary is small and deliberate and git makes a mistake permanent) or accepted automatically (which
  removes a confirmation step per recording and grows the vocabulary faster than any human would)? (3)
  kebab-case or camelCase for a newly invented tag? The corpus has 10 kebab and 23 camelCase tags, so no
  majority exists. (4) May vpp write into notes it did not create? The mentions relation links out to
  existing contact and project notes; the design writes that link only on the transcript's side and adds
  nothing to the target, and the alternative is vpp editing the operator's own writing. (5) Which mobile
  sync actually carries the vault? Obsidian's core Sync plugin is enabled and obsidian-git is configured
  to push every 15 minutes; they send the same transcripts to different third parties, which the
  transcription design's open question about committing transcripts cannot really be answered without.
  (6) Should `vpp path` stay the contract for the later note generator, or should vpp write the analysis
  note itself? (7) Does minutes stay? If it does, its notes are input that vpp files, and the two schemas
  sit side by side in one vault with no key in common; adopting its schema was rejected for reasons that
  would need revisiting if minutes becomes the note generator rather than a candidate. (8) Where does
  vpp's code live, and what is it called? Carried forward unresolved from the boundaries design so the
  chain does not stay in disagreement with itself.
- [ ] Plan meeting briefs using relevant notes, with optional read-only calendar and Todoist inputs.
  Record Bob, the future Hermes executive assistant, as a consumer of vpp's notes and briefs. The exact
  trigger, scheduling owner, access scopes and provider choices remain under discussion. Keep source
  references and unresolved transcription issues visible to Bob and in the brief. Designed 2026-09-14 as
  `docs/superpowers/specs/2026-09-14-vpp-meeting-briefs-design.md`, the fifth document in the vpp chain,
  built on the metadata and filing design. The permission verification this bullet asks for is the
  finding, and the three providers disagree: the Google Calendar interface defines eight read-only scopes
  and Todoist defines `data:read`, but neither offers a per-calendar or per-project scope, and Apple's
  EventKit has had no read-only level for events since macOS 14, only full access or write-only. So
  "read-only calendar" is achievable through exactly one provider, the already-installed `gog` v0.40.0,
  whose `--readonly`, `--enable-commands-exact` and `--wrap-untrusted` flags assert it at the grant, at
  the call and on fetched text; the Mac's own aggregated calendar cannot be read read-only at all, and a
  local-file read would also have to reimplement the recurrence expansion the interface already performs.
  Because no provider can scope a read to named calendars or projects, the configured list is the only
  scope that exists, so an empty list is a startup refusal rather than "everything" and the selection
  goes in the request rather than a filter afterwards. Measured on this machine: 16 calendars, 4,247
  items, 8 items in the last 90 days carrying any participant, and not one non-recurring forward item
  timed, so a scheduled brief would wake and find nothing; the recommendation is requested briefs only,
  with the launchd seam named and left unbuilt (the pns daemon schedules pns events, not commands, so it
  can remind but cannot produce one). vpp assembles and cites rather than writing prose, and prose
  arrives as a proposal checked by the existing `vpp verify-note`. Calendar and Todoist context arrives
  through one `vpp.context/1` document that either vpp's own disabled-by-default collectors or Bob can
  produce, so the question of whether vpp reads the two services or takes them from Bob sets a
  configuration value instead of gating implementation. Bob is recorded as a consumer through
  `vpp.brief/1`: every item carries `certainty` (two values, never a score) and `sources` (recording
  identity, offset, note) with no default, and uncertainty is marked in place inside the line as well as
  counted in a section, because a consumer that quotes one bullet drops a footer and keeps the sentence.
  Selection is four exact selectors over confirmed terms and confirmed tags, capped at 12 and
  explainable; the obvious participant join is nearly empty here, 78 of the 103 notes carrying an
  `email:` key hold `email: []`, so an unmatched participant is a visible line with its confirm command
  rather than a silent omission. A brief is a fourth stage in the existing filing rule table, routed by
  `vpp path`, so there is no second copy of the rules. Waiting on the operator: who holds the two
  credentials, requested against scheduled, which calendars and projects, the Todoist single-slot problem
  (`td` stores one credential per account and it is read-write today, so a read-only login would probably
  replace it), whether a brief may be auto-committed into the vault when it names the people who will be
  in the room, and whether "brief" collides with Forzare's own morning brief. No code written. Full
  document: `docs/superpowers/specs/2026-09-14-vpp-meeting-briefs-design.md`. Operator steps: (1) Decide
  who holds the calendar and Todoist credentials: vpp, or Bob. Either answer works without changing vpp,
  and the shipped default (brief.context.source = "none") is the answer to "neither, yet". If it is Bob,
  this bullet's context half waits for Forzare and vpp still ships useful, which is what L-R5 requires.
  (2) Decide requested against scheduled briefs, and say whether meetings are going to start appearing on
  a calendar. The measurement (not one non-recurring forward item on this machine is timed; 8
  participant-bearing items in the last 90 days) says a scheduler would find nothing today, but that is a
  measurement of the past, not of the intent. (3) Name the calendars and the Todoist projects a brief may
  read. No provider can enforce this, so the configured list is the only scope that exists; an empty list
  refuses by design, so this answer is required before either collector can be turned on at all. (4) If
  vpp is to read the calendar, authorize a read-only Google credential with a command of the form
  `gog auth add <email> --services=calendar --readonly`, and confirm it does not disturb the existing
  one. `gog auth services` shows the calendar service's default scope is the full
  `https://www.googleapis.com/auth/calendar`, so this is a distinct authorization; whether `--client`
  lets a second read-only token bucket sit beside the existing credential was NOT verified, because
  `gog auth list` did not return inside a 20 second timeout. (5) If vpp is to read Todoist, settle the
  single-slot problem. `td accounts list` shows one stored account keyed by numeric id and
  `td auth status` reports it read-write, so `td auth login --read-only` would re-authorize that same
  account and most likely replace the operator's daily credential. Three ways out, in increasing cost: a
  second Todoist account sharing the relevant projects; a dedicated read-only token in KeePassXC used
  directly, making a sixteenth secret-bearing target; or Bob supplying the task context. (6) When the
  vault layout is adopted, write a folder note for `briefs/` carrying the reference DataviewJS query,
  making four in total alongside the three the metadata design already named. vpp writes no folder notes.
  Open questions: (1) Does vpp read the calendar and Todoist, or does Bob supply them? The design makes
  both work through one input document, so this sets a configuration value, but it decides who holds two
  credentials and therefore what an agent with shell access can reach. (2) Requested or scheduled, and at
  what lead time? The recommendation is requested only, on the measurement that this machine's forward
  calendar holds no timed non-recurring events at all. The question behind it is whether that is the
  intended future or the current gap. (3) Which calendars and which Todoist projects? Required before any
  collector can run. And the sharper version: given that a read-only token reads every calendar and every
  project regardless, is a client-enforced selection an acceptable boundary, or does that make the
  context feature not worth its access? (4) Can a read-only Todoist credential coexist with the
  operator's read-write one? If not, which of the three ways out is acceptable. This is the one place
  where the design might have to add a sixteenth KeePassXC-backed target. (5) May a brief be written into
  the vault, given that Obsidian Git auto-commits and pushes within minutes? A brief names the people who
  will be in a room, which is a different exposure from a transcript naming whoever was mentioned, and it
  may mean briefs live outside the vault even when transcripts live inside. (6) Is "brief" the right
  word, given that Forzare already has a morning brief? Bob's is a day plan delivered on a schedule; this
  is a per-occasion evidence pack produced on request. Two things called a brief, one composed by Bob and
  one consumed by Bob, is a name collision waiting to confuse a future session. (7) If `minutes` stays,
  should its `research` and `person` output be a context source for a brief? It is the one installed tool
  that already ranks material about a person or topic, over its own corpus. This is downstream of the
  keep-or-replace ruling and does not need answering before it. (8) Is the local Apple Calendar store
  representative of the operator's calendars? The measurement came from that store (16 calendars), and
  recommending the Google interface assumes the meetings that matter are in Google. A calendar existing
  only in Calendar.app would be invisible to this design. (9) Where does vpp's code live, and what is it
  called? Carried forward unresolved from the boundaries design, because the chain should not stay in
  disagreement with itself.
- [ ] Support a separate redacted draft for sharing, reviewed before release, preserving private
  originals. Choose the summary format, retention, transcription engines and local/cloud processing
  before implementation. Speaker labels and dated digests remain unapproved candidates. 2026-09-14:
  design written at `docs/superpowers/specs/2026-09-14-vpp-redacted-sharing-design.md` (sixth in the vpp
  chain, building on the meeting-briefs design). Recommendation: four verbs
  (`vpp share draft|review|approve|release`), with the draft assembled by allowlist into
  `~/.local/state/vpp/share/<draft-id>/` at mode 0700, outside the vault and outside any git working
  tree, because the vault's Obsidian Git settings were measured at a 10-minute commit and a 15-minute
  push, so a draft built in the vault is published before anyone reviews it. Release refuses without an
  approval record whose SHA-256 digest matches the current draft bytes and policy, re-runs the residue
  scan rather than trusting the draft-time pass, and refuses a destination inside a git working tree, a
  cloud-sync root, the output root, the audio destination or vpp's state tree; approval refuses when
  standard input is not an interactive terminal and requires the operator to type the draft's short
  digest, measured against an agent shell on this machine that has no terminal and sets `CLAUDECODE=1`.
  Redaction is deterministic over confirmed `known-terms.txt` entries plus six closed pattern classes,
  with flagged spans omitted by default and a capitalized-token candidate report for what no pattern can
  find: measured on one real 639-word vault note, 33 unique capitalized tokens of which 13 were personal
  names, which is why the heuristic is a review prompt and never an automatic mask. vpp transmits
  nothing; release writes a file. Speaker labels, dated digests, sending, watermarking, model-based
  detection and audio redaction are out of scope. Not approved and not built; 14 assumptions and 9 open
  questions are recorded, and the ledger's own four choices (summary format, retention, engines, local or
  cloud processing) gate implementation. Full document:
  `docs/superpowers/specs/2026-09-14-vpp-redacted-sharing-design.md`. Operator steps: (1) Answer the four
  choices this ledger bullet names, as four separate decisions rather than one: summary format (extract
  or generated prose, noting that for prose the human read is the only real protection), retention of
  drafts and released copies (a draft directory holds the placeholder-to-real-value map, so it is more
  sensitive than the transcript), transcription engines, and local or cloud processing. The last one
  matters here for a reason the transcription design did not raise: a recording transcribed by a cloud
  engine already left the machine once, before any redaction existed, and this gate only protects the
  second egress. (2) Choose the release directory and confirm it is not synced anywhere. The proposed
  default is ~/Documents/vpp-shared; measured, ~/Documents on this machine is local and not redirected
  into iCloud, but an iCloud Drive container is active (brctl reports 98 containers, CloudDocs last
  synced 2026-09-08). (3) Say which source kinds may be drafted from: transcript, analysis note, brief,
  or all three. The brief is the one worth a moment's thought, because it names the people who will be in
  a room. (4) Confirm the approval ritual: an interactive terminal plus typing eight digest characters,
  with no bypass flag. If that is too heavy for how you actually share things, change the ritual now
  rather than adding a bypass later. (5) Nothing to install or seed beyond the existing review flow:
  every `vpp confirm --term` during transcript review improves every future draft, so expect the first
  few candidate reports to be long. Open questions: (1) What is a shared draft made of: an extract, or
  written prose? For an extract the residue scan is a real mechanical check; for prose a paraphrase can
  reintroduce a redacted fact in words the pass never saw, and the human read is the only protection. (2)
  What is the retention of drafts and of released copies? A draft directory holds the map from each
  placeholder to the real value it replaced, which makes the draft tree the most sensitive directory in
  the chain, and vpp deletes nothing. (3) Should pseudonyms be stable across drafts? Stable numbering is
  friendlier for a recipient reading several and lets two drafts be correlated by anyone holding both;
  per-draft numbering is the proposed default. (4) May a brief be drafted from at all? It concentrates
  participants and context, which is what makes it useful and what makes it the most exposing source in
  the chain. (5) Should an approval expire? An approval taken today and released in three weeks reviewed
  the same bytes but not the same situation. (6) Does the released file say it came from vpp? The
  proposed source line says only that the file is a redacted extract and not a verbatim record; naming
  the tool is more honest about provenance and tells a recipient a recording exists. (7) Is the PDF path
  in scope later? The vault already has a PDF export recipe, and a PDF carries producer, timestamp and
  sometimes path metadata that a Markdown file does not. (8) If `minutes` stays, should its `vocabulary`
  feed terms alongside known-terms.txt? Two lists that disagree would mask a name in one pipeline and not
  the other. Downstream of the minutes keep-or-replace ruling. (9) Where does vpp's code live, and what
  is it called? Carried forward unresolved from the boundaries design.
- [ ] Keep vpp application code in its own project, Mac installation and service configuration in
  dotfiles, output content in the configured directory (Ivy for this operator), and homelab deployments
  in homelab. Reuse existing transcription tasks. vpp must work without Bob, Forzare or the full homelab;
  Forzare integration follows the existing post-modernization ordering. Designed 2026-09-14 as
  `docs/superpowers/specs/2026-09-14-vpp-project-boundaries-design.md`. It compares three code homes (a
  fifth cargo workspace in dotfiles, its own repository built from a local clone, its own repository
  installed by `cargo install --git`) and recommends the second: `webdavis/vpp` from day one, built in
  the scalebar shape, which is already the proven precedent on this machine (`.chezmoidata/scalebar.yaml`
  plus a deferral-guarded builder plus one LaunchAgent). dotfiles' share is named file by file and stops
  at installation and service configuration; the configured output directory holds notes and links under
  the existing `agent-processing-pipeline/` layout; homelab keeps Open Notebook (L6) and any remote
  engine, all optional. Apple's container stays the canonical original and is read-only to vpp, including
  a `mode=ro&immutable=1` SQLite open, with one archive copy outside git because the vault ignores audio
  and no machine backup exists yet. Measured while writing: a `com.webdavis.vpp.plist` and a
  `~/.cargo/bin/vpp` fall outside the osquery known-good manifests, so vpp adds no CRIT coupling; pns
  needs no producer registration and `needs_attention` is real
  (`pns/crates/pns-protocol/src/request.rs:56`); `minutes` is already installed, already documents voice
  memos, and already owns the vault's `agent-processing-pipeline/minutes` symlink. The independence
  done-means is written as four absence checks (dotfiles absent, pns absent, network absent, vault and
  Obsidian absent) rather than mocks of Bob and Forzare, which do not exist yet. Waiting on the operator:
  own repository versus fifth workspace, and the shipping name, since `VPP` is FD.io's Vector Packet
  Processing and the repository's rules discourage new acronyms. No code written. Full document:
  `docs/superpowers/specs/2026-09-14-vpp-project-boundaries-design.md`. Operator steps: (1) Read
  docs/superpowers/specs/2026-09-14-vpp-project-boundaries-design.md and answer the seven open questions
  at its end. (2) Decide the code home first: own repository (recommended) or a fifth cargo workspace in
  dotfiles. Everything else in the document hangs off that answer. (3) Decide the shipping name before
  any repository is created; `VPP` is taken by FD.io's Vector Packet Processing (verified on fd.io) and
  renaming after install instructions circulate is a breaking change. (4) If the own-repository answer
  holds, create `webdavis/vpp` and clone it to ~/workspaces/Ivy/webdavis/vpp; the dotfiles builder is
  written to defer cleanly until that clone exists, so no apply is blocked in the meantime. (5) Rule on
  `minutes`: it is already installed, declared as a cask, documents "meetings and voice memos", and owns
  the vault's agent-processing-pipeline/minutes symlink. Answer before vpp's ingestion design is
  approved, since adopting it removes a layer. (6) Decide whether the audio archive copy may live
  unbacked on one machine until the restic work exists, or whether a backup is a prerequisite. (7) No
  apply, no build and no code are needed for this item; it is a document waiting on decisions. Open
  questions: (1) Own repository (`webdavis/vpp`) or a fifth cargo workspace inside dotfiles?
  Recommendation: own repository, matching the 2026-09-05 plugin ruling and the 2026-09-08
  shippable-product ruling. (2) Does the tool keep the name `vpp`? The acronym belongs to FD.io's Vector
  Packet Processing and the repository's own rules discourage introducing uncommon acronyms; the naming
  memories ask for self-documenting, user-agnostic names. (3) Which copy of the audio is canonical, and
  is one unbacked copy acceptable until a real backup exists? Recommendation: Apple's container stays
  canonical, the archive copy lives outside git, and backups stay a separate ledger item. (4) Do vpp's
  notes share the vault's existing `transcripts/` and `analysis/` directories, or get their own
  `agent-processing-pipeline/vpp/` subtree beside the `minutes` symlink? (5) Is `minutes` in or out? It
  already lists and searches voice memos and already owns a directory inside the vault; if it is in,
  vpp's scope shrinks, and if it is out, its open tool evaluation should record that vpp supersedes it.
  (6) Do the vault's folder-note and frontmatter conventions apply to machine-written notes, and who
  maintains the folder note for a directory a tool writes into? (7) Should vpp's binary, configuration
  and LaunchAgent join posture's user-configured watch list? They sit outside the osquery known-good
  manifests by default (verified), so this is an opt-in rather than a consequence.

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

- [x] Install and configure [gnhf](https://github.com/kunchenguid/gnhf), the overnight agent orchestrator
  ("each iteration makes one small, committed, documented change towards an objective"), requested by the
  operator on 2026-09-14. It is an npm CLI, so it goes on the fnm lane in
  `.chezmoidata/system_packages_autoinstall.yaml` (pinned, like the other npm tools there), with its
  `~/.gnhf/config.yml` deployed from source through chezmoi (secrets by keepassxc reference only) and
  `GNHF_TELEMETRY=0` set in the managed shell. Point it at the Claude CLI in non-interactive mode, decide
  which repository and objective it runs against first, and wire its run into the same worktree rule as
  every other agent (`herdr worktree create`). Its overnight runs are one of the four triggers for the
  Discord progress recap (see the pns recap task), so land that recap producer before the first
  unattended night. A second tooling wave 1 attempt (2026-09-14) got further: the `feat/gnhf-install`
  worktree folded the stray `graphify-out/graph.json` (commit `268a3543`) and merged `origin/main` clean
  (merge commit `89ff0f58`), but `just ship` failed at
  `cargo test --workspace --manifest-path pns/Cargo.toml -p pns --test daemon` on both the initial run
  and the one authorized rerun, in a workspace this branch never touches:
  `lifecycle::a_hung_child_does_not_stall_the_tick_and_is_killed` panicked "the hung job never started"
  on both runs, and the rerun also failed
  `spool::an_irregular_spool_entry_is_left_alone_and_never_opened` with a "database is locked" panic,
  with interleaved stdout consistent with concurrent machine load from other active sessions. No pull
  request was opened; the worktree is clean at merge commit `89ff0f58` on `feat/gnhf-install`, not
  pushed. Gated on rerunning `just ship` (or at minimum `just test-rust -p pns --test daemon`) once
  machine load has settled, then continuing from the push step. A later attempt on 2026-09-14 shipped:
  [PR #596](https://github.com/webdavis/dotfiles/pull/596)
  (`feat(gnhf): install the overnight agent orchestrator`, merged) pins `gnhf@0.1.49` on the fnm lane,
  deploys `~/.gnhf/config.yml` (agent `claude`, `conventional` commit preset, three consecutive failures
  before stopping, `preventSleep` on) as a plain file since gnhf reads no secrets, exports
  `GNHF_TELEMETRY=0`, and adds `docs/gnhf-objective.md` as the first objective file and
  `docs/runbooks/local-agents.md` documenting the invocation, the worktree procedure and the graphify
  hook interaction. `just ship` passed twice locally and the pre-push gate passed. Nothing runs gnhf
  automatically and `--push` stays off, so a run never reaches GitHub without the operator pushing by
  hand. Operator step left: run `chezmoi apply` to install the binary and deploy the config. Closed
  2026-09-15: the full apply ran and passed, `command -v gnhf` resolves on the fnm lane, and
  `~/.gnhf/config.yml` is deployed.

- [ ] 2026-09-14: [GitButler](https://gitbutler.com/) was installed for AI agents in
  [PR #603](https://github.com/webdavis/dotfiles/pull/603)
  (`feat(gitbutler): install GitButler and wire its agent skill through the managed store`, merged),
  requested by the operator, not a numbered task. The `gitbutler` Homebrew cask (`auto_updates true`)
  delivers both the desktop app and the `but` CLI; `but --version` reports 0.22.3 from the cask alone.
  The GitButler CLI skill was vendored into the cross-harness store (`dot_agents/skills/gitbutler/`,
  `on-demand` tier, no `forks` drift-watch row since `but skill check --global` is the staleness signal
  instead), with the Claude Code symlink declaration and the matching Codex overlay. The wizard's global
  workflow instructions went into `.chezmoitemplates/global-agent-rules.md` as a `## GitButler` section,
  gated on `but status` succeeding. Workspace mode (`but setup`) was deliberately NOT run on this
  repository; it was only measured inside a throwaway clone, where it switches HEAD to a
  `gitbutler/workspace` branch, writes five local config keys, and installs two `.git/hooks` scripts that
  are inert here because `core.hooksPath` is set user-wide. A new runbook, `docs/runbooks/gitbutler.md`,
  records the install, the skill lane and the workspace-mode measurements. Operator steps left: run a
  full `chezmoi apply` (KeePassXC unlocked) and confirm with `but skill check --global`; decide whether
  to run `but setup` on this repository, given the branch switch would move the main checkout off `main`
  while other worktrees run from it; optionally launch `/Applications/GitButler.app` to log in, only
  needed for the GUI, cloud review or `but pr`; optionally remove the one stray
  `gitbutler.project.portedMeta` local config key that merely running `but` wrote during verification.
  The full `chezmoi apply` ran and passed on 2026-09-15. Still owed: `but skill check --global`, and the
  decision on whether to run `but setup` on this repository.

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
- [x] Correct #51's integration path before merging its documents. It targets `integration/modernization`
  at remote `034d9a07`, not main; its two-file review becomes 222 changed files against current main
  (`76b37ae4`). The local integration branch also has nine additional commits. Preserve that history and
  carry the two reviewed Forzare documents onto current main in an isolated branch, then review the final
  diff and explicitly supersede or update #51. Do not merge the old integration branch as a shortcut.
  Done on 2026-09-14 as PR #615 (branch docs/forzare-canon-on-main, open, waiting for the operator): the
  two documents at PR #51's head 67ff1c0b,
  docs/superpowers/specs/2026-07-11-bob-executive-assistant-design.md and
  docs/superpowers/plans/2026-07-11-bob-executive-assistant.md, copied byte for byte onto a branch off
  main (blob ids identical, 2 files, 7189 insertions, matching #51's own count); PR #51 and the forzare
  worktree were left untouched. Operator step: review #615, then close #51 in its favor or say what to
  change; no apply. For the reverify step (task 51 proper), 55 stale lines in 7 classes were listed and
  NOT edited: Bats and \*.bats (spec 3107, 3155; plan 121, 649, 655, 670, 672, 673, 682, 840, 841, 865,
  3859, 3913), `nix develop ... bats` (plan 840), scripts/lint.sh and its helpers (plan 29, 668, 669,
  839, 3030, 3159, 3160), dot_hermes/ instead of private_dot_hermes/ (spec 2724, 3164; plan 29, 38, 40,
  361, 440, 452, 538, 539, 542, 555, 886, 907, 1119, 2022, 2033, 2070, 3025, 3038, 3927), dot_local/bin/
  for launchd-run scripts instead of dot_local/libexec/ (spec 3155; plan 29, 38, 243, 251, 347, 350, 663,
  839, 864, 3028, 3114, 3175, 3229, 3912), the old uptime-watchdog path (plan 3034, 3060), and the
  pre-commit gate named as lint-check plus test where main runs test-unit plus gitleaks (plan 114, 3218).
  No paseo or tmux reference remains; the `relay` mentions read as a rename to pns. Closed 2026-09-15:
  [PR #615](https://github.com/webdavis/dotfiles/pull/615) merged, carrying both Forzare documents onto
  main, and PR #51 was closed in its favour.
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
