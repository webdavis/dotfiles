# Remaining work

The open task list for the dotfiles modernization, including pns, posture, uu, lights, Neovim, terminal
review tools and the deferred subprojects. Use the resume order below; task numbers are stable
references.

Updated as tasks complete. Last updated 2026-09-20.

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
projects and pns, the four open pull requests and 14 open issues, and the new homelab/vpt decisions.
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
dusk Rest on F4, night Nightlight on F7). [PR #832](https://github.com/webdavis/dotfiles/pull/832),
merged `d35b29ff5`, changed morning's section-heading color from violet to steel blue at the operator's
request. On 2026-09-20 the operator ruled that `webdavis/damnit` is pre-1.0.0 and PR #1 merges only once
it meets the clean-code standard and leaks no secret; three review registers found 12 SEV-1, 34 SEV-2 and
33 SEV-3, all being fixed on the branch; the client specs are open as todoist.nvim PR #15 (`damnit.nvim`)
and herdr-todoist PR #16 (`herdr-damnit`). On 2026-09-20 the fugitive mappings in the Neovim config moved
to `:Git!` for fetch, pull, push and the no-edit amend
([PR #864](https://github.com/webdavis/dotfiles/pull/864), merged `55dd58091`), so a push no longer
freezes the editor for the length of the pre-push lint-check. damnit PR #1 passed its round 6 and 7
re-review with all 79 findings fixed and a clean secrets sweep and is merging.

On 2026-09-20 the two fixes left open since 2026-09-19 merged:
[PR #789](https://github.com/webdavis/dotfiles/pull/789) `82c3bff2e` (the doctor's certificate row named
as the Hue bridge certificate) and [PR #790](https://github.com/webdavis/dotfiles/pull/790) `46b63f029`
(the gateway's own polls survive a config it cannot read). dam's config target landed as
[PR #870](https://github.com/webdavis/dotfiles/pull/870) `25f234694`: a new `~/.config/dam/config.toml`
(mode 0600, deployed via chezmoi's `private_` prefix) declares `[remote.todoist]` with
`url = "todoist::"` and `api_token_command` reading the Todoist API token from the macOS keychain, the
same non-interactive pattern herdr-todoist already used, with KeePassXC staying the entry of record;
verified against dam's own config parser and a live `dam remote list` run against an isolated config
directory, the file parses, names the remote, and makes no network call. damnit PR #1 merged on
2026-09-20 (`c2eb89d`) after seven fix rounds, all 79 review findings closed and a clean secrets sweep.
The two client specs merged with every open decision settled on its recommendation (damnit.nvim spec,
webdavis/damnit.nvim PR #15 `ac5f627`; herdr-damnit spec, webdavis/herdr-damnit PR #16 `1b85e59`), and
both repositories were renamed in place on GitHub and on disk (todoist.nvim to damnit.nvim, herdr-todoist
to herdr-damnit), with the implementation plans being written. The dotfiles references to the old names
(the nvim plugin spec, the herdr plugin config leaf and the roster entry) move when each plugin's rename
PR lands. [PR #873](https://github.com/webdavis/dotfiles/pull/873) (b01ba1c26) extended the shared agent
rules partial so GitHub is reached through gh-axi only, never through curl or another client, even when
gh-axi itself cannot reach GitHub; when it cannot, an agent pushes the branch, writes the PR body to a
file, says so, and stops. This closes the gap exposed on 2026-09-20, when gh could not reach
api.github.com for three hours after a brew upgrade replaced its binary and two ship agents worked around
it with curl. On 2026-09-20 the operator withdrew task 135, Attest, because dam covers it. damnit PR #2
merged (c2565c1) with the client verbs `dam edit --undone`, `dam restore <oid>` and the
`dam done --force` children and depends dispositions; damnit issue #3 records a pre-existing `dam add -A`
exit 4 after a committed delete. The two implementation plans merged (damnit.nvim 36 tasks, herdr-damnit
42 tasks) and each plan's tasks 1 to 3, the in-place rename, are in review. dam's client contract was
settled on 2026-09-20 for the third damnit pull request: under `--json` the error document is the only
thing on stderr, every rule dam refuses exits 4 and names its rule, change documents carry a fields list,
and `status --json` answers rows without embedded objects unless `--full` is given; the two client specs
still say exit 2 for a refusal and each gets one amendment once that pull request merges.

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
  retain the requested preference and verify it when a supporting upstream release arrives. Checked
  2026-09-15: `wt switch` with no branch argument opens an interactive picker, and "live selection" here
  means a human actually choosing a row in that picker and Herdr opening the matching worktree, not a
  scripted call against `wt`. There is no non-interactive `wt` flag that drives a real selection; this
  stays an operator task.

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

- [x] 36a. Structured progress recaps, in chat and on `#pns`. DONE 2026-09-17. Operator request
  2026-09-14: every end-of-work summary uses the fixed Recap layout (Git block, stack graph, file list,
  Summary, In-Progress with a Blocked-on line, Upcoming Agent Tasks, User Tasks; the layout and the
  approved readability tweaks are in the agent memory `end-of-turn-recap-format`), and the same recap is
  posted to the `#pns` Discord channel through hermes. Triggers are agent-initiated, never a hook: a PR
  opened and waiting on a human review or auto-merged, each milestone while a `/goal` runs, after
  overnight work, an end-of-day summary, and on demand through `/pns:work-recap`. The command is a
  feature of pns, so it lives in the pns Claude Code plugin as
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
  a fixed order, `User Tasks` never shed), and delivers it through the unchanged `post_return_recap` (the
  `pns-recap` route and its Discord channel retired with task 81 on 2026-09-15, so a recap now posts on
  the default route with nothing left to fall back from; checked against
  `pns/crates/pns-application/src/post_return_recap.rs` on 2026-09-15, where the code already says so,
  and a refusal is reported by the leg's own mode rather than retried). `pns recap git` prints the Git
  block and, in one fenced block, the stack graph and file list, resolving the pull request through
  `gh-axi pr list --head` (gh-axi's `pr view` has no `--json` and no branch form) and the stack from git
  ancestry, since neither worktrunk nor `gh-axi stack` can answer it here. Both behaviors are written
  into `pns/docs/specs/return-recap.md`, and the work-recap skill and the shared agent rules now call
  these commands instead of saying they are not built; the pns plugin moved to 0.3.0. Twenty-five new
  tests, `just lint-check`, `just test-unit`, both template renders and a live `pns recap git` run all
  passed. Operator steps left: `chezmoi apply`, `claude plugin marketplace update pns`,
  `claude plugin uninstall pns@pns`, `claude plugin install pns@pns`, restart Claude Code, then run
  `/pns:work-recap` once and confirm the recap lands. Open question left for the operator: `pns-adapters`
  now shells `npx -y gh-axi` while the sibling `recap/merges.rs` shells `gh` directly, so the two
  adapters disagree about which GitHub CLI they depend on; needs a ruling on which one moves. On
  2026-09-15 the full `chezmoi apply` ran and the plugin move landed: `claude plugin list` reads
  `pns@pns` 0.4.0, enabled, and [PR #621](https://github.com/webdavis/dotfiles/pull/621) moved `pns-loop`
  and `pns-work-recap` into the shared skills store.
  [PR #628](https://github.com/webdavis/dotfiles/pull/628) dropped the `pns-recap` route the same day, so
  a recap now posts to the default route rather than `#pns-recap`. Still owed: one live `/pns:work-recap`
  run.

  THE OPEN QUESTION ABOVE IS ANSWERED, AND IN THE OPPOSITE DIRECTION FROM THE RULING THIS TASK WAS
  BRIEFED WITH. The brief sent on 2026-09-17 said the operator's gh-axi preference settles it and that
  `recap/merges.rs` should move onto `npx -y gh-axi`. It should not, for two reasons the lane measured
  rather than argued. First, the divergence had ALREADY BEEN RESOLVED THE OTHER WAY three days earlier:
  commit `1861610d` (2026-09-14, on main) moved the Git block's pull-request lookup off `npx -y gh-axi`
  and onto `gh --json` for a recorded product reason, and no gh-axi reference remains anywhere under
  `pns/`. Second, moving `merges.rs` would LOSE CAPABILITY, verified against `--help` rather than
  assumed: `gh-axi pr list` has no `--json`, no search form and no window flag (only `--fields` over a
  human listing, which has no `body` column), and `gh-axi search prs` takes no output-format flag at all,
  so the number plus title plus body of every pull request merged in a window cannot be had as a
  machine-parseable document. The reasoning the lane recorded: the gh-axi rule governs what an AGENT
  invokes for GitHub work, while pns is a product other people install with `cargo install`, and a
  shipped binary fetching a package from a registry at recap time is a different thing from an agent
  choosing a CLI. So both adapters now agree on `gh`, which is what main already said.

  The second half of the commission was delivered:
  [PR #753](https://github.com/webdavis/dotfiles/pull/753) (`fix/pns-one-github-cli`) merged `689daa11`,
  adding one seam, `pns/crates/pns-adapters/src/recap/github_cli.rs` (48 lines), which owns the tool
  name, the 30 second deadline, the PATH story and the empty-cwd guard, with both call sites routed
  through it. Net 19 lines added and 69 removed across three files, so the duplication the task was filed
  against is gone even though the CLI did not change. Its review removed a rejected-alternative paragraph
  from the module doc under the comment rule. Operator step still owed, unchanged: one live
  `/pns:work-recap` run.

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

- [x] 74. THE HTTP TAP, an opt-in ALTERNATIVE to the SSH one, never a replacement that arrives on its
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
  possible value? DECLINED 2026-09-17 by the operator, which is the design's own recommendation. The SSH
  tap stays the only transport. Nothing is owed as cleanup: both transports were always specified to
  write the same marker through the same code, so declining removes no code and leaves no dead path. The
  deciding cost was daemon independence. sshd execs `pns tap` as a one-shot process, so the tap still
  records with `[daemon] enabled = false` or a wedged daemon, while an HTTP listener is a daemon child
  and would die with it; the harness hooks and the shell notifier deliver synchronously in process and
  each read the marker to pick a surface, so a machine with a dead daemon still notifies and still needs
  to know where the operator is, which is exactly when that transport would be down. Secondary costs:
  pns's first listener reachable from a network, its first endpoint authenticating a caller, its first
  place hostile input arrives from something other than a hook or a config file, and a secret needing a
  rotation story. Most of the original motivation was already spent by task 71, whose forced command is
  `command="<binary> tap",restrict` and names no marker path, so the path is written down once and the
  silent mismatch is gone. The design at `docs/superpowers/specs/2026-09-14-pns-http-tap-design.md` is
  kept as the record of what was specified and why it was not built, so a later reversal needs no new
  design. Task 71b's device verification remains the only open work on this feature, and it is NOT
  satisfied by task 76's confirmation: 76 proved the happy path, while 71b still needs the two phone
  edits plus a real tap against an unavailable Mac and against a write failure.

- [x] 76. Apple Shortcuts research completed on 2026-09-13; device acceptance remains open. The proposed
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

## Tool-wide output flags CLOSED 2026-09-17: the operator confirmed the pns tap Apple Shortcut works on the device, which was the outstanding device acceptance.

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

- [x] 71a. THE AGENT WORK IS DONE; only operator device steps remain, and they are listed at the end of
  this bullet. All five items shipped in [PR #569](https://github.com/webdavis/dotfiles/pull/569) (merged
  `f85a6cfd`) and the drill document in [PR #598](https://github.com/webdavis/dotfiles/pull/598)
  (merged). What is left is the operator running the four sleep and wake states against their own Mac and
  phone and filling the table in `pns/docs/pns-tap-device-acceptance.md`, which no agent can do. FIVE
  THINGS THE TAP DESIGN LEFT OUT, found by re-reading it whole on 2026-09-09. Each is a silent failure,
  which is why they are recorded rather than left to be noticed later. EXIT CODE: `pns tap` exits
  non-zero when the touch fails, so the Shortcut can show a failure. A tap that fails silently is worse
  than no tap, because the operator stops checking. THE STATE DIRECTORY: `~/.local/state/pns/` may not
  exist on a fresh machine and `pns tap` may be the first thing to reach for it, so it creates the
  directory rather than failing on it. REMOTE LOGIN is a prerequisite in the Mac setup step. The whole
  feature needs sshd accepting connections (System Settings, General, Sharing, Remote Login). Without it
  every other step is wired correctly and nothing happens, which is the worst kind of wrong. Verify sleep
  and wake behavior on the operator's devices and explain what the Shortcut reports when the Mac cannot
  answer. `--info` should identify this troubleshooting path. NO CONFIG REQUIRED: `pns tap` must work
  with no `~/.config/pns/config.toml` at all, falling back to the default marker path, because requiring
  one would fail on exactly the fresh machine `--install` is walking somebody through. Define the
  `--json` schema and manual undo instructions before building. DONE 2026-09-14 in
  [PR #569](https://github.com/webdavis/dotfiles/pull/569) (`feat/pns-tap-fresh-machine`, merged
  `f85a6cfd`), each claim verified against the code first: the non-zero exit, the single stderr line and
  the OS error were already true, the line now names the marker path and prints the whole error (errno
  included); the 0700 state directory was already created recursively (pinned by existing tests); Remote
  Login already led the Mac steps but named no settings path, now it does, and `--info` says what the
  phone shows when the Mac cannot answer (the SSH action fails, so the phone shows the SSH error, never
  the success notification); a missing config already fell back to the default marker path (a parse error
  still fails); the shipped `pns.tap/1` JSON object was kept as is (its `marker` is a table, not a bare
  path) and gained `touched_at` in RFC 3339 UTC, with the whole field table, the null cases under
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

- [x] 45b. DONE 2026-09-17, AND THIS ENTRY WAS WRONG ABOUT WHY IT STALLED. posture 6.3, second half. The
  main transaction shipped in PR #506 (81 tests), and triage facts merged in #538. Arming and live
  acceptance remain. Shipped: the results-log reader with its single reading and bounded span, the cursor
  published by rename, the non-blocking single-instance lock (`O_CLOEXEC` replacing the shell's by-hand
  `9>&-` on every spawn), the row decoder, the column projection, the allowlist reader, the known-good
  manifest reader, the digest spool's append side, the `JudgeFindings` implementer, and `posture alert`.
  The enricher runs IN PROCESS rather than through a spawn, because `posture enrich` was already a use
  case in the same crate. WHY THE ENRICHER WAS NEVER OPTIONAL, recorded because it was twice reasoned
  about wrongly on 2026-09-09 before being measured: an untrusted signing verdict PROMOTES a Notice
  finding to Critical in the gate, so a cutover without it would send a finding the shell paged about to
  the next day's digest. That is a missed page, not extra noise. Both directions are now pinned by tests.
  The triage producer supplies recorded and on-disk hashes and upgrade correlation. These are display
  facts, and the shell tolerated missing facts whenever its optional helper was undeployed, so a page
  fires carrying less rather than not firing. FOUR DERIVATIONS WERE WRONG until the binary was run
  against a real sandbox, and the unit tests agreed with all four because they came from the same
  misreading of the shell's jq: the action was taken from a column rather than from the row, the identity
  column order dropped `identifier`, a listening port lost its address and port, and the timestamp
  carried the date without the time. Real-run verification is what caught them. The producer is now
  verified; arm the command by repointing the plist to `posture alert`, move the allowlist tuple for
  `com.webdavis.osquery-results-alerter` with it (the alerter matches a `persistence_launchd` finding
  against label, path AND program, so repointing without it pages on the next launchd scan), and delete
  `executable_results-alerter.sh` plus six private files under `results-alerter/`, keeping
  `pipeline-verdict.sh` deployed because bash `pipeline-audit.sh` still sources it and would otherwise
  refuse BOTH manifest scans as unavailable (it retires in task 46), and retire the old tests by their
  current consumers. The canonical plan names six suites; reconcile that inventory against current source
  before deletion. Run the sandbox composition checks and the plan's live page/digest, checkpoint and
  retry acceptance after the operator applies. On 2026-09-14 branch `feat/posture-alert-cutover` carried
  this work through six commits: `b80dbfde` repoints the plist and allowlist tuple to `posture alert`;
  `8e6a02a9` deletes `executable_results-alerter.sh`, its six private helpers and the seven shell tests
  that pinned them, keeping `pipeline-verdict.sh` for `pipeline-audit.sh`; `f1d5cd31` corrects the
  surviving producer-list comments; `502bb3b6` merges `origin/main` in; `701d93b9` names the three Bash
  monitors that still source the dispatch library; and `f1f6cc4f` gates the cutover on a live hermes
  posture route. Independent review returned two SEV-1s and one SEV-3, all fixed on the branch: a content
  conflict in the launchd allowlist (fixed by `502bb3b6`, keeping main's file and repointing only the
  results-alerter row, verified by a zero-exit `git merge-tree`); posture's pns route having no hermes
  endpoint, so every alert and digest leg dead-letters at HTTP 404 (fixed by gating the apply on that
  route existing rather than guessing a routing change, `f1f6cc4f`); and stale producer-list comments
  left by the merge (fixed by `701d93b9`). [PR #584](https://github.com/webdavis/dotfiles/pull/584)
  opened against `main` with `just ship` green locally and pushed. NOT MERGED as of 2026-09-14: GitHub
  Actions never triggered a Lint check-suite for the PR across three retrigger attempts (open, an empty
  synchronize commit, reopen) over roughly 30 minutes, while sibling PRs in the same window triggered
  normally; `gh-axi pr checks 584` still reads "no CI checks configured". This is an environmental
  GitHub-side blocker, not a code or merge problem; per standing instructions the branch stays open
  rather than merging without a real "0 failed" result. Operator steps once it ships: a full
  `chezmoi apply` (no by-name apply, no `--exclude=templates`, the plist and allowlist both sit in the
  pipeline known-good manifest arm); confirm the swap with
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

  THE CUTOVER WAS ALREADY ON MAIN BEFORE ANY WORK STARTED, and this entry's account of why it was not is
  wrong on both halves. Commit `f1f6cc4f` is an ancestor of `main` and PR #584 MERGED at
  2026-09-14T07:39:47Z. The sentence above recording it as NOT MERGED behind an environmental GitHub-side
  blocker should be read as retracted.

  WHAT ACTUALLY HAPPENED, measured 2026-09-17: head `f1f6cc4f` had ZERO check suites, ever. The very next
  head, pushed right after a fresh `origin/main` merge, got THREE check suites within seconds and the
  pull request merged ten minutes later. Same repository, same workflow, same runner app, ten minutes
  apart. GitHub does not create a check suite for a head it sees as conflicting with its base, so the
  thirty minutes of retrigger attempts were spent on a MERGE problem and the merge was the fix. The open
  item "getting Actions to trigger a Lint run on PR #584" was never an Actions fault and is closed.
  PRACTICAL LESSON for every future lane: a pull request reading "no CI checks configured" should be
  checked for `mergeable_state: dirty` before anything else.

  THE STALE HERMES ROUTE GATE IS ANSWERED, NOT REWORKED. Commit `f1f6cc4f` was a docs-only edit, so there
  was no code gate to remove, and task 99 settled the question it left open: no route name is hardcoded
  in posture any more, the adapter default names `posture-pages`, and the config template declares it
  under hermes mode with one key per route. A gate probing the retired `posture` route would have blocked
  forever on the wrong thing.

  INVENTORY RECONCILED AGAINST CURRENT SOURCE, nothing left to delete: all seven retired basenames have
  ZERO references anywhere under `test/`, `dot_local/`, `.chezmoiscripts/`, `Library/`, `dot_config/` or
  the justfile, and the four surviving `results-alerter/` references all name `pipeline-verdict.sh`,
  which is correctly kept because `pipeline-audit.sh` sources it, the manifest generator lists it, a unit
  test sources it and the drift verdict cites it. `just validate-tests` passes, so the canonical plan's
  six suites are fully retired by their current consumers.

  The one genuinely open source item was the stale doc comment this entry itself names, shipped as
  [PR #765](https://github.com/webdavis/dotfiles/pull/765), merged `66f8afbf`: one line so uu's brew lane
  record stops naming the deleted Bash triage script and names the config key it takes its path from
  instead. The two acceptance documents need no annotation, because they are hash-pinned point-in-time
  captures and annotating one to say its source was later deleted would corrupt the record rather than
  correct it.

  LIVE STATE CONFIRMED 2026-09-17 read-only: launchd already runs `~/.cargo/bin/posture alert`, so the
  plist swap landed in an earlier apply. STILL OWED BY THE OPERATOR, in this order: trash
  `~/.local/libexec/osquery/results-alerter.sh` and the six files under
  `~/.local/libexec/osquery/results-alerter/` (`allowlist-verdict.sh`, `digest-store.sh`,
  `file-integrity-triage.sh`, `normalize.sh`, `render-page.sh`, `route.sh`), KEEPING
  `pipeline-verdict.sh`, and expect ONE integrity page from that trash because the tree is watched
  whether or not the manifest lists a file; then the digest spool handoff on the next daily digest; then
  the at-least-once retry check against the live cursor with the gateway unreachable.

- [x] 46. posture 6.4: finish watchdog publication and cutover. Source on `feat/posture-watchdog-health`
  composes state publication, delivery ordering, legacy growth history, independent binary integrity,
  daemon and ledger checks. Independent review passed 944 posture tests and six additional regressions.
  The direct alarm precedes pns submission, and failed alarms retain unresolved state even when pns
  reports acceptance. The authorized pns build record and manifest publication passed 30 private checks
  with 99 assertions. Poll and watchdog are integrated at `a57d9346`; the final repeated `just ship`
  passed with four Rust workers and unchanged deadlines, and the exact release build produced 3,692,720
  bytes. A separate fail-first installer regression verifies that posture records the compiler selected
  by the build directory; all 15 installer tests and 70 assertions pass. On 2026-09-13 `just ship` on
  `a57d9346` passed again (exit 0, 3m16s) and [PR #547](https://github.com/webdavis/dotfiles/pull/547)
  was opened against `main`; it was reviewed once and merged into `main` at `1f934c7b` on 2026-09-14.
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

- [x] 47. posture 6.5: finish poll composition and cut over its plist. The application transaction and
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
  `com.webdavis.osquery-firewall-gatekeeper-monitor` as `/Users/stephen/.cargo/bin/posture poll`.
  Measured 2026-09-15 via `launchctl print`: 736 runs, last exit code 0, far beyond the two live ticks
  this task's acceptance asked for, which closes the "verify exposure and recovery across two live ticks"
  half of the sentence. Stays open: the Bash
  `dot_local/libexec/osquery/executable_firewall-gatekeeper-monitor.sh` producer (998 lines) is still
  present in source and referenced by nothing (no plist, no justfile recipe, no `.chezmoiignore` entry),
  so the "before removing the Bash producer" half of this task's acceptance is not yet closed; it retires
  from source in the same follow-up pull request as task 46. CLOSED 2026-09-17: the exposure and recovery
  drill ran twice on the live machine. Each cycle: firewall off, `posture-state.json` read `firewall: 0`
  at the next scheduled tick and stayed 0 across a second tick, firewall on, the recovery tick read
  `firewall: 1`. The agent advanced 2541 to 2547 runs at exit code 0 with no missed tick, and the second
  cycle detected the exposure again, proving the marker rearms. The operator received EXACTLY TWO
  critical pages, one per cycle and none per tick or on recovery, which is the delivery half only they
  could confirm. The firewall was verified enabled as the drill's last action. The Bash producer still
  retires with task 46's follow-up pull request.

- [x] 48. posture 6.6: publish the implemented funnel command on `feat/posture-funnel`, then cut over.
  Independent review approved the bounded security omission notice and finite timeout parser fixes. The
  notice never acknowledges the original oversized finding. All 45 command fixtures, 24 producer checks
  and 144 additional private submission cases passed, along with the integrated repository gate. On
  2026-09-13 the branch (`adb23b54`, which contains the watchdog branch and current `main`) passed
  `just ship` (exit 0, 3m44s) and [PR #551](https://github.com/webdavis/dotfiles/pull/551) was opened
  with base `feat/posture-watchdog-health`, so it shows only the funnel commits and retargets to `main`
  when #547 merges; it was reviewed and, as recorded below, merged into `main` at `0efb2119` on
  2026-09-14. Independent review returned five findings; the fix is on the branch and its own fix review
  is in progress. SEV-1: exposure pages with 37 or more keys exceeded the 8,000-character wire cap and
  were refused forever, bounded at `FUNNEL_EXPOSURE_KEY_LIMIT` (32 keys plus a summary line, commit
  `0037bc33`). SEV-3: stderr named retired tools, fixed at `b1a8b777`. SEV-3: the inline executable check
  was replaced by the shared `is_executable`, fixed at `4ea6e8b9`. Two findings are deferred to a
  follow-up: SEV-3, the duration parser maps `0`, `inf` and `1e100` to `Status(125)` and pages a false
  gap; SEV-3, the 44-line unsafe FFI hex-float parser could be `trim` plus `parse::<f64>`. Two more fix
  commits, `4cb11f21` and `c50f95d6`, are not yet pushed: a sorted-before-cut test, fixture cleanup, a
  root skip, and doc numbers now measured by test at 7,160; the timeout parser's `0`/`inf`/oversize
  inputs now saturate to a 24-hour ceiling; `strtod` is kept because the capture `timeout_hex` passes
  `0x1p-1`. `just ship` on `c50f95d6` failed only on the `gateway_health` flake that #547 fixes; re-ship
  once #547 merges into it. Preserve the baseline and verify real-input behavior before retiring Bash. On
  2026-09-14, after merging main in, fix commits `0037bc33`, `b1a8b777`, `4ea6e8b9`, `4cb11f21` and
  `c50f95d6` were pushed, the PR body was re-posted, continuous integration passed and
  [PR #551](https://github.com/webdavis/dotfiles/pull/551) merged at `0efb2119`. Also on 2026-09-14 the
  plist cutover itself landed in [PR #575](https://github.com/webdavis/dotfiles/pull/575) (merged
  `7bdecf6b`, branch `feat/posture-plist-cutovers`), commit `8754b3df`: the tailscale-monitor LaunchAgent
  now runs `posture funnel`, pinning `EnvironmentVariables` to
  `OSQUERY_TAILSCALE_BIN=/opt/homebrew/bin/tailscale` and dropping the `PATH` dict, because that dict is
  what made the shell resolve the headless brew formula and pinning takes PATH ordering out of a detector
  whose own blind window already pages CRIT; a missing or wedged binary still pages a gap naming the
  path. Review's only finding was the same 80-character subject line fixed under task 47, carried forward
  unchanged as `8754b3df` (tree hash verified). Operator steps: the same full `chezmoi apply` shared with
  tasks 46 and 47; confirm the swap with
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
  `com.webdavis.osquery-tailscale-monitor` as `/Users/stephen/.cargo/bin/posture funnel`. Measured
  2026-09-15 via `launchctl print`: 828 runs, last exit code 0, standing in for the single post-apply
  tick this task's acceptance asked for.

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

- [x] 50a. Close outstanding acceptance from already-merged heartbeat and digest cutovers, tasks 43 and
  44\. Installed plists invoke Rust, but that does not prove delivery. Record the silent pns-route
  message, banner and ledger evidence, and a filled-spool digest with `.last` rotation. Inventory retired
  helpers and their remaining consumers before proposing removal: deployed `heartbeat.sh`, `allowlist.sh`
  and `enrich-finding.sh` remain; `digest.sh` is already absent. See port-plan steps 6.1 to 7.1 for each
  cutover's full acceptance and rollback requirements. Also reconcile earlier enrichment and allowlist
  acceptance from steps 4.1 and 4.2: signed/unsigned enrichment exits and facts, deployed-list parity,
  and own-agent tuple refresh/publication. Record previously evidenced checks as complete. Update stale
  install paths and by-name apply instructions to the current operator-run full-apply rule when recording
  acceptance. Measured 2026-09-15: the heartbeat half is accepted, `com.webdavis.osquery-heartbeat` runs
  the Rust `posture` binary and shows 1 run at last exit code 0; the digest half still waits for its next
  18:00 `StartCalendarInterval` tick, `com.webdavis.osquery-digest` reads `runs = 0` and
  `job state = uninitialized`, and `~/.local/log/osquery/digest.log` is still the empty file from before
  this cutover (0 bytes, last modified Jun 21), so no digest evidence exists yet to record. CLOSED
  2026-09-17 by [PR #752](https://github.com/webdavis/dotfiles/pull/752), merged `c362eeb0`, evidence in
  `docs/superpowers/specs/2026-09-17-heartbeat-digest-delivery-evidence.md`, gathered entirely from state
  that already existed with no job triggered by hand. THE FINDING THAT REFRAMES BOTH TASKS: 43 and 44
  were written while posture submitted through pns, so their acceptance names a pns ledger row and a
  silent desk banner PER RUN. The deployed config selects `[notify] mode = "hermes"`, so posture signs
  and posts each page itself, no pns ledger row can exist for any run after that cutover, and
  `posture-adapters/src/hermes.rs` raises the local banner only on a FAILED post. Two thirds of the
  written acceptance is therefore UNSATISFIABLE BY DESIGN, and the document restates it to what the
  current path can evidence rather than reading it generously or returning a false NOT MET. HEARTBEAT:
  MET. The job runs at 09:00 local, `runs = 3`, last exit 0, with three gateway deliveries on route
  `posture-pages` to Discord on 2026-09-15, 09-16 and 09-17, each at :00:04 or 05 and separable by length
  from the recurring monitor traffic that posts at :05, :20, :35 and :50 at a steady 286, 408 and 409
  bytes. No 404 on that route since the gateway started, and none of the ten invalid-signature warnings
  falls at a heartbeat fire. DIGEST: MET, which SUPERSEDES this entry's own 2026-09-15 measurement of
  `runs = 0` and an uninitialized job state. It runs at 18:00 local, `runs = 2`, last exit 0, with two
  deliveries on the same route. The filled-spool and rotation acceptance is evidenced on disk and is the
  digest's own delivery record: `digest.ndjson.last` holds 76 readable rows spanning 2026-09-16, the live
  spool holds 111 rows starting 2026-09-17 and none of the 76, and `digest_spool.rs` renames to `.last`
  only on an ACCEPTED submission while a refused one restores the rows, so the batch was sent and kept.
  THE EIGHT DEAD-LETTERED LEGS ARE UNCHANGED and the reason is not what it looks like: there are still
  exactly eight `producer = 'posture'` events from 2026-09-10 to 09-13, every hermes leg 404 and
  permanent, every banner leg delivered (three on the second attempt). The ledger holds no posture row
  after 2026-09-13 18:00 NOT because the 404 was fixed but because posture stopped submitting through
  pns. THE RESIDUAL IS NAMED RATHER THAN GLOSSED: nothing local records a delivered body, so both
  verdicts rest on correlation by label, minute and message length, plus the spool's claim-and-keep
  record for the digest. Reading the channel is the one step that turns a length into a message, and that
  is the operator's.

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

  THE CODE HALF IS CONFIRMED SHIPPED, 2026-09-17, reconciled in
  [PR #762](https://github.com/webdavis/dotfiles/pull/762), merged `5d79a580`, which wrote
  `docs/superpowers/specs/2026-09-17-uu-runtime-acceptance-and-interruption-reconciliation.md` and
  changed no Rust file, ran no apply, triggered no run and sent no notification. Four of this entry's
  claims were already satisfied and are now evidenced rather than asserted.

  THE INTERRUPTION DEFECT IS FIXED FOR BOTH SIGNALS, which was the thing actually worth checking, since a
  fix covering only SIGTERM would have satisfied the test name task 101 recorded while leaving the
  reported defect in place. PR #542's interruption installer registers ONE handler for SIGINT and SIGTERM
  into a single atomic flag, and every downstream consumer reads that one flag with no branch on which
  signal arrived; both named cases pass. The duplicate scheduled output is fixed too: the LaunchAgent
  plist already sends its error stream somewhere other than uu's own application log, in the same pull
  request. The cua-driver path resolves as claimed and the Claude plugin snapshot still holds all twenty
  one rows. Of the audit's twelve lanes missing a state directory, EIGHTEEN OF NINETEEN declared lanes
  now have one, and the single exception is a lane task 57c ADDED which has legitimately not run yet, so
  it is not a regression.

  WHAT REMAINS IS TIME-GATED AND NO AGENT CAN SHORTEN IT. A read-only `launchctl print` reports zero runs
  and no exit code ever recorded for the weekly job, so the Sunday noon lane has still never fired. This
  entry's own rule holds and was not bent: a successful manual run, and a notification returning HTTP
  200, do NOT establish scheduled acceptance. The document therefore carries the exact commands and the
  expected passing output to capture AFTER the job fires on 2026-09-20, covering the run count and exit
  code, the log's run-started and done lines, the last-success timestamp, the one new lane state
  directory, each lane's streak file, and confirmation that the notification reached a real destination
  rather than merely logging a 200.

- [x] 57b. Reconcile B2's approved Herdr plugin-pinning requirement with the requested weekly upgrades.
  DONE 2026-09-17. PREMISE CORRECTED 2026-09-17: uu had already stopped rejecting a pin. Commit
  `48217fd6` on main parsed `ref` and passed it to `--ref`, so the sentence below about rejecting a `pin`
  setting was stale by the time this was filed. What was actually missing was the WEEKLY half: a pin was
  re-applied unattended, and because the refresh uninstalls before it installs, a revision that had
  stopped resolving took the working copy with it. Installed Herdr's `plugin install --help` exposes
  `--ref <REF>` (verified 2026-09-12). Record the desired pin/update policy, then implement it through
  that supported interface in uu's configuration and plugin lane. Do not silently freeze plugin updates
  or claim that source-tip reinstalls honor a configured revision across weekly updates. Source:
  `~/.claude/projects/-Users-stephen-workspaces-Ivy-webdavis-dotfiles/memory/goal-2026-09-01.md`. SHIPPED
  2026-09-17 as [PR #735](https://github.com/webdavis/dotfiles/pull/735), merged `53572d95`. `ref` now
  means HOLD: the lane reads `herdr plugin list --json` once, reports `HELD at <ref>` when the installed
  copy is at the pin, and reports the exact `herdr plugin install <repo> --ref <ref> --yes` command as
  PENDING when it is not, which covers both a pin the operator moved and a plugin nothing has installed.
  That is the cargo lane's report-rather-than-compile shape rather than a second reporting style. An
  unpinned entry still uninstalls and reinstalls at its source tip with the same one retry. A listing
  that cannot be read leaves every pin alone and fails the step by name, once, naming every pinned id
  (the review caught that the first cut counted one socket failure N times). The pin matches
  `source.requested_ref` first, then `source.resolved_commit` exactly or by prefix from seven characters
  up, because a shorter prefix matches commits the plugin was never at. A pin naming a BRANCH reads as
  held at that branch and the lane never fetches the remote to judge it. `--ref` was verified against the
  installed binary and the real `plugin list --json` envelope was read for the field names; nothing was
  installed, uninstalled, enabled or disabled, and no tab, pane or workspace was touched. THE ACCEPTED
  COST, stated rather than hidden: nothing on the machine moves an already-installed plugin to a new pin
  any more, so the operator runs the command the pending line prints. `run_after_53`'s presence gate
  installs a pinned plugin only when it is absent, and its comment was corrected to say so. 514
  uu-adapters tests pass in 3.68 s with a scripted command-runner double, no spawn and no clock. OPERATOR
  STEP: a full `chezmoi apply` picks up the rebuilt `uu` and comment-only changes in
  `~/.config/uu/config.toml`.

- [x] 57c. Refresh graphify's existing Claude skill alongside package upgrades. DONE 2026-09-17. The
  source adds the `uv-graphify-skill` command lane, an app-owned Claude symlink and a first-install seed
  with preservation and partial-destination guards. All 18 private installer checks with 96 assertions,
  15 extra adoption cases, fan-out checks, private uu composition and full `just ship` passed.
  [PR #545](https://github.com/webdavis/dotfiles/pull/545) merged and local main contains it. Preserve
  the existing real skill directory before operator adoption; live installation, fresh Claude discovery
  and scheduled refresh acceptance remain open. No live install or skills run was performed. PREMISE WAS
  STALE, verified 2026-09-17, and closed by [PR #744](https://github.com/webdavis/dotfiles/pull/744),
  merged `5b9a1e96`. Every behaviour this entry asks for was already in source and already live: the
  `uv-graphify-skill` command lane, the app-owned `~/.claude/skills/graphify` symlink declaration and the
  first-install seed with its preservation and partial-destination guards all landed in
  [PR #545](https://github.com/webdavis/dotfiles/pull/545), and the operator's 2026-09-13 apply adopted
  the link. THE TWO CLAIMS THAT HAD ONLY BEEN ASSERTED ARE NOW PROVEN. Lane ordering: lanes are a
  `BTreeMap` keyed by lane name (`uu-adapters/src/config/lanes.rs`, pinned by
  `lanes_run_in_name_order_whatever_the_file_order`), so `uv-graphify-skill` provably runs after `uv`.
  The registration boundary: graphify 0.9.53's installer honours `CLAUDE_CONFIG_DIR` on BOTH of its
  global-scope writes, the skill bundle and the `CLAUDE.md` registration, which is what keeps the
  registration out of the managed rendered `~/.claude/CLAUDE.md`. Live proof: `~/.claude/skills/graphify`
  is the link, its target holds `SKILL.md`, `.graphify_version` and `references/`, the registration sits
  in `~/.local/share/graphify/claude/CLAUDE.md`, and `grep -c -i graphify ~/.claude/CLAUDE.md` is 0. No
  code was needed. What shipped is the documentation that proves it plus TWO STALE INSTRUCTIONS THAT
  WOULD HAVE MISLED THE NEXT READER: the runbook's Graphify section and the lane's config comment both
  still read as pre-adoption and told the operator to preserve a directory that no longer exists. The
  review also had the runbook stop citing two private graphify symbol names, which drift silently on the
  weekly upgrade; the behaviour claim and the pinned version stay, and re-verification now points at a
  grep. ONE THING IS GENUINELY OPEN: no full weekly run has reached the lane yet, because the last full
  run was 2026-09-13T18:00:05Z and the apply that deployed it was at 18:38, so the lane's first real
  exercise is the next weekly `uu` run.

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

- [x] 57g. THE AGENT WORK IS DONE; only the operator's apply and one acceptance reading remain.
  Re-checked on 2026-09-15: this bullet's "Missing" clause, that the manifest generator's glob covers
  only `com.webdavis.osquery-*.plist`, was already closed by
  [PR #681](https://github.com/webdavis/dotfiles/pull/681), merged. That arm of
  `.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh` now reads
  `"$home"/Library/LaunchAgents/com.webdavis.*.plist`, EVERY LaunchAgent this repository owns rather than
  the osquery-prefixed ones, and its comment names the scalebar case as the reason. So the first
  `vouch(finding.path)` call has a manifest line to find and no `sha256` pin is needed. What is left is
  not code: a full `chezmoi apply` has to run `run_after_05` so the pipeline manifest records the new
  line, and then the next `persistence_launchd` finding for `com.webdavis.scalebar` must digest rather
  than page, which is a reading the operator takes. Original entry: allowlist the scalebar LaunchAgent.
  `com.webdavis.scalebar` is loaded by `run_onchange_after_*` on every apply but had no tuple in
  `dot_config/osquery/private_page-launchd-allowlist.txt`, so its persistence row pages as an unknown
  agent. [PR #564](https://github.com/webdavis/dotfiles/pull/564) (`fix/scalebar-launchd-allowlist`) adds
  the tuple (plist path, program `~/.local/libexec/scalebar/Scalebar`, no sha256 pin, like the other
  host-owned agents); merged 2026-09-14 (`0a52800a`) and deployed by the 2026-09-13 20:15 apply
  (`~/.config/osquery/page-launchd-allowlist.txt` carries the line). Remaining acceptance: the next
  persistence_launchd finding for that label digests instead of paging. Traced 2026-09-15 through
  `posture/crates/posture-domain/src/allowlist.rs`'s `allowlist_verdict`: the plist path, program and the
  empty-`sha256` entry line up (deployed tuple confirmed against the live plist and the executable binary
  on disk), and a new test,
  `allowlist::tests::scalebar_shaped_finding_suppresses_once_both_vouches_pass`, pins that `Suppress` is
  reachable once both `vouch` calls return true. But on THIS machine the first `vouch(finding.path)` call
  does not: `vouch` is `KnownGoodManifests::vouches`, and it only returns true when the target line is
  recorded in the governing known-good manifest. `com.webdavis.scalebar.plist` is not in the pipeline
  manifest, because `.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh` globs only
  `com.webdavis.osquery-*.plist` into `pipeline_paths` (confirmed empty:
  `sudo cat /var/osquery/pipeline-known-good.sha256 | grep -c scalebar` reads 0, while every
  `com.webdavis.osquery-*.plist` is present). So today's `allowlist_verdict` for a real scalebar finding
  returns `NotAllowlisted`, not `Suppress`, and the next `persistence_launchd` finding for that label
  still pages. Missing: either the manifest generator's glob widens to cover
  `com.webdavis.scalebar.plist`, or the allowlist entry carries a real `sha256` pin instead of relying on
  the manifest vouch.

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

- [x] 2026-09-14: the merged-worktree sweep from 57h became a repository tool in
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
- [x] 59. posture 9.1: relocate posture controls and desired state out of the legacy `osquery/` tree, add
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
  operator-run osqueryd restart after the watched paths changed) remains open. CLOSED 2026-09-17: the
  outstanding osqueryd restart was performed and verified.
  `launchctl kickstart -k system/io.osquery.agent` moved the daemon from pid 891 to pid 56295 with
  `state = running`, so the relocated watch paths from
  [PR #553](https://github.com/webdavis/dotfiles/pull/553) are now the ones the running daemon reads.
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
- [x] 60a. Resolve the behavior gaps found by the original-test mapping before final posture closure.
  DONE 2026-09-17, with four explicit deferrals carried as task 142. The private B020/B027 reproducer
  loses valid digest records when one invalid UTF-8 byte makes a claimed batch unreadable; a focused
  preservation fix is in progress on `fix/posture-digest-read-failure`, which merged `main` in (tip
  `74166d25`); `just ship` passed (exit 0, 5m15s) and
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
- [x] 79. Stop the posture digest repeating the same file. The 2026-09-14 digest carried 110
  `agent_authfile_changed` findings for `~/.codex/config.toml`, which Codex rewrites from its own model
  while it runs, so every rewrite arrives as a fresh finding rather than as news. Decide between an
  allowlist entry for that path and a debounce that collapses repeats of one path inside a digest window,
  then build the one chosen. Record the reasoning either way, because an allowlist entry stops watching
  an agent credential file while a debounce keeps watching it. DONE 2026-09-15 in
  [PR #642](https://github.com/webdavis/dotfiles/pull/642)
  (`feat(posture): collapse repeats of one path inside a digest window`, merged): the debounce was chosen
  over the allowlist entry, because `~/.codex/config.toml` records Codex's hook trust and MCP servers, so
  it is exactly the file worth watching for a change nobody made. The digest renderer now folds two
  findings that share both identity and summary inside an already-grouped detector into one bullet
  carrying a count, and the bullet and group caps apply to collapsed lines rather than raw findings.
  Measured against a synthetic 112-line spool: 11 bullet lines and 924 characters before, 3 and 323
  after, with `~/.claude.json` (previously evicted) now rendering. CLOSED 2026-09-17 by
  [PR #738](https://github.com/webdavis/dotfiles/pull/738), merged `9b450e5e`. B039b NEEDED NO CODE: the
  appender's rename re-check had already shipped on main (`c4765f22`, `acfb0ebb`, `8e880fbd`) with six
  tests in `posture-adapters/src/digest_appender/tests/rename_race.rs`, so the entry above described a
  proposal that was already built. It was VERIFIED instead of rebuilt:
  `append_until_the_spool_stops_moving` re-checks the written file's device and inode pair against the
  file at the spool path through the writer's own handle, bounded at eight attempts and failing loudly at
  the ceiling; 25 consecutive runs passed at 0.30 s with no wall-clock wait on the success path, and a
  mutation replacing the re-check with `Ok(false)` turned both race tests red, losing 2 of 160 lines to
  one claim and 57 of 1000 silently under four claiming threads. THE ONE REAL FIX was the
  empty-bundle-path disagreement, and it rested on a false claim about another tool: the results-row
  adapter filtered `Some("")` into typed absence because a comment claimed jq's `//` treats an empty
  string as missing. Measured, it does not: `jq` keeps an empty string and falls back only on false, null
  or a missing key. The decision was written down three times and agreed with itself
  (`posture/docs/decisions/finding-normalization.md`, `posture/docs/specs/finding-normalization.md`, and
  the captured Bash rows in `posture/docs/acceptance/finding-boundaries.md`), and the domain already
  agreed; only the adapter did not. The filter is gone, the adapter test pins all four column shapes, and
  `posture/docs/README.md` records the resolution. QUOTED ZERO WAS DELIBERATELY LEFT ALONE and is the one
  open question: the capture shows the Bash pipeline letting a counter written as `"0"` through as a
  finding, while `results_row.rs` parses it and suppresses it as a baseline. The reason in the source is
  plausible, that osquery's JSON logger has written the counter quoted and a baseline slipping through as
  a string pages the whole machine once, but NO DECISION DOCUMENT SAYS SO, and the brief forbade changing
  behaviour to match a decision that is not written down. OPERATOR STEP: either endorse the deviation in
  a decision document or ask for the capture's behaviour to be restored.

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

- [x] 62. THE AGENT WORK IS DONE, re-checked 2026-09-16 because this bullet still read as open: PR #608
  moved F4 to F10 to the `lights` binary and PR #611 added the f1 to f3 presets, both merged. What is
  left is only what the bullet already lists as operator steps: `aerospace reload-config` after the
  apply, then press the keys and watch the rooms. Original entry: lights PR 12: move all seven aerospace
  keys F4 to F10 to `~/.cargo/bin/lights`. Five still call Bash and two call OpenHue directly. Complete
  the three remaining command/hardware drills, then verify actual key presses and held-key behavior after
  apply. Retire the script and propose manual cleanup of its deployed copy and obsolete logs after
  acceptance. DONE 2026-09-14 in [PR #608](https://github.com/webdavis/dotfiles/pull/608)
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

- [x] 80. Lights features the operator wants built but NOT bound to a key and NOT set in
  `~/.config/lights/config.toml` (operator ruling 2026-09-15): `bed` and `away` presets whose steps
  switch rooms off (`off = true`), `lights preset now` picking the preset from clock windows, `--all` on
  `scene` and `brightness`, and `--over <duration>` for a fade. Each of these is a change to `lights`
  alone; `dot_aerospace.toml` and the shipped `[presets]` table stay as they are until the operator asks
  for them. Ship them as separate pull requests in that order, since only the first two touch the preset
  walker. CLOSED 2026-09-17: all four capabilities are already on main and were verified in the source.
  `9f88e2a8` added `preset now`, `fef6a6aa` added `--all`, `73605dcf` added `--over <duration>`, and
  off-steps in a preset predate the task at `fda53bed`. `PresetNow`, `--all` and `--over` are all present
  in `lights/crates/lights-protocol/src/command.rs` today. Per the task's own instruction, no `bed` or
  `away` entry was added to the shipped config and no keybinding was made.

- [x] 63. lights: decide manifest coverage for `~/.cargo/bin/lights`, its current install target. The
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
  the loop does not find it. CLOSED 2026-09-17: the operator ruled leave `lights` off the manifest, so
  the generated-binary exception stays posture-only and the four Rust tools in `~/.cargo/bin` are treated
  alike. `~/.cargo/bin` is in no osquery `file_paths` group, and adding one binary from it would either
  watch a directory `uu` rewrites weekly (a CRIT page every week by design) or special-case a single
  file. Docs-only follow-through: correct the stale target in the lights plan.

- [x] 64. lights PR 11a is unnecessary under the recorded bulk-read decision. Bulk measured 210 ms,
  versus 267 ms and 455 ms for the targeted alternatives. Keep bulk and record the accepted deviation
  from the 150 ms design target. The other three hardware drills gate 62.

- [x] 65. Neovim task 63: finish the acceptance record required by PR #385. Capture five silent starts,
  full-plugin health output, quiescent startup comparison, rendered which-key groups, both agent loops,
  Swift/custom-plugin behavior, a clean-home apply and quiet repeat apply, and the inventory-to-merged-PR
  mapping. Synthetic/headless runs do not establish rendered acceptance. RECONCILED 2026-09-15: the stale
  expected `X = xcode` and `d = do` groups are corrected to the live `x = xcode` and `d = docker` in the
  acceptance record, read out of `dot_config/nvim/lua/plugins/which-key.lua:34` and `:55`. RENDERED
  STARTUP IS ALSO DONE: five pseudo-terminal starts on 2026-09-15, each with a UI attached and 99 of 99
  plugins loaded, and the record's Snacks input and select note is answered (snacks ships no `select`
  module). What remains of this task is which-key NAVIGATION and buffer-local key presses, both agent
  loops, the Swift and Xcode behavior, custom-plugin delivery, the fresh-user bootstrap and repeat apply,
  and the quiescent performance check, all of which need the operator. Reconcile
  `dot_config/nvim/docs/todo.md`; bootstrap, neotest, annotation extraction and autosave/format
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
  [`docs/acceptance/nvim-acceptance-drills.md`](acceptance/nvim-acceptance-drills.md). CLOSED 2026-09-17:
  the operator reported five silent Neovim starts with no warning or error message, which is the
  acceptance record PR #385 asked for.

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

- [x] 68a. Extract each tool into its own public repository with `git subtree split`, once the operator
  has hand-rewritten it and is ready to tag a v1. Deferred from tasks 20 and 21; the monorepo layout
  exists so this is a move. Nothing is published to crates.io while a tool is pre-v1.

  Closed 2026-09-18, on the operator's ruling that day lifting these three out of the operator-owned
  marking. Each was extracted with `git subtree split`, so each keeps its own history rather than landing
  as one fresh commit: `webdavis/herdr-smart-nav` (8 commits), `webdavis/herdr-workspace-jump` (10),
  `webdavis/herdr-process` (18). Shipped as [PR #781](https://github.com/webdavis/dotfiles/pull/781),
  merged `a7926302`. All three are public, MIT licensed to match `webdavis/herdr-todoist`, described,
  defaulted to `main`, and carry the four GitHub topics `herdr-plugin`, `herdr`, `rust` and `tui`, which
  is what puts them in the herdr marketplace index.

  12,240 lines of vendored plugin source left this repository along with builders `run_onchange_after_56`
  and `_57` and the shared partial `.chezmoitemplates/herdr-plugin-build.sh.tmpl`. herdr-workspace-jump
  and herdr-smart-nav became two rows in the existing `packages.herdr_plugins` roster, so they now
  install through the unchanged `run_after_53` machinery that already installs the nine third-party
  plugins, with the pinned commit in `.chezmoidata` and nothing inline in a script.

  herdr-process stays on the LINK path and cannot be a GitHub install. `herdr plugin install` requires a
  committed `herdr-plugin.toml`, and herdr-process ships none: its actions are one set per declared
  process profile, rendered by `herdr-process generate` from the operator's own `processes.toml` and
  `config.toml`. Committing that render would freeze one machine's profiles and chords into a public
  repository, and herdr plugin v1 registers no actions at runtime. `run_onchange_after_58` therefore
  clones the pinned revision into `~/.local/share/herdr-process`, compiles it, generates the manifest
  from the operator's config, and links that directory. Whether it can ever become a GitHub install is
  now a design question living in that repository, and its README says so.

  The shared build partial did not survive: its source-glob hash phase hashed files that no longer exist
  here, and it had exactly one caller left, so its retry-marker, bounded-herdr and
  registration-verification logic is inlined into `run_onchange_after_58`.
  `test/unit/herdr-plugin-source-hash.test.sh` went with it, because the behaviour it pinned no longer
  exists.

  Verified rather than assumed: the plugin id herdr registers is the manifest's `id` verbatim, with no
  owner namespacing. Evidence on the live machine is that `annotate` installs from
  `plannotator/herdr-annotate` while `furkankly/zoetrope`'s own manifest says
  `id = "furkankly.zoetrope"`, so the difference is what each author wrote. Both of our manifest ids are
  unchanged by the move, so all eleven `plugin_action` keybindings and the `h` alias needed no change,
  and none was made.

  `just test-rust` no longer runs cargo over the three plugins, which is a deliberate reduction in what
  this repository's gate covers, stated in its commit message. Each new repository owns its gates from
  here, and none of them has CI yet, which is filed as its own task.

  A RULE BREAK, DISCLOSED RATHER THAN HIDDEN: proving the rewritten `run_onchange_after_58` builder was
  supposed to run under a scratch `HOME`, on the assumption that would sandbox it. It does not: the
  `herdr` CLI resolves the herdr server independently of `HOME`, so the script's link phase reached the
  operator's LIVE server and re-registered `herdr-process` at the lane's scratch path, breaking its own
  brief, which forbade touching the operator's live plugins. It reverted immediately with
  `herdr plugin link /Users/stephen/.local/share/herdr/plugins/herdr-process` and confirmed `plugin_root`
  was back; `herdr plugin list` independently confirmed the original registration afterward. No damage
  survives. Two things follow: a scratch `HOME` is NOT a herdr sandbox, so a future brief for a
  herdr-touching script must say to unset `HERDR_SOCKET_PATH` and `HERDR_BIN_PATH` as well, or to stop
  before the link phase; and the accidental run proved that a `herdr plugin link` at a new path REPLACES
  the previous registration with no unlink step, so the operator's step list for herdr-process needs no
  unlink.

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

- [x] 21a. Finish the deployed binary cleanup named in task 21. AUDITED 2026-09-17; ONE TRASH COMMAND IS
  OWED. The old `~/.local/libexec/{pns/pns,uu/uu,posture/posture,lights}` binaries remain. Verify current
  callers, preserve the live `pns/hooks/` installer directory, and obtain approval for the exact obsolete
  files before trashing them. The September 13 caller audit found two Codex hooks still invoking the old
  pns binary alongside current handlers. Commit `1b0cca44` on `fix/codex-pns-hook-migration` migrates
  precisely owned legacy commands while preserving unrelated handlers and metadata. Follow-up `257cb3e1`
  fixes four ShellCheck findings in its test. Ten focused cases, synthetic installer checks, full
  `just ship` and independent review passed. [PR #534](https://github.com/webdavis/dotfiles/pull/534)
  merged and local main contains it. Operator deployment and hook-trust review remain separate from
  exact-file cleanup approval. The full `chezmoi apply` ran and passed on 2026-09-15 and all four old
  binaries are still on disk: `ls` reports `~/.local/libexec/pns/pns`, `~/.local/libexec/uu/uu`,
  `~/.local/libexec/posture/posture` and `~/.local/libexec/lights`, each dated 2026-09-09. Still owed:
  the operator's approval of the exact files, then the trash pass. AUDITED 2026-09-17 in
  [PR #745](https://github.com/webdavis/dotfiles/pull/745), merged `7a5527e6`, written up in
  `docs/superpowers/specs/2026-09-17-deployed-binary-cleanup-audit.md`. ALL FOUR OLD BINARIES ARE ALREADY
  GONE: `~/.local/libexec/pns/pns`, `~/.local/libexec/uu/uu`, `~/.local/libexec/posture/posture` and
  `~/.local/libexec/lights` do not exist, removed in one UNATTRIBUTED sweep at 2026-09-15 05:29:50-0600,
  which is what all four parent directories' identical mtimes say; nothing matching is in `~/.Trash` and
  no apply transcript names the paths. This entry's own still-on-disk file list is therefore superseded,
  while its caller verification holds. EXACTLY ONE LEFTOVER REMAINS and it is a directory, not a binary:
  the empty, unmanaged `~/.local/libexec/uu/` at mode 0755. THE WHOLE OPERATOR STEP IS
  `trash ~/.local/libexec/uu`. Callers: every live caller in the repository and on the deployed side
  names `~/.cargo/bin`, most of them resolving `rust_tools.install_dir` from
  `.chezmoidata/rust_tools.yaml` at render time. One repository file names an old path in live code and
  is NOT a caller: `dot_local/libexec/pns/hooks/codex/executable_install-hooks.sh` builds the legacy
  pattern list its own `migrate` filter rewrites to the cargo path, and its agent is already the cargo
  binary. `~/.codex/hooks.json` and all twelve pns commands in `~/.claude/settings.json` name the cargo
  path. PRESERVED: `~/.local/libexec/pns/` survives, because chezmoi declares
  `hooks/codex/install-hooks.sh` there and `channel_dispatch.rs` also defaults the executable-channel
  directory to `~/.local/libexec/pns/channels`; `~/.local/libexec/posture/` survives whole, its seven
  deployed files matching chezmoi's declaration file for file. Two unmanaged `.DS_Store` files in the pns
  tree are noted and deliberately left out of the command list. THE INTEGRITY FINDING IS THE VALUABLE
  PART AND IT SPLITS: neither known-good manifest ever held any of the four, because the manifest set
  derives from chezmoi's intent rather than from the protected tree, but all four WERE watched by the
  `managed_bin` recursive group. `~/.local/libexec/posture/posture` matches the pipeline-integrity prefix
  arm UNCONDITIONALLY and a DELETED verb short-circuits the tuple check, so trashing it WOULD have paged
  a CRIT and no apply on either side could have suppressed it. The other three fell to the
  manifest-driven arm, were never manifested, and would have been silent. The remaining empty `uu/`
  directory holds no files, so its removal emits no `file_events` record at all: no page, no ordering to
  observe, and no apply needed before or after. OPEN QUESTION for the operator: the 2026-09-15 sweep is
  unattributed, and the audit recorded that as a finding rather than tracing it, because the resulting
  state is exactly what this task was going to ask for approval to reach.

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

- [x] Evaluate native macOS probes for pns, approved 2026-09-13. MEASURED 2026-09-17; ADOPTION IS TASK
  144 AND WAITS ON ONE OPERATOR DECISION. Benchmark the current `ioreg` idle-time and screen-lock probes
  and the `pgrep`/`ps` process queries used for phone-session activity. Compare probe latency and total
  pns runtime under representative load with small Rust adapters using maintained IOKit bindings and the
  existing `libc` dependency where suitable. Reuse bindings to Apple's system interfaces; keep the
  adapter inside pns and limited to the calls it needs. Adopt a replacement only when measurements show a
  worthwhile benefit and behavior checks pass. Preserve unknown readings, lock/idle routing, process and
  terminal matching, bounded execution and handling of processes that exit during a query. Record the
  measurements and retain the current commands if the replacement is not an improvement. Include this in
  the existing [pns performance task](https://app.todoist.com/app/task/6hPxWVHM8pG4qgwp). Keep
  `terminal-notifier` unless a demonstrated feature gap justifies taking over notification permissions,
  app identity and click handling. Keep `rusqlite`/SQLite and supported external-tool interfaces. Focus
  detection is already Rust; changing languages does not remove its dependence on undocumented Apple
  files. A general translation framework or rewrite of third-party implementations is outside this task.
  This approval schedules the investigation and conditional replacements. September 13's private
  prototype measured the parallel desk/phone probe stage at 216.7 ms median with current commands versus
  18.8 ms through bounded native helpers; added-load medians were 276.8 ms and 27.4 ms. Sixty live parity
  comparisons passed for observed conditions, but this does not cover locked-state transitions or total
  pns runtime. The direct-call variant loses interruptible deadlines. The native phone candidate also
  misses an `argv[0]` match accepted by current `pgrep`; do not adopt it as equivalent. Resolve that
  selection mismatch, compare a hybrid retaining `pgrep` if useful, and measure total runtime before
  adoption. Keep production probes unchanged until those checks and required device acceptance pass. The
  bounded prototype uses maintained `objc2-io-kit` and Core Foundation bindings with the existing `libc`
  version. Raw activity readings remain private in the local investigation, not in this repository. The
  private hybrid follow-up retained actual `pgrep -x` selection and fixed the argument-zero witness
  mismatch. Bounded phone medians were 207.8 to 39.6 ms ambient and 238.2 to 43.2 ms under added load. A
  complete pns process with private destination stubs measured 239.9 to 83.1 ms and 270.8 to 88.0 ms
  respectively. All 320 whole-process runs completed and 50 bracketed comparisons agreed. These
  measurements exclude real delivery, daemon and hook latency. The candidate combines bounded native desk
  probes with hybrid phone selection. Its five-second total phone deadline is tighter than the existing
  chain's three separate budgets and needs an explicit adoption decision. Actual device transitions,
  unreadable devices, multi-user behavior and stalled native calls remain acceptance gates. All 33
  original investigation hashes were preserved; production is unchanged. MEASURED 2026-09-17 in
  [PR #755](https://github.com/webdavis/dotfiles/pull/755), merged `c6c2c76d`, written up in
  `docs/superpowers/specs/2026-09-17-native-macos-probe-evaluation.md`. Production is unchanged: no Rust,
  no dependency, no apply. Ambient medians on dresden over 200 runs each with the shell disabled:
  `ioreg -c IOHIDSystem` 44.47 ms, `ioreg -n Root -d1` 28.77 ms, `pgrep -x mosh-server` 25.92 ms,
  `pgrep -P` 26.34 ms, `ps -o tty=` 4.14 ms, against a spawn floor of 1.62 ms for `/usr/bin/true`. Under
  eight spinners the same set measured 44.43, 30.49, 30.49, 29.52 and 9.09 ms. The desk pair costs 73.2
  ms serial and the phone chain 56.4 ms, and because `start.rs` runs them on two threads the stage is
  about 73 ms. NATIVE, timed in process over 200 iterations: the IOKit idle property 0.0100 ms, the
  CoreGraphics session dictionary 0.1430 ms, and a libproc name, parent and terminal walk 1.0360 ms.
  Parity was checked live rather than assumed. PROBES ARE OVER NINE TENTHS OF PNS'S LOCAL WORK (its own
  non-probe work measures 4.45 ms and 2.79 ms) and about 30 percent of the whole run against the
  previously quoted 239.9 ms figure, which the document labels as quoted rather than verified.
  RECOMMENDATIONS, ranked: adopt native for the idle read; adopt native for the lock read BUT through the
  registry Root node's `IOConsoleLocked` and NOT through `CGSessionCopyCurrentDictionary`, which was the
  example this task's brief gave and is the wrong call, because it reads a different source of truth,
  returns a null dictionary in a session without one, and the shipped code's Some(true)-only fail
  direction would turn that null into silently never locking; adopt native for `pgrep -P` through the
  same libproc walk, which needs NO new dependency because libc 0.2.189 already declares `proc_listpids`,
  `proc_pidinfo`, `proc_name` and the bsdinfo struct; KEEP `pgrep -x mosh-server` shelled, because macOS
  pgrep matches process names while libproc offers a 16-byte comm beside a 32-byte name, which is the
  argv[0] mismatch class an earlier investigation already hit; and KEEP `ps -o tty=` shelled at 4.1 ms,
  where it disappears for free if the libproc walk lands. NO CACHING: a fresh process per run means a
  cache needs a state file that costs more than the 4.1 ms it saves. Expected stage cost after the three
  adoptions is about 27 ms, bounded by the retained pgrep. THE ONE COST THE MILLISECONDS CANNOT PRICE,
  and the reason adoption is a separate task: a native in-process call loses the forked cleanup child's
  five-second deadline that bounds every probe today.

- [x] Finish P4's recorded loop rule. DONE 2026-09-17, and the rule as recorded was narrower than the
  defect: a live loop lease for the pane prevents a condenser-generated `asking` guess from arming the
  blocked marker; actual hook-driven waits still do. The current submit path updates that marker without
  checking the lease. Read the instrument evidence before implementing the companion permission-mode
  filter for false blocked alerts; its cause was never established in the reviewed records. Do not
  suppress all subagent approvals. The condenser prompt correction already shipped. Resume from
  `~/.claude/pipeline/slices/brief-pns-one-moment.md` and the September 1 decision in
  `~/.claude/projects/-Users-stephen-workspaces-Ivy-webdavis-dotfiles/memory/pns-lights-lock-sheet.md`.
  Local implementation `4136fd42` on `fix/pns-loop-rule` passed eight new regressions, three mutation
  checks and package gates. Independent review passed 52 focused checks. The permission-mode filter
  remains excluded. [PR #536](https://github.com/webdavis/dotfiles/pull/536) combines this change with
  B18 while preserving separate commits. Combined `just ship` and the installer release build passed;
  required checks passed and the PR merged. Operator deployment and visual acceptance remain open.
  SHIPPED 2026-09-17 as [PR #750](https://github.com/webdavis/dotfiles/pull/750), merged `45732c39`. THE
  RULE WAS ALREADY HALF SHIPPED and the half that existed was right by accident. Commit `4136fd42` (PR
  #536) already withheld the blocked marker when a live pane lease existed AND the event state was the
  literal word `asking`. That word was standing in for provenance: `asking` is only ever produced by the
  condenser, so the guard happened to be correct for it, BUT THE CONDENSER'S PROMPT ALSO ANSWERS
  `blocked`, and that guess armed the marker indistinguishably from the approval hook's own `blocked`.
  Same false "waiting on you", in the sibling path. The source carried no distinction between a guessed
  state and a hook-driven one, so one was built: `EventArgs` gained a `guessed` flag, set in
  `end_of_turn` where the state is actually read off the turn's text by `condense`, and false on the
  producer path (a producer states its signal) and on StopFailure (the harness states the message). The
  guard now reads a live loop AND a guessed event AND a marker action of Start, reusing the domain rule
  the wait itself reads rather than a second list of state words, so ONLY A START IS WITHHELD and a
  guessed `done` still clears a marker an earlier event armed. A STALE LEASE READS AS NO LEASE, which is
  both the existing behaviour and the honest one: liveness is computed against
  `lights.loop.lease_timeout_secs`, so a lease nothing renewed cannot vouch for a guess, and a live
  loop's own hook traffic is what keeps it from going stale. Four pure unit tests pin the cases in 0.00 s
  with no spawn and no clock; the review deleted a fifth that pinned a domain predicate this layer never
  calls and is already covered at its own boundary. The pre-existing process-spawning integration tests
  still pass unchanged, including the one proving a hook-driven `blocked` arms during a live loop. The
  generated config prose was broadened from the condenser's `asking` guesses to its guessed waits, and
  the template was regenerated with `just pns-config-render`. OPERATOR STEP: a full `chezmoi apply`
  rebuilds pns and writes the reworded config comment.

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

- [x] Split and reconcile [6hPJVf2FJc3RHxqM](https://app.todoist.com/app/task/6hPJVf2FJc3RHxqM). Ordinary
  hook fixtures still inherit the five-second payload deadline and need bounded fixture inputs. The
  Hermes redirect fixture already consumes the complete request and keeps its socket until disconnect;
  commit `f3b5a21b` records that repair. Verify historical closure without rebuilding it. Keep production
  deadlines distinct from fixture ceilings and follow the repository's test-runtime policy. Explicitly
  include B105's approval-submission exit-code failure, historically `0` instead of `42` under load. The
  September 7 disposition leaves it unresolved after #383 and #441; #378 closed unmerged. A bounded
  fixture is not proof of closure, and this audit did not establish a current reproduction.

  DONE 2026-09-17, merged as [PR #770](https://github.com/webdavis/dotfiles/pull/770) at `74c867b8`. Two
  faces of one defect class, both fixed with evidence.

  FACE ONE, the sqlite fixture collision, and the root cause is TIME rather than concurrency. `state()`
  in both sqlite test helpers built its path from the process id plus a counter, nothing removes those
  directories, and macOS recycles process ids, so a later run of a recycled id rebuilds paths an earlier
  run already populated. Evidence: 39,150 leftover roots from only 495 distinct ids, and one run leaves
  160\. REPRODUCED EXACTLY by replaying one run's 160 databases into the next run's paths under a
  pid-controlled shell: 84 failures, 22 of them an already-exists refusal on the directory, and the
  sessions row answering `left: "one"` against `right: "arm posture alert"`, which is the sibling lane's
  report bit for bit including the failure count. Six more fixtures shared the shape, one of which failed
  live in the lane's first `just ship`. Eight files fixed with idioms the repository already had. One
  test file already carried this diagnosis verbatim in a comment, so the repository had diagnosed it once
  and never swept it.

  FACE TWO, B105. The hook-fixture half was ALREADY CLOSED by commit `e8f29551`, which raised the hook
  limit to fifteen seconds, and was not rebuilt. What was still open is the exit-code half: the two
  megabyte approval-payload rows injected no payload deadline, so they inherited the PRODUCTION five
  seconds and raced the read they only mean to observe. Losing that race is invisible, because the read
  answers nothing, the hook returns 0 having done nothing, and the row reports 0 where 42 was expected
  with no timeout named anywhere, which is B105's exact historical signature.

  B105 WAS NOT REPRODUCED SPONTANEOUSLY and is not claimed to be: 24 full-binary copies at load average
  349 produced zero failures, as did 55 loops of the payload filter. What was established is stronger
  than one reproduction. The at-cap sandbox lived 5615 to 9813 ms across 47 readings under that load,
  every one past the 5000 ms deadline its own read races, so the margin that hid the bug is gone. The
  coupling is proved by mutation in both directions. The fixture now states its own ten-second ceiling,
  ten times the worst megabyte read measured and still under the hook limit, so a pipe that really hangs
  stays bounded by production rather than by the test killing the child. The moshi submit leg was checked
  as a second possible coupling and is not one.

  OPERATOR OWES ONE DELETE: the code fix stops new collisions but removes no existing directory, so
  `just test-rust` can still fail on a clean tree until the stale roots are gone. Run
  `trash /var/folders/*/*/T/pns-sql-* /var/folders/*/*/T/pns-ledger-* /var/folders/*/*/T/posture-curation-*`
  by hand.

- [ ] 93. Implement `pns/docs/pns-refactor.md`, the agreed refactor plan the operator asked for alongside
  the numbered list. The plan merged as documentation in
  [PR #698](https://github.com/webdavis/dotfiles/pull/698) and nothing in it is built yet. Its changes:
  stop using the sender's name as a feature switch (delete the `event.agent != "claude"` check in
  `arm_nag.rs`), add a per-call `--remind` and `--no-remind` flag, add `[producers.<name>] remind` in
  config, resolve flag then config then a built-in default of off, warn when a reminder is armed for a
  producer that sends no answered signal, rename `--agent` to `--producer` with no alias and move every
  caller, and rename the config keys, environment variables and types its names table lists. Plan the
  pull request split from the document's numbered changes before building; the names table alone touches
  every workspace that calls pns. SLICED 2026-09-17 into
  `docs/superpowers/plans/2026-09-17-pns-refactor-slices.md`: FORTY-NINE pull requests in merge order, 22
  small and 27 medium, each with the callers it must update in the same change and the one behaviour a
  test must pin. The plan's 119 numbered items collapse to that count because four are settled twice by
  later items (79 by 118, 80 by 117, 88 by 116, 87 by 119), three are properties every slice follows
  rather than slices of their own (16, 33, 62), and one is already true (89). The slicing also corrects a
  path error in the plan itself: there is no `posture/crates/posture-producer-wire/`, and posture's copy
  of the wire contract is `posture/crates/posture-adapters/src/wire/` with its fixture at
  `posture/crates/posture-adapters/fixtures/request-v1.json`. Standing cost: every slice that changes
  config shape ships the parser change and the values change together and needs one operator apply,
  because `dot_config/pns/private_config.toml.tmpl` is generated by `just pns-config-render` and is
  itself a chezmoi target, so the deployed config lags the new binary until an apply. FOUR OPERATOR
  QUESTIONS block the ladder, all of them naming collisions inside the plan: (1) `[producer.<name>]` or
  `[producers.<name>]`, since item 3 writes it plural while item 110's rule makes a table keyed by one
  name singular, and slice 23 needs it first; (2) whether `max_age` is a fourth allowed time word beside
  `deadline`, `interval` and `delay`, given that `reading_max_age`, `desk_input_max_age` and
  `event_max_age` all keep `age`, and whether `PNS_PHONE_INPUT_AGE` becomes `PNS_PHONE_INPUT_MAX_AGE` to
  match, which slices 33, 40 and 44 need; (3) confirmation that item 94's four credential names reduce to
  `key` and `keys`; (4) whether item 105 splits `[lights] refresh_secs` into `arm_interval` and
  `fade_duration`. RULING 2026-09-17 on the credential name, answering slicing question 3 and unblocking
  slices 36 and 39: each plugin's credential key is named for the kind of secret THAT TOOL issues,
  spelled out in full (`device_token`, `bot_token`, `personal_access_token`, `api_key` for both the
  router and hue), and the authority is the KeePassXC entry, whose titles already state the type
  correctly per tool. This REVERSES the plan's item 94, which wanted `key` and `keys` everywhere: a
  config key spelled `key` beside a vault entry and a vendor document that both say personal access token
  makes the reader guess whether they are the same thing. If standardizing helps the Rust, the
  translation belongs in the code behind one internal type, never in the file a human reads, and only
  when it makes the code cleaner rather than as a rule applied for its own sake. `[plugins.github]` does
  NOT take item 90's `type = "<vendor>"` table shape: that shape is for delivery destinations and GitHub
  is a notification source. Two slicing questions remain: the two numbers `[lights] refresh_secs` splits
  into, which ships at 12 today and serves both the daemon re-arm interval and the fade budget.

  SLICE PROGRESS. The ladder is `docs/superpowers/plans/2026-09-17-pns-refactor-slices.md` and slices are
  taken in its order, one pull request each.

  SLICE 1 DONE 2026-09-17, [PR #758](https://github.com/webdavis/dotfiles/pull/758), merged `268e01fe`.
  The duration parser moved out of `pns/crates/pns-domain/src/quiet.rs` into a new `pns-domain::duration`
  module that takes the field name its refusals quote, the text, and the inclusive range that field
  allows. It returns a standard duration, refuses a bare number, and spells each range bound back in the
  largest unit that holds it whole, so a refusal reads in the units an operator would actually type.
  `quiet` keeps only its own policy bound, a new mute range of one second to twenty four hours, and both
  mute callers now call the one parser at that range. Five tests pin the units, both refusal shapes, the
  per-field range refusal, the range spelling, and that two different field names produce two differently
  named refusals. No config key or template moved, so no apply is owed for this slice.

  THE SLICE SHIPPED WITHOUT THE `ms` UNIT THE LADDER ASKED FOR, AND THAT IS A DEFECT IN THE PLAN RATHER
  THAN IN THE WORK. The ladder's behaviour line for slice 1 says a duration is accepted in `ms`, `s`, `m`
  and `h`. The review found that the parser's only caller today is the mute, whose range starts at one
  second, so every millisecond value the parser accepted was then refused by the range WHILE THE REFUSAL
  MESSAGE STILL ADVERTISED `ms` AS A LEGAL UNIT: a refusal contradicting the usage line printed directly
  beneath it. The unit was removed rather than left dead, and the parser's own tests stopped borrowing
  the mute's policy constant, which had coupled a generic parser to one caller's bound. CONSEQUENCE FOR
  THE LADDER, and the first later slice to hit it should read this first: plan item 40's unit list is the
  EVENTUAL set, not slice 1's acceptance, so whichever slice first introduces a field whose range reaches
  below one second has to re-add the unit and its millisecond spelling row. Slices 12, 22, 29, 30, 33,
  34, 35, 43 and 44 all parse a duration and are where that will surface.

  SLICE 2 DONE 2026-09-17, [PR #767](https://github.com/webdavis/dotfiles/pull/767), merged `43c50fc2`.
  The highest-risk rung in the ladder. `pns send` is the one sending subcommand: a form selector picks
  the envelope form on the json flag and the flag form otherwise, both reach ONE request path, the
  producer-argv classifier is deleted, the producer-flag predicate is private again, and argv naming no
  subcommand ends at a usage type that prints to stderr with exit 2 for anything but help.

  ALL SIX IN-REPO PRODUCER CALLERS MOVED WITH IT: the lights announcer plus its Rust and Python
  expectations, uu's weekly alert argv, the shell notifier's spawn, the failed-command text the failures
  command rebuilds, the skills-bootstrap failure notice in chezmoiscript 64, and the cutover gate's smoke
  send. posture's producer argv is CONFIG ONLY, so its config template and its own notify default carry
  the new verb rather than any Rust change. THREE OF THE LADDER'S LISTED FILES NEEDED NO CHANGE and the
  lane confirmed why rather than editing them, which corrects the ladder's own file list: the submit path
  already takes the args slice it is handed, the daemon child spawner spawns only subcommand argvs and
  never the bare producer form, and posture's wire module holds document shapes rather than an argv.

  THE APPLY-ATOMICITY WORRY IS ANSWERED GREEN, AND PROVED RATHER THAN ASSUMED. Once the bare form is
  refused, a caller left on the old argv stops notifying SILENTLY, so the whole set must move in one
  apply. All four apply-time builders glob and hash every file at every extension in their own workspace,
  so a source-only edit moves the trigger; rendering each builder in the worktree and in the main
  checkout and diffing the hash comments showed every changed file present as a hashed build input with a
  changed digest. NO CALLER ESCAPES A FULL APPLY, and a partial or by-name apply would still leave one
  broken, so this slice's apply must be a full one.

  THE CROSS-CHECK WALK was done in full and every hit accounted for. One was a live producer call and
  changed, the cutover gate's smoke notification; the rest use a surviving subcommand spelling, are
  binary-path constants with no argv, or are prose and historical specs.

  `pns.nvim` WAS ALSO FIXED, in its own repository, and the operator authorized that afterward: it sent
  bare argv and would have broken. `webdavis/pns.nvim#2`, merge `4e52741a`, prepends the verb and bumps
  its default minimum engine version across 38 tests, and the commit pin and minimum version in
  `dot_config/nvim/lua/plugins/pns.lua` moved to match. OPERATOR RULING 2026-09-17, given when this was
  surfaced: a lane may change any repository the operator owns, `pns.nvim` and the herdr plugins
  included, so a caller in a sibling product is fixed there rather than left broken.

  SIX OF FORTY-NINE SLICES MERGED as of 2026-09-17. Slice 1 promoted one duration parser out of the quiet
  module. Slice 2 made `pns send` the one sending subcommand and proved apply atomicity rather than
  assuming it. Slices 3, 4, 5 and 6 are recorded below.

  SLICE 3, `--producer` and `PNS_PRODUCER` replace `--agent` and `PNS_AGENT`, merged as
  [PR #772](https://github.com/webdavis/dotfiles/pull/772) at `b715698f`. `pns send --producer <name>`
  names the sender, `--agent` is refused with exit 2 and a message naming the replacement, and the hook
  path reads the new variable. WHY REFUSAL RATHER THAN A SILENT SKIP, which the slice found rather than
  assumed: the parser's existing leniency would have read `--agent codex` as two stray words and sent the
  event under the DEFAULT producer, so the retired flag takes its value with it and hands the refusal on.
  Every caller moved in the same change, including `scripts/cutover-gate.sh`, WHICH THE SLICE PLAN DID
  NOT LIST. lights and uu changed only the argv strings they build, so neither gained a dependency on
  pns. The Codex installer's migration list gained the retired spelling at both engine paths, so an
  existing row is rewritten in place, and the migration test pins that as its own case.

  SLICE 4, `pns tap info` and `pns tap install` replace the two flags, merged as
  [PR #773](https://github.com/webdavis/dotfiles/pull/773) at `8bdd805c`. Each retired flag spelling is
  refused with exit 2 and a message naming its verb. Bare `pns tap` is unchanged. The verb must be the
  first word after `tap`, which keeps the parser one pass and matches how `pns recap agent` and
  `pns presence poll` read theirs. A LIVE DEFECT THE SLICE FOUND AND DID NOT HIDE:
  `pns/docs/pns-tap-apple-shortcut.md` records the phone Shortcut's Comment field verbatim, and that
  shipped Comment still names the retired spelling, so the document was left holding the true text and
  its surrounding paragraph now names the defect.

  SLICE 6, remove `pns gate` and fold `pns home` into `pns doctor`, merged as
  [PR #775](https://github.com/webdavis/dotfiles/pull/775) at `fa0735b4`. The bare `pns <harness>-hook`
  is now the only spelling, and the dispatcher routes every hook-shaped word straight to the gate rather
  than keeping its own copy of the shape test. THE SILENT-EXIT BUG IS FIXED, and it was a real bug rather
  than a rename: the gate's refusal used to be an exit 0 in silence, so a hook word it would not vouch
  for looked wired when it was not; it now exits 2 with a sentence naming the word and what the gate
  accepts, and exit 0 survives only where the gate genuinely declines. The home reading returns doctor
  rows instead of painting, and its stale-identifier alert trigger moved with it so there is still one
  memory and one decision. The one caller was VERIFIED rather than assumed: `run_after_62` writes the
  bare binary pathname with no subcommand. The plan's file list missed eleven further references,
  including CLAUDE.md, seven spec files and a config renderer whose change meant regenerating the shipped
  template.

  SLICE 8, `pns shell end --exit-code` replaces `--exit`, merged as
  [PR #777](https://github.com/webdavis/dotfiles/pull/777) at `da1a5466`. The retired flag is matched as
  a KEY AND VALUE PAIR, so it takes its value with it rather than leaving the status at a default, which
  is the lesson slice 3 recorded. It is refused for both verbs, not only `end`, because naming the
  replacement beats "unknown argument" for a caller who typed it on `begin`. Both `dot_bashrc.tmpl` call
  sites moved in the same change, the exit trap and the precmd function, along with the usage text and
  every test. A repository-wide grep found ONE place the plan did not list, a prose line in the vendored
  clean-code Rust skill example, corrected in the same commit. The rendered bashrc was checked to prove
  the new flag reaches the deployed shell, with zero occurrences of the old spelling.

  SLICE 5, `pns failures open` and `pns lights pulse` replace `pns click` and `pns pulse`, merged as
  [PR #776](https://github.com/webdavis/dotfiles/pull/776) at `4f12ab84`. Both files fold into their new
  homes rather than staying as thin wrappers, and neither old word is an alias: each is refused with exit
  2 and a two-line message naming the new spelling plus that subcommand's usage. THE ONE CALLER IS A
  STORED STRING: the banner's click command is composed from the running binary and stored in config, so
  it now names the open verb. A banner ALREADY ON SCREEN when this lands carries the old command and its
  click is dead until it is dismissed; new banners are correct. No pulse was ever run against the real
  bridge, which would be refused now anyway, since the transport requires the pinned certificate.

  OPERATOR OWES: one full `chezmoi apply` per config-shape slice, and slices 3, 4 and 6 each changed
  deployed behaviour, so one apply covering all three is what makes the deployed binary and the deployed
  config agree.

  SLICE 7, every subcommand answers `--help` and `-h` with its own usage, merged as
  [PR #779](https://github.com/webdavis/dotfiles/pull/779) at `04c2f322`. Every subcommand answers
  `--help` and `-h` with its OWN usage on standard output and exit 0, routed in the composition root
  before any subcommand parser sees the arguments. A new catalog module pairs each subcommand path with
  the usage text its own command file owns, sixteen subcommands plus two verb-level paths whose narrower
  text wins. Three commands had no usage string at all and got one. The tool-wide listing was rewritten
  to name every subcommand, with the producer flags split into their own text, and a trailing paragraph
  for the machine-called ones.

  HELP RECOGNITION IS POSITIONAL, and that is a deliberate decision: slot 0 of the tail, or slot 1 when
  slot 0 is a bare verb. A flag's value always follows its flag, so this can never mistake a detail text
  reading `--help` for a question, which is the rule the producer parser already pins. A tail-wide scan
  would have flipped it. Exit 0 for help is kept apart from the exit 2 refusals slices 3 to 8 built, and
  both are pinned. The plan named six machine-called subcommands to list; grepping the repository found
  FIVE MORE it missed.

  A RULE BREAK, RECORDED RATHER THAN EXCUSED: this lane's ship stage used `--no-verify` on one commit,
  caught itself, undid it with a soft reset and recommitted through the pre-commit hook. No bypassed
  commit reached main and every gate ran, but `--no-verify` is forbidden outright.

  SLICE 9, `--route` replaces `--channel`, merged as
  [PR #780](https://github.com/webdavis/dotfiles/pull/780) at `95b5ad38`. Plan item 20. `--route` is now
  the flag that names a hermes route, and `--channel` is refused. `--channel` joined `RETIRED_FLAGS`
  beside `--agent`, carrying the same replacement message and exit 2, and it is matched as a key and
  value pair so its value cannot leak into the positional arguments; a test pins that consumption in both
  the with-value and without-value forms, and it was mutation-checked by removing the guard and watching
  the state arrive empty. `--route` took its slot in `VALUE_FLAGS`, the unusable-name warning in
  `channel_dispatch.rs` names `--route`, and the rebuilt failure-record command text says `--route` while
  the pns-domain fixture deliberately keeps the old spelling so an old stored record still renders. No
  callers had to move: a repository-wide grep for `--channel` found nothing outside pns's own source, its
  documentation and the dated superpowers specs, and uu's
  `an_alert_names_no_route_channel_or_gateway_of_any_kind` was read and confirmed to forbid both
  spellings. `EventArgs.channel`, `PNS_CHANNELS_DIR` and every Discord channel id keep the word channel,
  which the plan's own rule reserves for a Discord channel id; renaming the internal field is left as its
  own slice. Nine of the forty-nine slices are now merged.

  SLICE 10, `state` becomes one closed set and `Signal` is deleted, merged as
  [PR #782](https://github.com/webdavis/dotfiles/pull/782) at `5800121c`. Plan items 17 and 18. `state`
  is now ONE CLOSED SET of six words, `done`, `failed`, `blocked`, `resolved`, `observation` and
  `progress`, and it is the same set on the flag path and the JSON path. The `Signal` wrapper left
  pns-protocol, so a state arrives as a plain word, and `--state` refuses anything outside the set with
  exit 2 in the form the earlier refusal slices established. `observation` and `progress` are quiet
  updates on both paths through one `Attempt::of_state`, which is the behaviour change the slice existed
  for: an observation used to be an ordinary message.

  Both golden fixtures moved together, pns-protocol's and posture-adapters', which is what holds the two
  copies of the wire contract honest. posture's wire copy stayed posture's own, with no engine name in
  it.

  The apply-time caller moved with the slice, which was the whole risk: the skills bootstrap script sent
  `--state first-install-failed`, a word that stops being legal, so leaving it behind would have turned
  an apply-time failure notice into a refusal. It now sends `failed` with the rest of the meaning in its
  detail, and the rewritten template was rendered and shellchecked to prove it.

  Two review findings were fixed: an internal state word was leaking into a replay command, and a
  trailing `--state` with nothing after it was not refused the same way an empty one was. Ten of the
  forty-nine slices are now merged.

  SLICE 11, the request envelope flattens and `elapsed_secs` becomes `elapsed`, merged as
  [PR #783](https://github.com/webdavis/dotfiles/pull/783) at `7e0875f8`. Plan items 19, 21, 23 and 24.
  The version 1 request now carries `project`, `branch`, `pane` and `session` at the JSON TOP LEVEL, with
  the `Context` and `Session` structs deleted and the unread session turn count going with the wrapper.
  `elapsed_secs` became `elapsed`, a duration written as a count plus a unit, refused as a bare number
  with exit 2 on the JSON path, on `pns send --elapsed` and on `pns shell end --elapsed`. `--request-id`
  and `--session` joined the flag path, each held to the same identifier rules the envelope holds its
  JSON twin to, and neither is required. Both golden fixtures moved together, pns and posture.

  Two review findings were fixed: a zero elapsed was being spelled in milliseconds, which the decoder
  refuses, and a caller-named request id was being filed under the wrong producer.

  The lane reported its mutation count honestly rather than to the brief: teaching the duration parser to
  read a unit-less count as seconds reddens five tests, not the one the brief predicted.

  A CALLER LIVED IN ANOTHER REPOSITORY, as it has in several slices now. `webdavis/pns.nvim` spawns the
  producer argv and passed both `--agent` (retired by slice 3) and a bare elapsed count. The lane fixed
  it there and committed, but left the commit UNPUSHED on a branch whose base was two behind, so the
  orchestrator finished it by hand: merged `origin/main`, resolved the argv conflict (main had added the
  `send` subcommand pns 0.2.0 requires, this branch renamed the flag and made the duration, and all three
  are needed together), ran the plugin's own gates, and merged it as pns.nvim PR #3. Its specs assert the
  argv position by position, so the index shift the extra subcommand word causes is pinned rather than
  assumed.

  Eleven of the forty-nine slices are now merged.

  SLICE 11'S APPLY IS PAIRED WITH SLICE 8'S: the deployed `~/.bashrc` passes a bare elapsed count until
  the operator applies, so between this merge and that apply every long-running command notification
  would be refused. The plan says to pair the two applies and the morning list carries them together.

  A STRAY MERGE-CONFLICT MARKER sat inside `docs/superpowers/plans/2026-09-17-pns-refactor-slices.md`'s
  Operator rulings section, a lone `||||||| <sha>` line with no opening or closing marker beside it,
  found and removed 2026-09-17. Checked before removing: the referenced commit does not contain the file
  at all, so the marker is the diff3 middle line from a merge where the file was new on both sides, and
  the resolver deleted the outer markers and missed this one. All four ruling bullets were present and
  distinct, so nothing was lost. The hazard was that every lane reads that section as binding.

  SLICE 12 DONE 2026-09-18, [PR #786](https://github.com/webdavis/dotfiles/pull/786), merged `3482daaa`.
  Slice 12 of the pns refactor ladder replaced the narrowing pair --local-only and --remote-only with one
  --scope flag taking automatic, local_only or remote_only, the three words the JSON request's scope
  field already used. pns_domain::DeliveryScope gained from_word and WORDS beside Kind's, and --scope
  parses in its own arm, so a word outside the three, or a --scope with no value, refuses with exit 2
  naming the three rather than falling back to automatic and sending off the machine what a caller meant
  to keep on it. Each retired spelling is refused naming --scope. The refusal that existed only to catch
  both flags being given together went with them, along with its two tests, decision 0007's accepted
  status and the Refusal enum that carried it, because one flag cannot contradict itself. Neither retired
  flag took a value, so RETIRED_FLAGS now records which ones do: --channel still takes its value with it,
  while --local-only --help still prints the usage instead of eating the help as a value. lights was the
  one caller and moved in the same commit with its unit test and the argument-surface expectation that
  pins its argv; a sweep of the checkout, of pns.nvim and of the three herdr plugin trees found no other
  caller. just test-rust and just ship both passed, and a mutation that let an unknown scope fall through
  to the default reddened three specs.

  SLICE 13 DONE 2026-09-19, [PR #796](https://github.com/webdavis/dotfiles/pull/796), merged `5481a477`.
  Slice 13 of the pns refactor removed the request fields that changed nothing: `event`, `occurred_at`,
  `interaction` and `session.turn` from the pns-protocol request struct and its golden fixture, and
  `--long-running` from pns's legacy CLI (now derived from `--elapsed` alone), with posture's producer
  and wire crates and fixture mirrored to match. The shell notifier stopped precomputing the long-running
  tier in-process and instead passes `--elapsed` to the spawned `pns send`, which derives it itself the
  same way every other producer does. A batch of test files across pns-protocol, pns and posture that
  still asserted on the retired wire fields (a golden encoded-bytes literal, a required-field list, a
  detail-format ordering, and several posture alert tests keyed on the removed `event` string) were
  updated to match the corrected wire shape, distinguishing gap versus exposure alerts by their body text
  instead. `just test-rust` and `just lint-check` both pass clean from the worktree root.

  SLICE 14 DONE 2026-09-19, [PR #799](https://github.com/webdavis/dotfiles/pull/799), merged `0eae47ce`.
  Pns slice 14 merged `kind` and `class` into one request field, `delivery_class`, spelled
  `--delivery-class <name>` on the command line and `"delivery_class": "<name>"` in a version 1 request.
  The two fields had been one idea wearing two names: the flag picked the route an event took when its
  producer named none, while the JSON-only `class` decided whether a message passed a mute, and only the
  JSON half could carry posture's `security`. The merged field takes a validated name on both paths, so a
  producer stating it in JSON and one typing the flag reach the same route. `pns_protocol::Kind` and
  `pns_domain::routes::Kind` are gone, replaced by a `routes::HEALTH` constant and a `routes::route_for`
  function, because `health` is the one class pns routes for itself and the rest are the operator's to
  define. `EventArgs` carries the class as a plain word now, which also collapsed a duplicate source: the
  mute-bypass check reads it off the event rather than off a second copy on the producer request.
  `--kind` is a retired flag, refused with exit 2 and a sentence naming `--delivery-class`, and a JSON
  `kind` or `class` is reported in the ignored-fields list the way slice 13's removed fields are. Callers
  moved in the same change: uu's weekly alert, posture's producer request and its copy of the wire
  contract, both golden fixtures, the generated pns config template, the routing runbook and four specs.
  uu and pns are rebuilt by the same apply, so the two binaries move together.

  SLICE 15 DONE 2026-09-19, [PR #802](https://github.com/webdavis/dotfiles/pull/802), merged `03ff31be`.
  Task 93 (pns refactor slice 15) landed the `[delivery_class.<name>]` tables. Which delivery classes
  exist, where each one routes and which of them cross a mute moved out of pns and into config: each
  table carries `route` (empty is the default route) and `bypass_mute`, parsed by a new
  `pns/crates/pns-adapters/src/config/delivery_class.rs` and rendered by a hardcoded branch beside the
  lamp declarations. `[delivery] bypass_silence_classes` left the roster, and `config/delivery.rs` now
  exists to refuse a leftover `[delivery]` key by name so an operator still carrying the retired one is
  told. `pns-domain/src/routes.rs` lost the compiled words `agent` and `health`; `route_for` takes the
  route the class named and keeps only the severity rule that a class routes of its own while the state
  is one somebody waits on, and `EventArgs::routed` takes that route rather than the `[routes]` pair. The
  one class word pns still writes is `stale::DELIVERY_CLASS`, where pns names its own page the way uu and
  posture name theirs. A `delivery_class` naming no configured table is refused with exit 2 and named, on
  stderr and in the reply's diagnostics, on both the argv and JSON paths; a message naming no class reads
  `[delivery_class.default]`; and the retired JSON `class` and `kind` fields are refused by name rather
  than listed as ignored. The shipped values file gained `default`, `health` (route `priority`) and
  `security` (`bypass_mute = true`), both keys written at their default on every class, and the config
  template and resolved-configuration snapshot were regenerated from it. `just test-rust` and
  `just lint-check` both exit 0 on the branch merged with main. The apply window is the risk this slice
  carries: until a full apply runs, the deployed config has no class tables, so uu's health page and
  posture's security page are refused rather than routed.

  SLICE 22 DONE 2026-09-19, [PR #798](https://github.com/webdavis/dotfiles/pull/798), merged `b26f79c1`.
  Slice 22 of the pns refactor ladder landed the reminder rename and the table split. The approval nudge
  is called the reminder everywhere now: `pns nag` became `pns remind`, the old word is refused with a
  sentence naming the new one and exit 2, and every module, type, constant, on-disk name (the `remind/`
  record directory, `remind-<session>` markers, `remind:<session>` job ids) and the `remind=`
  decision-log field followed across all four pns crates. The single `[nag]` table, which held two
  unrelated features, became `[remind] delay` for the local nudge about an unanswered approval and
  `[stale] escalate_after` for the page about a session stuck past its window, with a new `[stale] route`
  naming where that page goes (unset still leaves the health kind to resolve it against
  `[routes] urgent`, so shipped behaviour is unchanged). Both keys take a duration string rather than a
  count of seconds, read through the domain's one parser behind a new shared `duration_key` helper in
  `schema.rs`; `"0s"` remains the feature off at either key, which plan item 101 will take up later.
  `[nag]` is refused at load as an unknown top-level table and the refusal lists `remind` and `stale`
  among the tables the file serves. The shipped config was regenerated with `just pns-config-render`, the
  specs and the Claude Code hook comment follow the new names, and `just test-rust` and `just lint-check`
  both pass. Tasks 42, 35 and 100 of `pns/docs/pns-refactor.md` are done, as is the
  `nag.stale_after_secs` third of task 99.

  SLICE 23 DONE 2026-09-19, [PR #801](https://github.com/webdavis/dotfiles/pull/801), merged `734bdc14`.
  Pns slice 23 landed. The approval reminder no longer arms itself because the sending producer happens
  to be called "claude": the `event.agent != CLAUDE_AGENT` gate and the constant behind it are gone from
  `ArmRemind`, which now takes an already-resolved delay and reads zero as off. A call switches the
  reminder on for itself with `--remind`, `--remind=<duration>` or `--no-remind` on its own hook
  invocation, and a producer nobody can pass a flag to is served by the new `[producer.<name>] remind`
  table. The hook path resolves the two most specific first, the call's switch beating the producer's
  entry beating a built-in default of off, and `--remind` with no delay anywhere is refused with exit 2
  that names both fixes rather than guessing a delay. The shipped config template was regenerated and the
  specs for reminding, blocking approvals and producer submission were brought to the new flags and
  table. The behavioural cost is stated and accepted: nothing passes `--remind` yet, so the Claude Code
  approval reminder stops arming until slice 25 moves the harness declarations, and task 93 is that
  slice. Gates: `just test-rust` and `just lint-check` both exit 0.

  SLICE 24 DONE 2026-09-19, [PR #803](https://github.com/webdavis/dotfiles/pull/803), merged `f0109634`.
  Pns slice 24 landed the reminder's honesty line. Arming a reminder for a producer that sends no
  answered signal now writes exactly one line to stderr and not a byte to stdout, which is what keeps
  Claude Code's reading of the hook's stdout intact, and the reminder is armed anyway with the `[remind]`
  staleness cap left as the hard stop. Which producers answer is derived from how the reminder was armed
  rather than from a new config key or a compiled-in roster of names: a harness wires `--remind` on its
  own approval hook only when it also wires the answered event, so the switch is the assertion, and
  `[producer.<name>] remind`, which exists for a producer nobody can pass a flag to, carries none.
  `remind_delay` answers with the delay and that assertion together, and both travel to `ArmRemind`. Two
  process-level tests with the real binary pin the stderr line, the empty stdout, the silent `--remind`
  path and the cap ending both arms; `just test-rust` and `just lint-check` are green.

  SLICE 29 DONE 2026-09-19, [PR #794](https://github.com/webdavis/dotfiles/pull/794), merged `f09af878`.
  Pns's point-in-time flags were renamed to say epoch: pns recap --since/--until became
  --since-epoch/--until-epoch, and pns daemon schedule --until was split so the relative +<duration> form
  stayed on --until while the absolute form moved to a new --until-epoch flag, so a point in time can no
  longer be typed where the parser expects a duration. The one caller, recap_child::spawn_recap, and
  every usage string and live spec under pns/docs/specs were updated to match, with a unit test pinning
  that the old bare spellings are refused as unknown input and that the new flags parse an epoch.

  SLICE 30 DONE 2026-09-19, [PR #792](https://github.com/webdavis/dotfiles/pull/792), merged `4481ed4c`.
  Slice 30 of the pns refactor gave every environment variable pns owns the PNS\_ prefix and spelled its
  words out: MOSHI_HOOK_BIN became PNS_MOSHI_HOOK_BIN, CODEX_BIN became PNS_CODEX_BIN, PNS_IDLE_SECS
  became PNS_SCREEN_IDLE, and PNS_DESK_IDLE_SECS became PNS_DESK_IDLE. Every reader and every test caller
  moved in the same change, pns/docs/specs and two dated design records were updated to match, and new
  tests pin that each old name is now ignored. A repo-wide grep confirmed no deployed file exports any of
  the four old names, matching the slice's own risk note; the one coincidentally-named MOSHI_HOOK_BIN in
  the moshi-hook bounce chezmoi script is that script's own unrelated variable and was left alone.

  SLICE 31 DONE 2026-09-19, [PR #797](https://github.com/webdavis/dotfiles/pull/797), merged `9ef9ce3e`.
  Pns refactor slice 31 deleted the four environment variables that duplicated a config key.
  PNS_PHONE_MARKER_FILE, HUE_PULSE_ROOMS, PNS_MOSHI_SUBMIT_DEADLINE_MS and PNS_PULSE_THRESHOLD_SECS are
  gone; config ([phone] marker_file, [plugins.hue] rooms, [plugins.mobile] submit_deadline_secs,
  [lights.loop] threshold_secs) is now the only source for each. pulse_threshold_secs previously never
  read the config key at all, only the deleted env var with a hardcoded fallback; it now loads
  [lights.loop] threshold_secs the same way the loop lamp does. Every reader, every test env setter and
  the render layout's precedence prose moved with it, one pin test per deleted variable was added, and
  the seven affected pns/docs/specs files were updated. A grep of the deployed tree found nothing
  exporting any of the four names, matching the slice's own risk note: a stale shell-profile export now
  silently reverts to the config value rather than winning, which was the intent.

  SLICE 32 DONE 2026-09-19, [PR #800](https://github.com/webdavis/dotfiles/pull/800), merged `53cc5b27`.
  Pns refactor slice 32 landed. Each of the six install-wide settings that had only an environment
  variable now has a config key and reads the file first: `[paths] state_dir` ahead of `PNS_STATE_DIR`,
  `[paths] channels_dir` ahead of `PNS_CHANNELS_DIR`, `[plugins.hermes] url` ahead of `PNS_HERMES_URL`,
  `[plugins.mobile] url` ahead of `PNS_MOSHI_URL`, `[plugins.macos-banner] terminal_bundle_id` ahead of
  `PNS_TERMINAL_BUNDLE_ID`, and `[delivery] remote_deadline`, which retired `PNS_REMOTE_TIMEOUT` outright
  because a delivery bound belongs beside `max_attempts`. One resolver, `install_settings`, answers all
  six off a single config load, with an empty value on either side naming nothing; `state_dir()` caches
  its answer for the process, since the parse measured 3.6 ms against the deployed file and a blocked
  hook path reads it several times. Two readers that had gone their own way were brought back to it: the
  recap child stopped injecting a deadline variable into itself, leaving the 30-second group watchdog it
  already arms to bound that process, and `pns failures` stopped reading two of the variables directly
  for its address line. Unit tests pin every setting in both directions and pin that the retired variable
  changes nothing, the shipped config template was regenerated, and twelve spec files were restated.

  SLICE 33 DONE 2026-09-19, [PR #804](https://github.com/webdavis/dotfiles/pull/804), merged `8b24cc36`.
  Slice 33 of the pns refactor ladder landed, plan items 84 and 85. Every duration environment variable
  that survives on main now reads the same `<count><ms|s|m|h>` string as pns's flags and config keys,
  through the domain's one duration parser, with a range of its own and a refusal that quotes the
  variable's own name and the shapes it accepts. The parser learned `ms` to make that possible, trying it
  ahead of `s` so the second's suffix cannot claim the tail of a millisecond value. The names now use one
  word per kind of knob: PNS_PAYLOAD_DEADLINE_MS, PNS_MOSHI_JSON_DEADLINE_MS,
  PNS_MOSHI_STATUS_DEADLINE_MS and PNS_CONDENSER_DEADLINE_MS dropped their unit suffix,
  PNS_DAEMON_TICK_MS became PNS_DAEMON_TICK_INTERVAL, PNS_PHONE_INPUT_AGE became PNS_PHONE_INPUT_MAX_AGE
  under the operator ruling that makes max_age a fourth time word, and PNS_REPLY_REREAD_INTERVAL kept its
  name and stopped parsing float seconds. A value the parser refuses is reported and the caller keeps its
  default, because every one of these is read on a path whose contract is exiting 0. Nothing in the
  deployed tree set any of them, so no caller moved. PNS_DB_BUSY_TIMEOUT_MS and
  PNS_RING_LOCK_TEST_DELAY_MS were left for slice 34, which trades one for a config key and puts the
  other behind cfg(test). The spec set and the environment-variable table were updated to match.

  The gateway daemon's `pns gateway start|stop|restart|status` verbs merged as
  [PR #795](https://github.com/webdavis/dotfiles/pull/795) at `c1733362`, giving it the same service-verb
  shape as pns's other subcommands.

  SLICE 16 DONE 2026-09-19, [PR #808](https://github.com/webdavis/dotfiles/pull/808), merged `3a018b968`.
  Pns's result status now reports delivery rather than storage. `Status` became `delivered`, `partial`,
  `undelivered` and `rejected`, derived in the submission receipt from what each destination actually
  did, and the ledger's commit state moved to the `ledger_committed` and `ledger_unavailable` diagnostics
  beside it, so a committed row whose every destination failed reads as `undelivered` instead of the
  `accepted` it used to claim. Both send paths map that answer to one set of exit codes, 0 when every
  durable destination took the page, 1 when one did not, and 2 for input pns will not honour, and the
  `--require-delivery` opt-in was retired as a refused flag that names the exit code that replaced it. A
  silent destination counts as an arrival because it is the ordinary success of an executable channel,
  and a decorative destination (the banner, the phone card) does not decide the status on either path,
  its verdict staying in the destinations list. The harness hook paths keep their always-exit-0 contract,
  pinned by a test. posture and uu moved in the same change: posture accepts only a `delivered` result
  carrying `ledger_committed`, and uu still fails open on an exit code it now hears on every call. The
  protocol, producer-submission, routing-and-delivery and legacy-producer-flags specs and posture's
  producer-api document were rewritten to the new words.

  SLICE 17 DONE 2026-09-20, [PR #822](https://github.com/webdavis/dotfiles/pull/822), merged `a9ebe2d6e`.
  Slice 17 of the pns refactor ladder renamed the version 1 result envelope's fields and finished the
  bare-string rule for closed sets. A result now carries `ledger_sequence` where it carried
  `decision_id`, which stopped that value sharing a word with pns's unrelated decisions table keyed by
  producer and request id, and each destination outcome names itself in `name` rather than in
  `destination`, which stuttered with its own array. The hardcoded `interaction` field left the result:
  the receipt set none and the submit path set `no_opinion` whenever a request asked, so `answered` was
  never constructed outside tests, and the request-side field it answered was already retired. Deleting
  its `InteractionResult` type removed the last one-key wrapper object on either envelope, so every
  closed set is now one bare word and both decoders refuse a wrapped word instead of guessing. posture
  moved in the same change, since it is the only consumer: its wire reader, its wire tests, its golden
  fixture and the one fixture command that prints a result document. Both golden fixtures were edited
  together so the two sides stay pinned to the same bytes, and posture refuses a destination still naming
  itself with the retired field, so a stale producer fails loudly rather than reading as delivered.
  `pns/docs/specs/protocol-v1.md` and `posture/docs/producer-api.md` were updated to match.
  `pns/crates/pns-protocol/src/request.rs` needed nothing: slice 10 had already left it free of wrappers.
  `just test-rust` and `just lint-check` both exited 0.

  SLICE 18 DONE 2026-09-20, [PR #829](https://github.com/webdavis/dotfiles/pull/829), merged `0934f9d46`.
  Pns refactor slice 18 landed plan items 75 and 76. An unrecognized top-level field on a version 1
  request used to be reported by pushing the marker string `ignored_fields` into the result's diagnostics
  and appending the field names after it, so a reader had to know that one diagnostic changed the meaning
  of every entry beside it. The names now travel in an `ignored_fields` list of their own on the result
  envelope, empty when the request carried none, bounded at the same item cap the diagnostics obey. The
  golden result fixture was corrected with it: it spelled one ignored field as the single diagnostic
  `ignored_field:detial`, singular and colon-joined, a shape pns never emitted, and it now carries the
  `ledger_committed` diagnostic a committed row really answers with beside `ignored_fields: ["detial"]`.
  posture's copy of the same golden moved with it and posture's wire reader gained the field, because
  posture re-encodes that document field for field in its own test and would otherwise have dropped the
  key. Four new tests pin the behaviour (a named field in the list with nothing about it in diagnostics,
  an empty list when there is none, the fixture round trip, and the item-cap bound), and protocol-v1.md
  was updated at S017, S020 and the submission section. Task 93 stays open: the recap route work it names
  is untouched by this slice.

  SLICE 19 DONE 2026-09-20, [PR #830](https://github.com/webdavis/dotfiles/pull/830), merged `0ff030972`.
  Pns refactor slice 19 filled the destination outcome the protocol had left half empty. Every entry in a
  result's `destinations` array now carries `note`, the sentence the destination itself offered about a
  leg it did not deliver, which pns already had and threw away; `route`, the named route the leg was
  submitted on, so a producer that posted to `priority` can see which destination took it there; and
  `retry_at`, the unix second the ledger will try the leg again, absent when it will not, which is how a
  producer tells a leg pns is still retrying from one it has given up on. Each of the three is omitted
  rather than written as null when it does not apply. The outcome word gained `unknown`, splitting the
  replay path's meaning of "the ledger never learned the answer" out of `silent`, which keeps its one
  meaning of a channel that ran and said nothing; an unknown leg still counts with the arrivals when the
  status is computed, because its attempt is unresolved rather than proven to have missed. The rule that
  the event's own text never comes back is unchanged and now has a test of its own on the replay path,
  where the event is in reach: `note` carries the destination's sentence and never the event's detail.
  posture moved in the same change so both sides pin the same bytes, its wire `DeliveryOutcome` gaining
  `unknown` and its `DestinationOutcome` gaining `route` and `retry_at`, and both golden fixtures now
  carry a failed hermes leg with all three new fields beside a banner leg that carries none of them. Plan
  items 70 through 73, with `pns/docs/specs/protocol-v1.md` and `posture/docs/producer-api.md` updated to
  match (task 93).

  SLICE 25 DONE 2026-09-19, [PR #810](https://github.com/webdavis/dotfiles/pull/810), merged `14520233e`.
  Task 93 wired the waiting and answered pair on every harness, which is what restores the approval
  reminder that stopped arming when slice 23 made arming explicit. Claude Code's `PermissionRequest`
  declaration in `private_dot_claude/modify_settings.json` passes `--remind`, so the reminder arms again
  and the existing `pns hook resolved` declarations clear it. The Codex installer at
  `dot_local/libexec/pns/hooks/codex/executable_install-hooks.sh` gained its answered signal on the two
  events that end a wait, `PostToolUse` and `Interrupt`, both pointing at `pns hook resolved` beside the
  `Stop` and `PermissionRequest` rows it already wrote, reusing the same migration that preserves rows it
  did not write and collapses duplicates it did. `private_dot_hermes/modify_private_config.yaml` gained a
  `hooks` block wiring `pre_approval_request` to `pns hook blocked --remind` and `post_approval_response`
  to `pns hook resolved`, each list written whole like the routes map beside it, with the pns path built
  from the one `rust_tools` declaration and no producer prefix because hermes runs a hook command through
  shlex.split with no shell. The event names were taken from hermes's own documentation in the checkout
  at `~/.hermes/hermes-agent`, which lists both among VALID_HOOKS, and from `hermes hooks --help`; no
  hermes command that sends anything was run. Both files are modify-templates over files their apps
  rewrite themselves, so both were proven to keep reading the live file back: the Claude one was rendered
  headless over a sample live settings.json and preserved an undeclared live key while emitting the flag,
  and the hermes one was rendered with its vault reads stubbed and shown to leave a hermes-owned
  `pre_tool_call` hook alone and to reproduce its stdin byte for byte on a no-op.
  `test/unit/pns-codex-hook-migration.test.sh` grew the two new rows plus a twice-run idempotence check,
  and `test/unit/hermes-config-modify-template.test.sh` grew the approval pair alongside a hermes-owned
  sibling event. The risk is that both templates sit over app-owned files, and one full `chezmoi apply`
  is the operator step that makes the reminder arm again; Codex and hermes each also need a one-time
  trust or consent action before their new rows run. Dated 2026-09-17.

  SLICE 26 DONE 2026-09-19, [PR #818](https://github.com/webdavis/dotfiles/pull/818), merged `a8f7ea894`.
  Task 93 (slice 26, `remind` in the JSON) landed. Version 1 of the pns request envelope gained an
  optional `remind` field that is either a boolean or a duration string: `true` uses the configured
  delay, `"5m"` sets the delay for that request, `false` matches `--no-remind`, and absent still falls
  through to `[producer.<name>] remind` and then to off. The switch type moved into pns-protocol so the
  flag path and the JSON path share one value and one resolution: `remind_switch` builds it from argv,
  the decoder builds it from JSON, and `remind_delay` now takes that switch rather than argv, which is
  what keeps precedence, config reading and refusals from drifting apart. The delay's range moved to
  pns-domain, so the config key, the flag and the JSON field are held to one bound of thirty seconds to
  an hour. A value that is neither a boolean nor a valid duration is refused before effects with a
  sentence naming the field, an absent field is omitted when encoding so unmarked requests keep their
  canonical bytes, and the golden fixture gained the field. No in-tree producer sends it, so no caller
  moved, and posture's copy of the wire contract was left alone because the field is optional and posture
  never sends it. The submit path resolves the field but does not arm a reminder from it yet.
  protocol-v1.md, producer-submission.md, configuration.md and reminding.md were updated.

  SLICE 28 DONE 2026-09-19, [PR #811](https://github.com/webdavis/dotfiles/pull/811), merged `3006dd259`.
  Done 2026-09-17. `pns quiet` became `pns mute` and `pns lights quiet` became `pns lights mute`,
  aligning the command with the `bypass_mute` key the delivery-class tables already carried. Both old
  words are refused with exit 2 and a sentence naming the replacement, following the same retired-word
  pattern `pns pulse` and `pns click` already use, because a mute an operator believes is on is the
  failure this command exists to prevent. `command_quiet.rs` became `command_mute.rs`,
  `pns-domain/src/quiet.rs` and its calendar submodule became `mute.rs`, and
  `pns-application/src/set_lights_quiet.rs` became `set_lights_mute.rs`, each moved with `git mv` so
  history follows, and the identifiers, the operator-facing report wording, `pns --help`, the render
  layout prose and the pns/docs/specs text moved with them. The daemon's calendar job argv became
  `mute calendar` while its spool id stayed `quiet-calendar`, so a registration already in the spool is
  replaced rather than orphaned. The shipped config template was regenerated from the new prose. Tests
  pin that `pns mute 2h` mutes for two hours, that `pns lights mute "<place>" off` clears that place and
  reports nothing muted, and that both retired words are refused. Left for the config-table slices: the
  `[quiet]` table and its `calendar` child, `[focus] silence`, and the persisted file and table names,
  which a rename would migrate rather than reword.

  SLICE 34 DONE 2026-09-19, [PR #812](https://github.com/webdavis/dotfiles/pull/812), merged `b78939033`.
  Slice 34 of the pns refactor ladder took the two test-only knobs out of production builds. The ring
  lock's stall used to come off `PNS_RING_LOCK_TEST_DELAY_MS`, which unguarded production code read on
  every ring append and slept on inside the locked section every event passes through, so a stray
  variable exported in a real shell changed real behaviour; it became `ring::stall_inside_the_ring_lock`,
  a `cfg(test)` thread-local injection point whose `cfg(not(test))` twin is a fixed zero, leaving a
  release build with no environment read and nothing settable. SQLite's busy bound used to come off
  `PNS_DB_BUSY_TIMEOUT_MS`, a variable whose own comment called it test-only while `busy_timeout()` read
  it on every connection; it became the config key `[storage] busy_deadline`, a duration string through
  the domain's one parser, bounded at 10 milliseconds (below SQLite's own busy-handler sleep granularity,
  where a bound is indistinguishable from zero) and 60 seconds (a hook you are waiting on pays the bound
  per lock acquisition), with "0s" turning the handler off and the shipped 5 seconds as the default, and
  it reaches the store through `InstallSettings` beside `remote_deadline` rather than being threaded
  through the twenty-odd `SqliteStore` construction sites. `duration_key` gained a `duration_value`
  sibling that keeps the parsed `Duration` whole, because the `ms` unit the parser has accepted since
  slice 33 (task 93's own slice 1 note) makes a sub-second bound expressible and its seconds count would
  have read one as zero. The one test that set the retired variable now writes the key into its own
  sandbox config. Three assertions were added: the ring stall is zero until a test asks for one, the key
  parses to the millisecond with zero allowed and an out-of-range value refused by name, and a parsed
  value reaches the connection's `busy_timeout` pragma. The config template was regenerated with
  `just pns-config-render` and two stale passages in
  `pns/docs/specs/persistence-and-process-lifecycle.md` were corrected, one of which already credited the
  ring race test to a delay that test does not use. `just test-rust` and `just lint-check` both passed at
  exit 0 from the worktree after `origin/main` was merged in.

  SLICE 35 DONE 2026-09-19, [PR #820](https://github.com/webdavis/dotfiles/pull/820), merged `a7d51c5ce`.
  Slice 35 of the pns refactor ladder landed, settling the condenser variable that slice 31 noted and
  deferred. The recap's text shortener is the summarizer everywhere now: `condenser_prompt` and
  `condenser_verdict` became `summarizer_prompt` and `summarizer_verdict` in the domain, `condense` and
  `condenser_home` became `summarize` and `summarizer_home` in the codex adapter,
  `pns-domain/src/condenser.rs` became `summarizer.rs`, and the same word moved through the render
  layout, the configuration and return-recap specs, and every test name that carried it. The bound is
  `[recap] summarizer_deadline`, which takes a duration string the way `[storage] busy_deadline` does and
  replaces the bare-seconds `summarizer_deadline_secs`; the old spelling is refused by name at load, a
  value outside the millisecond floor or the one-hour ceiling is refused by name too, and zero is still
  accepted as the statement that the recap falls to its plain lists. `PNS_CONDENSER_DEADLINE_MS` and the
  `PNS_CONDENSER_DEADLINE` spelling that replaced it are both gone, which settles plan item 87 for this
  pair: the file the operator reads is the only place the bound is set, and it reaches the codex spawn
  through `InstallSettings`, the seam slice 34 used for the database's busy bound. The turn summarizer
  takes at most thirty seconds of that bound, because a Stop hook is blocked on that call while nothing
  waits on the recap's own episode, and a unit test pins both halves of the cap. Three tests pin the
  rest: the parsed duration reaches the summarizer's spawn, an out-of-range value is refused by name, and
  the three retired variable spellings set in the environment leave the deadline at its default.
  `dot_config/pns/private_config.toml.tmpl` was regenerated with `just pns-config-render`.
  `just test-rust` and `just lint-check` both exit 0.

  SLICE 36 DONE 2026-09-19, [PR #806](https://github.com/webdavis/dotfiles/pull/806), merged `58c7603af`.
  Task 93 (plan item 90) shipped. The three vendor-named plugin tables now name their function with
  `type` naming the vendor, which is the shape `[plugins.mobile] type = "moshi"` already used:
  `[plugins.hue]` became `[plugins.lights] type = "hue"`, `[plugins.macos-banner]` became
  `[plugins.banner] type = "macos"`, and `[plugins.router]` became
  `[plugins.home_presence] type = "unifi"`. A plugin table's heading is its registered name, so the
  rename carried through `registry::ROSTER`, `CORE` and `REQUIRES`, every doctor line and delivery leg,
  the schema roster, the render layout, the committed values file, the regenerated config template and
  the resolved-config snapshot, across 175 files in the four pns crates. Two refusals were added at load:
  one names a config still holding an old heading, giving the new heading and the type it takes, because
  a plugin table nothing registered keeps its settings free-form and would otherwise arm nothing
  silently; the other refuses a `type` under `[plugins.lights]` or `[plugins.banner]` that no compiled-in
  backend answers, naming the one that is accepted. `[plugins.home_presence]` kept its own type refusal
  in `router_settings`, which is what lets the probe's diagnostic still report a router that is
  configured and unreachable. An absent `type` under the two tables that gained one reads as their single
  compiled-in vendor. The behavioural specifications under pns/docs/specs moved to the new spelling
  wherever they quote a heading, a roster name or a doctor line. `just test-rust` and `just lint-check`
  both exited 0.

  SLICE 37 DONE 2026-09-19, [PR #819](https://github.com/webdavis/dotfiles/pull/819), merged `88e82fa32`.
  Slice 37 of the pns refactor ladder (plan item 91) collapsed the two durable-log tables into one.
  `[plugins.hermes]` and `[plugins.discord]` served one function and could only ever be one at a time, so
  they became a single `[plugins.log]` table whose `type` names the transport, "hermes" or "discord",
  with the sub-tables keeping their key names under the new heading: `[plugins.log.keys]` for the
  per-route hermes signing keys and `[plugins.log.channels]` for the Discord channel map. The load-time
  refusal that named both tables was deleted with its test, because one table cannot declare two logs and
  the failure is now unrepresentable rather than caught. In its place the type is settled at load,
  refused by name with both accepted transports listed whichever way the switch is set, and the table is
  then filed under the transport it names, which is the name the roster registers, the delivery leg
  carries and the ledger records, so nothing downstream of the config layer changed name. That settling
  replaced `[plugins.discord] type = "bot"`, so the backend check and the refused reading it fed
  (`discord_backend`, `BOT_TYPE`, `DiscordSettings::refused`, `refused_discord_line` and the switched-off
  discord warning) went with it as unreachable. Both old headings are refused by name with the new
  spelling. The shipped file carries the hermes keys and the Discord token and channel map in the one
  table, so the cutover between transports is a single line and the rollback is that line back; the
  per-route key names and the channel ids remain KeePassXC entry titles in
  `dot_config/pns/config-values.toml` and did not change, and `dot_config/pns/private_config.toml.tmpl`
  and the resolved-config snapshot were regenerated. Task 93 covers this work and is done.

  SLICE 38 DONE 2026-09-20, [PR #824](https://github.com/webdavis/dotfiles/pull/824), merged `6bf8620be`.
  Slice 38 of the pns refactor ladder gave the phone plugin the top-level `[phone]` table.
  `[plugins.mobile] type = "moshi"` became `[plugins.phone] type = "moshi"`, and `[phone] marker_file`
  moved under that heading keeping its name. Two keys lost a stutter and a unit at once:
  `mobile_watch_card` became `card_while_watching`, and `submit_deadline_secs` became `ack_deadline`, a
  duration string read through the config layer's `duration_value` helper over the range the count of
  seconds already allowed, one second to one hour, with zero still refused by name because a deadline
  that expires before the daemon can answer costs the phone card on every approval. `config/mobile.rs`
  became `config/phone.rs` after the old top-level table module was removed, so the file's history
  followed the plugin rather than the retired table. Both `[phone]` and `[plugins.mobile]` are refused at
  load naming the new spelling, the second through the same renamed-tables row every other moved heading
  uses, and the registered plugin name, the destination id and the ledger's leg name followed the heading
  to `phone` the way they did for hue, router and macos-banner in slice 36; `Surface::Mobile` and the
  decision input named for it were left alone. The shipped config template and the resolved-config
  snapshot were regenerated. `just test-rust` and `just lint-check` both exited 0. Phone delivery was
  down between the merge and the apply that followed it in the same sitting.

  SLICE 40 DONE 2026-09-20, [PR #825](https://github.com/webdavis/dotfiles/pull/825), merged `a98cfd3da`.
  Slice 40 of the pns refactor ladder renamed the `[plugins.presence]` settings to say what each one
  measures. `exclude` became `excluded_rooms`, which the table documents as a subtraction from `rooms`,
  keeping the rule that `desk_room` must be in `rooms` and not in `excluded_rooms` and refusing either
  violation by name. `poll_secs`, `stale_after_secs` and `desk_stale_after_secs` became `poll_interval`,
  `reading_max_age` and `desk_input_max_age`, each a duration string read through the config layer's own
  duration helper, at the values the file already shipped ("5s", "15s", "2m") and inside the ranges the
  code already enforced ("2s" to "1m" for the interval, never under the interval for the reading bound,
  "1s" to "1h" for the desk bound). Each retired spelling is refused at load by the plugin roster's
  unknown-key listing, which names the key that replaced it, so a config that missed the rename is
  refused whole rather than read half-way at a default the operator believes they changed. The render
  layout, the shipped values file, the regenerated config template, the resolved-configuration snapshot
  and the two specs quoting the old keys all moved with the rename, and the presence tests pin the
  subtraction, both desk-room refusals, the three durations reaching the settings the seconds used to, an
  out-of-range and a bare-count value refused by name, and every old key refused by name. Task 93 stays
  as filed. `just test-rust` and `just lint-check` both pass, before and after a merge of origin/main.

  SLICE 41 DONE 2026-09-20, [PR #827](https://github.com/webdavis/dotfiles/pull/827), merged `3d9d0a627`.
  Slice 41 of the pns config refactor moved the lights behaviour and state names. `[lights.github]`
  became `[lights.checks]` and its two colour keys became `pass_color` and `fail_color`;
  `[lights.unread]` became `[lights.unseen]`, the finished-run lamp; and the word a target declares
  became `behaviours` rather than `shows`, pairing it with its sibling `dim_behaviours` (plan items 93
  and 98 and the `[lights.unread]` rows of the Names table). The lamp behaviour enum, the config roster,
  the target parser, the render layout and prose, the shipped values file, the regenerated template and
  the resolved-config snapshot all moved together, and every retired heading, key and behaviour word is
  now refused at load by name, with the spelling to write listed in the same sentence. The lamps are
  pns's own, so no caller outside pns moved, and the GitHub notification source kept its own vocabulary.
  Risk carried: the lamps go to their unconfigured state between the merge and the operator's apply, so
  the apply belongs in the same sitting.

  SLICE 44 DONE 2026-09-20, [PR #826](https://github.com/webdavis/dotfiles/pull/826), merged `1fcd5b086`.
  Task 93, slice 44 of the pns refactor ladder, renamed the three `[delivery]` keys whose names disagreed
  with what the code did with them (plan item 104). `max_attempts` was compared against the retry count
  rather than the attempt count, so it became `max_retries`, and the boundary it sets is now pinned: N
  retries run and the N+1th is refused, with zero permitting no retry at all. `retry_base_secs` was never
  a base but the increment the retry count multiplies, so it became `retry_step` and now takes a duration
  string bounded from a second to an hour. `max_age_secs` was measured against the original event's
  creation epoch, which its name did not say, so it became `event_max_age` and now takes a duration
  string bounded from a minute to thirty days, with the shipped default spelled "168h". None of the three
  comparisons needed correcting; each read correctly under the new name and each is now pinned by its own
  test, including that the age is judged against the original event rather than the retry about to run.
  Every retired spelling is refused by name at load through the roster's own unknown-key listing, which
  carries the word to write instead. The domain fields followed the keys, the two new durations carved
  zero out the way every other duration key does, the render layout, the resolved-config snapshot, the
  persistence spec and the delivery decision record followed, and the shipped config template was
  regenerated. The values file needed no change because all three ship at their defaults. Gates green:
  `just test-rust` and `just lint-check` both exit 0.

  SLICE 45 DONE 2026-09-19, [PR #823](https://github.com/webdavis/dotfiles/pull/823), merged `1f0ba65c8`.
  Task 93, slice 45 of the pns refactor ladder. The recap and failures tables now name what they control.
  `[recap] digest` became `post_window_recap` (its value is whether the whole-window recap is posted,
  inside a table already called recap), `min_events` became `minimum_events`, `repos` became
  `repositories` (the parser reading it was already called that), and `review_notes` became
  `review_notes_glob`, because it names a glob pattern rather than the notes. `[failures] serve` and
  `port` configure an HTTP listener inside a table named for the failure record, and became
  `page_enabled` and `page_port`. Every retired spelling is refused by name at load through the roster's
  existing unknown-key listing, which prints the word to write instead, and two tests pin that one key at
  a time across both tables. The struct fields, the refusal sentences, the render layout, the specs and
  the shipped config template followed the keys, and the template was regenerated rather than hand
  edited. Plan item 109. Gates: just test-rust and just lint-check, both exit 0 after merging origin/main
  past slice 37. Operator step: one full chezmoi apply, because a deployed config still carrying an old
  spelling is refused at load until it lands.

  SLICE 20 DONE 2026-09-20, [PR #840](https://github.com/webdavis/dotfiles/pull/840), merged `3bf5a2e19`.
  Both version 1 envelopes got one rule for an absent optional field: omit it. Five fields that used to
  serialize as null (the result's `request_id` and `ledger_sequence`, the request's `session`, `elapsed`,
  `project`, `branch`, `pane` and `route`) now carry `skip_serializing_if` beside their `default`,
  joining `note`, `route` and `retry_at`, which slice 19 had already skipped, and a field that arrives as
  null still decodes as absent. posture's own reading of both envelopes moved in the same change, so the
  two sides continue to pin the same golden documents field for field, and protocol-v1 S011 and S017 plus
  posture's producer-api state the rule once instead of per field. The second half removed the last panic
  on the delivery path: the receipt built each destination outcome with `Name::new(...).expect(...)`,
  which would have crashed a whole submission on a registered name over the wire's 64-character cap or
  carrying a control character, so the plugin registry now refuses such a name at registration with a
  `RegistryError::UnencodableName` that names the limit, `pns-protocol` takes its cap from that one
  constant, and the receipt reports an absence on a path that can no longer produce one. New tests pin
  the omission on each envelope, the two registration refusals, and a receipt built for every name the
  compiled roster registers. `just test-rust` and `just lint-check` both exited 0 from the worktree after
  a final merge of main.

  SLICE 21 DONE 2026-09-20, [PR #845](https://github.com/webdavis/dotfiles/pull/845), merged `433e8dfb4`.
  The last request-envelope slice of the pns refactor ladder landed the symmetric Rust type names, with
  no wire change. `pns_protocol::request::Request` became `RequestEnvelope`, standing beside the
  already-correct `ResultEnvelope`, and the decoded request became `DecodedRequest` under its own name
  rather than a `Decoded as DecodedRequest` re-export alias, so the two version 1 envelopes now read as
  one naming convention instead of three. The colliding domain enum `pns_domain::retry::DeliveryOutcome`
  became `retry::TransportOutcome`, leaving the wire `pns_protocol::DeliveryOutcome` its name and ending
  the collision between a transport answer and a per-destination delivery verdict. Every use site inside
  pns moved in the same commit, across pns-protocol, pns-domain, pns-application, pns-adapters and pns;
  posture, which carries its own separate types, was not touched. `pns/docs/specs/protocol-v1.md` and
  decision record 0013 now name the new types. The schema strings stayed `pns.request/1` and
  `pns.result/1`, and both pns-protocol golden fixtures came out byte-identical to main, which with a
  green `just test-rust` and `just lint-check` is the evidence the wire did not move. Task 93 is
  complete.

  SLICE 27 DONE 2026-09-20, [PR #837](https://github.com/webdavis/dotfiles/pull/837), merged `92ea723a0`.
  Slice 27 closed the request-envelope work by making bad input a refusal on both paths. On the command
  line an unknown flag or stray word is refused as "<word> is not a flag pns takes" and a value flag
  given no value as "<flag> requires a value", each exit 2 with nothing delivered, where the parser used
  to skip the first in silence and warn about the second; the warnings channel went with it. In a JSON
  request the first top-level field version 1 does not define is refused as "`<field>` is not a field pns
  takes", correlated to the request id and before any effect, where the decoder used to ignore it and
  name it back. Retired flags and retired fields keep their name-the-replacement refusals, and closed-set
  values (--state, --scope, and their JSON twins) kept the refusals they already had. An unknown JSON
  field is refused and named like an unknown flag, and the result envelope's ignored_fields list stays,
  now meaning fields the envelope recognizes but acts on nowhere; no such field exists today, so the list
  is empty on every accepted request. Two flags pns does take are recognized by the parse rather than
  refused: the three reminder switches, which remind_switch reads off the raw argv, and --no-color, which
  the composition root answers while the event path still receives it. The caller grep the plan names
  found six live callers and every one already passed only flags and fields the final envelope knows, so
  none needed moving. `protocol-v1.md` S013, S014, S017 and S028, the pns-protocol crate's compatibility
  policy, and the two historical spec pages describing the lenient parser were updated. `just test-rust`
  and `just lint-check` both exit 0.

  SLICE 39 DONE 2026-09-20, [PR #831](https://github.com/webdavis/dotfiles/pull/831), merged `3fc3d9d01`.
  Slice 39 gave one word to a credential, a route and a host. Following the operator ruling recorded
  above, each single credential is now named for the kind of secret its own tool issues, spelled out,
  with the KeePassXC entry title as the authority: `[plugins.phone] token` became `device_token`,
  `[plugins.log] token` became `bot_token`, `[plugins.github] token` became `personal_access_token`,
  `[plugins.lights] key` became `api_key`, and `[plugins.home_presence] api_key` was already right; that
  ruling reverses plan item 94's `key`/`keys` rule, and `[plugins.log.keys]` keeps its name because it is
  a map of route keys rather than one credential. `[plugins.home_presence] stale_alert_channel` became
  `alert_route`, leaving "channel" to mean a Discord channel id, and the two host settings stopped
  stuttering and said what they hold: `[plugins.lights] bridge_host` and `[plugins.home_presence] url`,
  the latter matching the `url` spelling the log and phone tables already use. Every retired spelling is
  refused by name at load with the key to write listed in the same sentence, because the roster in
  config/schema.rs is checked before any arm reads a key, and a new test module pins that alongside each
  credential reaching its own plugin's reader through the resolved config. The readers, the render
  layout, the runtime failure lines that quote a config key, the committed values file, the regenerated
  template and the resolved-config snapshot all moved together, and
  `plugins.github.personal_access_token` was added to the render's secret-bearing key list, which had
  never covered the GitHub token. Risk carried: every credential key moved at once, so between merge and
  apply no plugin that needs one could load, and the operator ran one full apply in the same sitting.

  SLICE 42 DONE 2026-09-20, [PR #834](https://github.com/webdavis/dotfiles/pull/834), merged `4e671e923`.
  Slice 42 gave the lights timing keys one word per idea and one value shape.
  `[lights.loop] threshold_secs` and `[lights.unseen] after_secs` became `arm_after`,
  `[lights.loop] lease_timeout_secs` and `[lights.blocked] give_up_after_secs` became `lease_expiry`, and
  `[lights] refresh_secs` became `arm_interval`, a plain rename rather than the fade split the plan once
  proposed, since `refresh_secs` was never a fade budget. Each key now takes a duration string with the
  bounds the bare counts carried spelled in the same units, zero refused by name on the interval and both
  leases and still meaning "at once" on the unseen arming delay, and each retired spelling refused at
  load with the word to write named in the refusal. The Rust fields and bound constants moved with the
  words, `dot_config/pns/config-values.toml` states `arm_after = "6m"`, and the shipped config template
  and the resolved-config snapshot were regenerated. Task 93's sentence: the lights config vocabulary is
  now one word per idea across the three tables, with slice 43's percentages and durations the remaining
  lights work. Risk carried: an apply window, because the deployed config still holds the old keys the
  new binary refuses, so the lamps go to their unconfigured state until the operator applies.

  SLICE 43 DONE 2026-09-20, [PR #846](https://github.com/webdavis/dotfiles/pull/846), merged `c5f5b075e`.
  The last of the three lights slices in the config-table block. The lamp tables' unitless numbers now
  name their unit: `brightness`, `high`, `low` and `flare` became `brightness_percent`, `high_percent`,
  `low_percent` and `flare_percent`, still 1 to 100 and refused by name at 0 and 101, while `duration_ms`
  and `flare_ms` became `duration` and `flare_duration`, duration strings carrying over the 200ms to 5s
  bounds the fade code already enforced, through a `fade_duration` helper built on slice 42's own
  `positive_duration`. `[lights] dim_window` became the one dim window in the vocabulary: the house
  default every place that states none of its own runs, overridden per place by a declaration's own key,
  so a lamp can name which behaviours run dimmed without repeating when. The no-dead-knobs guard moved up
  to `parse_lights`, where the whole table is in hand and can see the house key a declaration may be
  leaning on, and `Target::dim_behaviours` became an option so an explicitly empty list stays a different
  answer from silence. `plugins.lights.quiet_hours` and `plugins.lights.rooms` are gone and refused at
  load by name with the spelling to write in the sentence; the window they duplicated is the key above,
  and the room list was dead whenever a lamp map exists, so the plain pulse takes the plugin's own
  default rooms, the bare `pns lights mute` reads the house window for its schedule, and the setup wizard
  stopped asking for a list nothing would read. The values file ships the house window, drops the four
  per-place copies of it and adds the `lights.zone` example the template carried only as prose; the
  rendered template and the resolved-config snapshot follow, and the four specification documents that
  named the retired keys were swept. Task 93 is closed by this slice: the percent-valued keys, the fade
  durations and the single dim window were its three remaining items. Both gates were green from the
  worktree root, and the apply window was the standing risk, so the operator's full apply belonged in the
  same sitting as the merge.

  SLICE 46 DONE 2026-09-20, [PR #843](https://github.com/webdavis/dotfiles/pull/843), merged `187f5ebaa`.
  Slice 46 landed the config block's switch cleanup, closing plan items 101 and 102. No config key
  doubles as its own on/off switch any more. `[focus] silence` split into `[focus] modes`, the roster,
  and `[focus] enabled`, the switch (default true), read together through one accessor so nothing can
  consult the roster and forget the switch. A new refusal in the schema names a zero duration and says to
  leave the key unset for off, which retires `[remind] delay = "0s"` and `[stale] escalate_after = "0s"`;
  `[stale]` gained its own `enabled` key (default true) because an unset window there means an hour
  rather than off, and the page now reads the window through an accessor that answers zero while the
  switch is off. The zero carve-out stayed for the four keys whose zero is a real bound rather than a
  feature off (the database busy deadline, the event age ceiling, the unseen lamp's arming delay and the
  summarizer deadline), and the recap's summarizer, repository list and review-notes pattern needed no
  code change because unset was already their off statement. Every `enabled` is now written out at its
  own default: the render layout writes each plugin switch at the schema default of false instead of
  true, the committed values file states the switch for the seven plugins dresden runs (a new banner
  table holds only that line), and the first-run wizard writes the switch for every plugin it arms, so
  the rendered template carries the same seven live plugin switches it carried before and nothing flipped
  off. Eleven switch lines now appear in the shipped file, live in a live table and commented in a
  commented one, pinned by a test that counts them against the schema roster. The specs for
  configuration, stale-block escalation, reminding, doctor diagnostics, quiet behavior, presence and
  visibility, setup and publication and producer submission were moved to the new semantics.

  SLICE 48 DONE 2026-09-20, [PR #848](https://github.com/webdavis/dotfiles/pull/848), merged `c53de1ece`.
  Slice 48 closed plan items 112 and 113, the values file's two uses of presence and of an open table as
  settings. `[plugins.log.keys]` lost the `note` entry the renderer used to strip, and write.rs stopped
  stripping a note in any open table, so a gateway route actually named `note` now gets a key like every
  other route; the per-class `[delivery_class.<name>]` notes and the lamp, room and zone declaration
  notes kept working, since each of those tables has a closed key roster. An empty opt-in table now
  renders exactly like a table the values file never mentions, and the two callers that had leaned on the
  old behaviour state the setting instead: the values file writes `[remind] delay = "5m"` and the setup
  walk writes the same literal, taken from the layout's own sample, when it arms the reminder. The
  regenerated template changed by three comment lines and nothing else, and the resolved-config snapshot
  was unaffected. Two tests pin it: an empty `[remind]` renders byte-identically to no `[remind]` at all,
  and a route named `note` round-trips as a key. Both gates green.

  SLICE 49 DONE 2026-09-20, [PR #851](https://github.com/webdavis/dotfiles/pull/851), merged `d8b43f157`.
  pi and omp gained unanswered-approval reminders, and the pns gate their moshi extensions were reported
  to call was corrected rather than left claiming a job it never did. Each harness now carries a pns
  extension of this repository's own, deployed beside the moshi-hooks.ts that moshi-hook generates and
  owns, which arms `pns hook blocked --remind` when the harness opens a question and clears it with
  `pns hook resolved` when the harness closes one, the same waiting and answered pair Claude Code carries
  in its settings and Codex gets from its hook installer. The two event pairs were confirmed against
  upstream before any code was written: pi has `ui_prompt_start` and `ui_prompt_end`, and omp, which has
  no such pair, has `tool_approval_requested` and `tool_approval_resolved`, so omp shipped with reminders
  rather than with the reminders-off setup output the plan had allowed for. No Rust was needed, because
  `pns pi-hook` and `pns omp-hook` already existed (the gate accepts any lowercase harness word) and
  neither producer forwards to moshi, so a blocked event raises pns's own notification without a second
  round trip. The apply script that repoints those extensions stopped describing itself as a presence
  gate: the generated file's own header says its `helperBinary` field is kept for debugging and manual
  replay while every live event is written straight to the moshi daemon socket, so the repoint decides
  which binary a replayed payload goes through and changes nothing about what pi and omp push in the
  moment, and the script's three warnings now say that instead.

  SLICE 50 DONE 2026-09-20, [PR #859](https://github.com/webdavis/dotfiles/pull/859), merged `d8949bbb5`.
  Slice 50 of task 93's pns refactor ladder landed, the first of the six recap slices. `pns gateway`
  absorbed `pns daemon`: `run`, `retry`, `schedule` and `cancel` joined `start`, `stop`, `restart` and
  `status`, `command_daemon.rs` was deleted with its body folded into a `command_gateway/` module
  directory split by verb group, and `pns daemon <anything>` is now refused with exit 2 and a sentence
  naming the gateway spelling rather than falling through to the event path. The `[daemon]` config table
  became `[gateway]` with the same two keys, `config/daemon.rs` became `config/gateway.rs` with
  `GatewayTable` and `parse_gateway`, `Config::daemon_enabled` and `daemon_service` became
  `gateway_enabled` and `gateway_service`, and a config still holding `[daemon]` is refused by name with
  `[gateway]` in the message. Every caller moved in the same commit so one apply closes the window: the
  `com.webdavis.pns-daemon` LaunchAgent runs `gateway run` under its unchanged label, the osquery launchd
  page allowlist records the new program string, `dot_config/pns/config-values.toml` moved its table and
  the shipped template was regenerated, and the seven named pns spec documents plus decision 0013 took
  the new command spellings, the new table name and the `pns gateway:` notice prefix. The daemon process
  kept its name wherever the prose is about the process rather than the command, and so did the on-disk
  job spool directory, because renaming that would orphan jobs already registered across the apply. Tests
  pin the four moved verbs, the eight-verb usage, the exit-2 refusal of every `pns daemon` spelling,
  `[gateway] enabled = false` stopping the clock, the refusal of the old heading, and the committed
  values file resolving to the committed template.

  SLICE 51 DONE 2026-09-20, [PR #863](https://github.com/webdavis/dotfiles/pull/863), merged `9afd65631`.
  pns recap learned to name its window the way a person reads a calendar. `--since` and `--until` now
  take a local date (`2026-09-19`), a local date-time (`2026-09-19T08:00`, seconds optional) or a
  duration ago (`2h`, `30m`, `3d`), and an omitted `--until` means now. `--since-epoch` and
  `--until-epoch` kept working unchanged, as an all-or-nothing pair, because the return moment spawns the
  detached recap with them. `recap_bounds` took a `now` argument and an injected local-zone function, so
  the window arithmetic stayed a total function of its arguments; the zone itself was read in one new
  adapter built on `mktime`, beside the `localtime_r` one, which refuses a day the calendar does not have
  rather than rolling February 30th forward. A duration reached pns-domain's one duration parser, with
  days spelled as hours so that parser's `ms|s|m|h` ranges and refusal text were left alone. Every
  refusal still exits 2 with the usage sentence, which now lists both spellings and all three value
  forms, and the tool-wide listing and the return-recap spec were updated to match.
  `pns gateway schedule --until +<duration>` and its `--until-epoch` were untouched. This covers task 93.

  SLICE 52 DONE 2026-09-20, [PR #865](https://github.com/webdavis/dotfiles/pull/865), merged `7f7819c07`.
  Slice 52 of the pns refactor ladder landed the activity store. pns keeps a durable `activity_events`
  table beside the ledger, one row per harness hook event carrying the arrival, the harness, the state,
  the project, the branch, the session and its title, the pane, the herdr workspace, the model, the card
  title and the detail; the schema moved from version 10 to 11 behind an idempotent migration a running
  store applies on first start after the apply, and a store from a later schema is still a refusal. The
  session title is read from the harness transcript in the order the recap design states, and the order
  was verified against real files rather than assumed: a Claude Code transcript writes the operator's own
  name on a `custom-title` line and the harness's generated one on an `ai-title` line, so the store takes
  the newest `customTitle`, then the newest `aiTitle`, then the title the sessions row already holds from
  the first prompt, while a Codex rollout file carries neither and so shows no title. That finding is
  written back into the recap design document. `[recap] retain` arrived as a duration string through
  pns-domain's parser, defaulting to thirty days spelled `"720h"` because the parser has no day unit,
  refusing zero by name the way `[remind] delay` does, with its roster row, its rendered config line, a
  regenerated `dot_config/pns/private_config.toml.tmpl` and a regenerated resolved-config snapshot. The
  gateway prunes rows older than the retention on its own tick, once an hour rather than once a second.
  Nothing reads the table yet, which is slice 53's work, and the activity ring keeps running untouched.
  Task 93 stays open until the recap engine reads this table. `just test-rust` and `just lint-check` both
  passed.

  Slice 53 merged as [PR #872](https://github.com/webdavis/dotfiles/pull/872) (c50c86e2b). 2026-09-20:
  shipped slice 53 of the pns refactor ladder, the recap engine, which is task 93's reader half. pns
  recap now takes the day's four periods plus today, yesterday, week and last-week, steps any of them
  back with `--previous`, answers a bare invocation with the window that most recently ended, and prints
  open on its own with no window at all. The window arithmetic is pure calendar work in pns-domain,
  answering in local civil moments the composition root resolves through the zone, so a step back is
  right on both sides of a daylight transition. `[recap]` repositories and the gh merged-listing adapter
  retired in favour of an argv-list `[recap.sources]` table (`pull_requests`, `commits`, `tasks`,
  `applies`) whose words carry `{since}` and `{until}`, and a section nobody configured starts no process
  and is absent from the page, from the document and from `--section`, and a command that exits non-zero
  renders one line naming the code rather than an empty section. The page gained `--section`, `--limit`,
  `--json`, `--toon`, `--schema` and `--to`: a mask alone selects document output in the mask's own
  format, `--json` or `--toon` beside it overrides only the format, and a mask key that names no field is
  refused with exit 2. The return card became one caller of this engine rather than a second poster. The
  styled page and the schema 1 document are each pinned by a golden fixture. Slices 54 (morning removed),
  55 (the summarizer) and task 170 are in flight as their own pull requests.

  SLICE STATUS 2026-09-20: merged 1 to 53; in flight 54 and 55; the ladder is 55 slices.

- [x] 92. CLOSED 2026-09-17, and it was a PRODUCT BUG rather than the flake it was being rerun past.
  Fixed on `fix/pns-dispatch-records-race`, merged as
  [PR #715](https://github.com/webdavis/dotfiles/pull/715). `open_existing` treated
  `PRAGMA journal_mode=WAL` as fatal. Measured on SQLite 3.53.4, that statement answers SQLITE_BUSY in
  0.000 seconds while another connection holds the write lock on a rollback-journal database, whatever
  `busy_timeout` says, because the conversion wants the database to itself and the busy handler is never
  consulted for it. A `?` on it aborted the whole open, and every record that open was about to write
  (the decision, the journal entry, the activity line, the delivery ledger row) went with it behind the
  fail-quiet `report` path, so the process exited 0 having notified nobody. That is exactly the
  `left: 2, right: 5`: three of five racing events lost the one-time conversion on a fresh database and
  silently wrote nothing. `prefer_wal` now treats a busy or locked refusal as settled rather than fatal,
  which loses nothing, because the journal mode lives in the database header and whichever connection
  wins settles it for every later one. Reproduced on demand before the fix two ways: a staged refusal
  (0.03s, now the committed regression test, red without the fix) and a load reproducer of 3200 fresh
  opens raced 16 at a time, which lost 11 opens without the fix and 0 with it. The fixture is exonerated:
  it reads read-only after every child has been waited on. The rerun-once tolerance was written down
  nowhere in the repository, so nothing remained to delete; it lived in per-lane briefs.

- [x] 87. Make the pns nag delivery test deterministic. Continuous integration for
  [PR #629](https://github.com/webdavis/dotfiles/pull/629) failed once on
  `nag_delivery::the_daemon_really_fires_the_nag_and_really_drops_it_when_the_marker_is_there`, which saw
  two cards where it expects one, although that pull request touched no pns code; a rerun passed.
  Reproduce it under load before changing anything, since a second card is either a real double fire or a
  fixture reading one delivery twice. Root-caused and fixed 2026-09-15; the fix is open as
  [PR #691](https://github.com/webdavis/dotfiles/pull/691) (`fix/pns-nag-delivery-deterministic`). It was
  neither a double fire nor a fixture reading one delivery twice: the daemon ticking beside the fire
  re-delivers the SAME event, because a channel script cannot confirm a delivery (`deliver_executable`
  answers `Silent` whatever it exits), so the nag's legs are retry-eligible the moment they are written
  and `pns daemon retry` hands the same event over a tick later. A delivery count is therefore a function
  of when the test thread happens to read it. The test now counts distinct CARDED EVENTS, which is one at
  every instant after the first delivery and is the number the operator's ruling is about. An earlier
  attempt on `fix/pns-nag-delivery` never got a continuous-integration run at all, through a close and
  reopen and an empty commit; the branch was re-cut and the new one ran immediately, so the silence reads
  as macOS runner queueing rather than anything about the branch. CLOSED 2026-09-17: root-caused and
  shipped as [PR #691](https://github.com/webdavis/dotfiles/pull/691), merged `bf4f9137`, with the fix in
  `4feb6aa4` counting carded events rather than delivery attempts. Only the checkbox was outstanding.

- [x] SUPERSEDED 2026-09-15 by tasks 78 and 90. This entry's premise, that image cards are blocked on
  transport, is no longer true: moshi's documented upload interface holds, the operator approved the
  capability, and the build is filed as task 90 with its two pieces and its deep-link tradeoff. Read task
  90 rather than this entry. Original entry, kept for its transport record: retain Moshi image recap
  cards as blocked on transport, not ready to build. The recorded reopening conditions are a homelab
  HTTPS image host, an upstream upload interface, or a documented data-URL path. An operator-approved
  single-card probe must establish actual image display before treating data URLs as supported. Revisit
  usefulness before adding a renderer; the proposed recap duplicates Discord. Source:
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
  operator-run upload probe live in `docs/research/2026-09-moshi-image-cards.md`. Decided 2026-09-15: the
  capability is approved, covering every card type including the recap, as a per-card-type opt-in shipped
  off by default. What the operator declined is an image on their own recap card specifically, a setting
  in their own config, not a limit on the capability; the trade is a real one, since a Moshi card's
  `data` carries one `type`, so turning images on for a card type gives up the deep link that focuses the
  originating herdr pane when that card is tapped. See task 78 (the decision) and task 90 (the build).
  Also noticed while checking: moshi-hook is six releases behind (0.3.16 installed, 0.3.22 in the tap).
  Full document: `docs/research/2026-09-moshi-image-cards.md`. Operator steps: (1) Read
  docs/research/2026-09-moshi-image-cards.md, specifically the Verdict and the seven assumptions in
  "Assumptions made in the operator's place"; assumption 1 (whether a documented web endpoint counts as
  "an upstream upload interface", when moshi-hook still has no upload subcommand) is the one that decides
  whether this entry reopens at all. (2) Tap the image test action in the Moshi app's notification
  settings on `mister`. Zero code, no token, one tap. It answers whether a rich image notification
  displays on that device at all, and everything else is moot if it fails. (3) Decided 2026-09-15: the
  capability is approved as a per-card-type opt-in covering every card type, the recap included; the
  operator's own recap keeps images off. (4) The token's path is left for the build to answer with
  evidence: may the Moshi token ride an `Authorization: Bearer` header on the upload leg?
  `pns/crates/pns-adapters/src/destinations/moshi.rs` currently states the rule as the request body "and
  nowhere else", and the documented upload interface requires the header form, so the build settles
  whether that rule is amended. (5) If you want the real round trip proved, run the two-command probe at
  the end of the research document while awake. It spends one of ten hourly uploads and sends one real
  card to your phone. (6) Separately from this task, consider `brew upgrade moshi-hook`: 0.3.16 is
  installed and 0.3.22 is in the tap, and 0.3.20 through 0.3.22 carry Pi agent detection, Codex
  named-session reset fixes and Herdr sidebar controls that touch this machine's daily path. Open
  questions: (1) Does the Moshi app's own image test action actually display a rich image notification on
  `mister`? Everything below is moot if it does not. (2) Do you want the real upload-then-webhook round
  trip proved with your token, and if so may it happen while you are awake rather than overnight? (3)
  Should the multipart body be hand-built through the existing `send()` (about twenty lines, no feature
  change), or should ureq's `multipart` feature be enabled despite living in its `unversioned` module,
  whose stated policy is that breaking changes there will not produce a major version bump? (4) Unrelated
  to the verdict: upgrade moshi-hook from 0.3.16 to the tap's 0.3.22 now, or leave it pinned? Resolved on
  2026-09-15: whether a documented web API endpoint satisfies "an upstream upload interface" with no
  `upload` subcommand on moshi-hook itself (yes, this is the surface pns actually calls), and whether the
  capability is worth building (yes, approved as a per-card-type opt-in). Superseded by task 78 (the
  decision) and task 90 (the build).

- [x] 78. DECIDED 2026-09-15: approved. This task was a decision and the decision is made, so it is
  closed here; the BUILD is task 90 and is separately open. Decide whether a recap card on the phone
  carries an image, and build it only if the answer is yes (operator ruling 2026-09-15, low priority).
  Decided 2026-09-15: approved. The capability covers every card type, the recap included; what the
  operator declined is an image on their own recap card specifically, a setting in their own config, not
  a limit on the capability, since another user might want exactly that for their own recap. The pattern
  is the one this repository already uses everywhere: build the capability, ship it off, and leave it off
  in the operator's own configuration. It is opt-in per card type because of a real tradeoff: a Moshi
  card's `data` carries one `type`, so turning images on for a card type gives up the deep link that
  focuses the originating herdr pane when that card is tapped. The operator's own configuration keeps the
  recap card's images off and keeps its deep link. The build itself, the per-card-type opt-in, the
  deep-link tradeoff stated at the toggle, and the card-ownership refactor `replay_missed` still needs,
  is filed separately as task 90, approved and not yet started. Full record:
  `docs/decisions/2026-09-15-pns-behavior-backlog-brief.md`. MEASURED CORRECTION 2026-09-16, and it bears
  on the decision above: THE RECAP CARD HAS NEVER CARRIED A DEEP LINK, so on that particular card an
  image trades away nothing. The replay card is built from
  `EventArgs { agent: "pns", state: "missed", detail, ..default() }`
  (`pns/crates/pns-application/src/replay_missed.rs:121`), so its `pane` is the empty string;
  `pane_is_safe("")` is false on its first clause (`pns/crates/pns-domain/src/safety.rs:17`), so
  `herdr_link` answers `None` (`pns/crates/pns-adapters/src/destinations/moshi.rs:55`) and the card ships
  with no `data` object at all. The tradeoff is real for every card type that DOES carry a pane, which is
  what still makes the opt-in per card type worth building. The operator's preference stands either way,
  as a preference; it is recorded here because the reason given for it does not hold, and a decision
  resting on a fact that is not true is worth re-offering rather than quietly inheriting.

- [x] 90. Build Moshi image cards as a per-card-type opt-in. DONE 2026-09-17. Approved 2026-09-15. Covers
  every card type, the recap included; the operator's own configuration keeps the recap card's images
  off. Two pieces, per `docs/research/2026-09-moshi-image-cards.md`: the card-ownership refactor, moving
  recap posting out of `replay_missed` and into the detached `pns recap` child, since the card is
  dispatched today before any render could exist. DONE 2026-09-16 in
  [PR #704](https://github.com/webdavis/dotfiles/pull/704), merged `9c5deb23`. The pointer this bullet
  used to carry was stale by file: `pns/crates/pns/src/return_replay.rs:38` is only `replay_missed`'s
  signature, and the posting is in `pns/crates/pns-application/src/replay_missed.rs`, publish at :91 and
  delivery at :118. The dispatch-before-render claim was CONFIRMED from source: `spawn_recap` returns as
  soon as `Command::spawn` succeeds, so the card was on the wire while the child had not yet read its
  config. The card now travels to the child as one JSON line on its stdin and is dispatched there, first,
  before the summarizer runs, so a parked model cannot hold the phone card for the child's whole
  deadline; a hand-off the child refuses leaves the card with the return moment, which delivers it
  exactly as before. WHAT REMAINS is the second piece: and the opt-in a per-card-type toggle plus the
  render, upload and image body, shipped off by default. State the tradeoff at the toggle, not only in a
  design document: a Moshi card's `data` carries one `type`, so turning images on for a card type gives
  up the deep link that focuses the originating herdr pane when that card is tapped. Say it accurately,
  though: see the correction under task 78, because the RECAP card carries no deep link to lose and a
  toggle claiming otherwise on that card would be wrong. THE TOKEN-PLACEMENT QUESTION IS ANSWERED,
  measured 2026-09-15 with bogus tokens only and no real credential read: the two routes differ. The
  upload route reads the token ONLY from an `Authorization: Bearer` header (a bogus header answers 401
  "Invalid token"; the token in the body with no header answers 401 "Missing or invalid Authorization
  header"). The webhook route requires it in the BODY (a header with no body token answers 422 naming
  property `/token`), which is what `moshi.rs` already does, so the current placement is correct and was
  not changed. Consequence for this piece: its upload leg needs the header, and `moshi.rs`'s "the request
  body and nowhere else" sentence has to become "the body on the webhook, an Authorization header on the
  upload". Source: task 78 and `docs/decisions/2026-09-15-pns-behavior-backlog-brief.md`. SHIPPED
  2026-09-17 as [PR #751](https://github.com/webdavis/dotfiles/pull/751), merged `73a33acf`, the second
  of the two pieces (the card-ownership refactor had already landed in PR #704). `MoshiChannel` gained an
  `image_cards` list, fed by a new open `[plugins.mobile.image_cards]` table, and SHIPPED OFF FOR EVERY
  CARD TYPE: an empty list leaves the channel byte for byte as it was, which is pinned by a test. For an
  armed card type, delivery draws the event's whole message as a one-bit greyscale PNG, uploads it, and
  posts an image object in place of the text card. ANY FAILURE ON THAT PATH FALLS THROUGH to today's text
  card with its pane deep link, pinned by four tests: nothing to draw, an upload that returned nothing, a
  reply carrying no code that still passes validation, and a webhook that rejects the image body. NO NEW
  DEPENDENCY: the font is 95 hand-authored five-by-seven glyphs as embedded data, plus a real glyph for
  the separator every pns title carries, and the PNG uses deflate's stored block type with hand-rolled
  checksums. It was verified DECODABLE OUTSIDE THE SUITE, read back as a 672 by 90 eight-bit greyscale
  image whose pixels dump as legible text art including the wrapped line. THE TOKEN PLACEMENT QUESTION
  THIS TASK LEFT OPEN IS ANSWERED IN CODE AND IN A TEST: the sentence claiming the body and nowhere else
  now reads per route, the body on the webhook and an `Authorization: Bearer` header on the upload, and a
  loopback fixture sends a bogus token through the real upload path and asserts the bearer header
  arrived, the request was multipart, and THE TOKEN IS NOT IN THE BODY; a second asserts that 401, 302
  and 500 all yield no code. No live credential was used and no card was sent. The deep-link tradeoff is
  stated at the toggle in the rendered config and stated ACCURATELY: an image card gives up the pane link
  for every card type THAT CARRIES A PANE, and the example key is `missed` precisely because the return
  card carries no pane and so has no link to lose. The renderer is pure and its tests take microseconds.
  OPERATOR STEPS: a full apply rewrites `~/.config/pns/config.toml` with the new heading, its prose and
  one COMMENTED example, so nothing is armed by the apply itself; and the live round trip is UNPROVEN and
  cannot be proven without the operator's own token, so arming one card type and watching for the image
  is theirs to do. Arming any type but `missed` gives up that type's deep link.

- [x] Preserve the pns refactor plan's explicitly carried-forward behavior work (section 7). B1 needs a
  reviewed Hue bridge certificate/identity-pinning design; `pns/crates/pns-adapters/src/hue/bridge.rs`
  still disables certificate verification. Define enrollment, changed-certificate handling and recovery
  before changing that behavior. Designed on 2026-09-14 in
  `docs/superpowers/specs/2026-09-14-hue-bridge-certificate-pinning-design.md`, approved 2026-09-15
  (approach A) and ready to build, not yet built; verification behavior is unchanged. The live bridge was
  measured: its certificate is `CN=<bridge id>, O=Philips Hue, OU=BSB003`, issued by `CN=root-bridge`,
  valid to 2038, with NO subjectAltName, and the unauthenticated `/api/config` reports the same bridge
  id, so chain verification against the published Hue root succeeds (verified locally with
  `openssl verify`) while name verification is impossible on any modern stack. ureq 3.4.x exposes no
  custom-verifier hook and couples native-tls's two danger flags to one, so verification requires a
  custom rustls verifier behind a connector supplied through `Agent::with_parts`; a scratch prototype
  built only from ureq's public `unversioned::transport` items accepted the matching pin (HTTP 200) and
  refused a one-bit-flipped pin with our own message intact, over a loopback fixture (the agent sandbox
  blocked the live LAN handshake, and the stock-ureq control failed the same way, so a live handshake
  remains an acceptance gate). The operator approved pinning the bridge's own certificate fingerprint
  rather than the Hue root plus identity: one `[plugins.hue] certificate = "sha256:..."` key that is a
  config refusal when hue is enabled without it, one `UreqBridge::new` constructor replacing seven struct
  literals, a print-only `pns lights enroll` that refuses when the certificate common name and the
  reported bridge id disagree, and a single permanent mismatch report through the 2026-09-08
  delivery-failure path with `pns doctor` showing the pin state. lights
  (`lights/crates/lights-adapters/src/hue.rs`) gets the identical change in this wave; the UniFi client
  (`pns/crates/pns-adapters/src/unifi/client.rs`) also disables verification and is filed as its own
  design task, task 89. All seven open questions were answered by the operator on 2026-09-15; the ten
  assumptions stand unopposed. Full document:
  `docs/superpowers/specs/2026-09-14-hue-bridge-certificate-pinning-design.md` and
  `docs/decisions/2026-09-15-pns-behavior-backlog-brief.md`. Operator steps: (1) After pull requests one
  and two land, run `pns lights enroll` on dresden with the bridge reachable, check that the printed
  certificate common name equals the bridge id, and paste the `certificate = "sha256:..."` line as a
  custom attribute on the "OpenHue :: API Key (hue-bridge-pro)" KeePassXC entry, beside the bridge
  address and key. (2) Run the full `chezmoi apply` yourself once the vault carries the pin; agents do
  not apply, and the shipped template is regenerated with `just pns-config-render`. Open questions, all
  answered 2026-09-15: (1) Approach A (pin the bridge's own certificate fingerprint), over B (Hue root CA
  plus bridge identity) and C (keep the current behavior, record the risk). The bridge certificate
  carries no subjectAltName, so B could only prove "some Hue bridge" rather than "this Hue bridge", and B
  would also require trusting a Philips root certificate copied from a third-party mirror. (2) Fail
  closed at config parse on a missing pin, no warn-then-refuse release. (3) The pin lives in the vault,
  as an attribute on the OpenHue entry, reversing the design's own recommendation: this repository is
  public, and its own convention already treats an id as a vault reference rather than a committed
  literal, which `dot_config/pns/config-values.toml` states in its own words about channel ids. (4) The
  bridge id check at `pns lights enroll` is optional with a warning when skipped, not required; without
  it, enrollment trusts whatever answers first, so an impostor present at enrollment time gets pinned and
  everything afterward looks correct. (5) lights gets the same pinning change in this wave; the UniFi
  router client's unverified TLS becomes its own design task, task 89. (6) `pns lights enroll` stays the
  command name; doctor reports state, enrolling performs an action and hands back a value to save. (7)
  Moot: approach B was not chosen, so its third-party trust anchor question does not arise.

  BOTH HALVES DONE 2026-09-17, merged as [PR #771](https://github.com/webdavis/dotfiles/pull/771) at
  `5304b62c` for pns and [PR #774](https://github.com/webdavis/dotfiles/pull/774) at `b8875c75` for
  lights. Approach A as approved. `certificate` is a required key whose absence or malformation is a
  config refusal at parse time, fail closed with no warning path and no default. The pin is a KeePassXC
  custom attribute on the existing "OpenHue :: API Key (hue-bridge-pro)" entry, which the pns config
  renderer gained a secret marker for, so no fingerprint enters the repository. A custom rustls verifier
  behind a connector supplied through ureq's agent parts completes the handshake inside connect, so a
  wrong certificate is a refused connection rather than a later error, and the two signature callbacks
  delegate to rustls rather than asserting. Enrolling prints the pastable line and writes nothing,
  refusing when the certificate common name and the reported bridge id disagree and warning, with the
  cost stated, when that check is skipped. A refused handshake is recorded once per process and announced
  once, and each tool's health output carries a pin-state row where a mismatch counts as an issue.

  The lights half is a DELIBERATE SECOND COPY, not a shared crate, because no cargo workspace may depend
  on another; lights still builds with pns absent from the filesystem. `disable_verification` and its
  "approved exception" comment are gone from both tools.

  Proven over a loopback TLS fixture whose certificate has the bridge's shape, a synthetic bridge id as
  common name with a `root-bridge` issuer and no subjectAltName: a matched pin reads and writes, a
  one-bit-different pin is refused at the handshake with no request reaching the server, and enrolling
  reads a real presented certificate plus the host's own config answer. THE LIVE HANDSHAKE AGAINST THE
  REAL BRIDGE IS UNPROVEN and stays the operator's acceptance gate, which is what the design said.

  OPERATOR OWES, in this order, and HUE DELIVERY IS REFUSED FOR BOTH TOOLS UNTIL STEP 2 IS DONE because
  fail closed was the approved choice:

  1. `pns lights enroll --bridge-id <the bridge id>` on dresden with the bridge reachable, then check the
     printed certificate common name equals that bridge id.
  1. Paste the printed `certificate = "sha256:..."` line as a custom attribute on the "OpenHue :: API Key
     (hue-bridge-pro)" KeePassXC entry. One value serves both tools.
  1. One full `chezmoi apply`.

- [x] 89. DONE 2026-09-15 in [PR #693](https://github.com/webdavis/dotfiles/pull/693), merged `5257bf24`,
  which wrote `docs/superpowers/specs/2026-09-15-unifi-client-certificate-pinning-design.md` (416 lines)
  and no code. The bullet's premise held up against source: `UniFiRouter::new`
  (`pns/crates/pns-adapters/src/unifi/client.rs:39`) builds its ureq agent with
  `.tls_config(TlsConfig::builder().disable_verification(true).build())` at line 42, so the client
  verifies nothing at all rather than pinning the wrong thing, and the API key rides that connection as
  the `X-API-KEY` header at line 70. The only production construction is `pns home`
  (`pns/crates/pns/src/command_home.rs:69`). The design offers three approaches and recommends a SHA-256
  fingerprint pin on the leaf certificate, names a private certificate authority as the better answer on
  rotation, puts the pin in the vault as an attribute on the existing UniFi entry, and states the cost
  plainly: a UniFi OS upgrade that regenerates the console certificate makes `pns home` read Unknown
  until the operator runs `pns home enroll`. The live certificate was NOT measured (reaching the
  operator's router was out of scope), so the document carries that measurement as an acceptance gate
  rather than an input. BUILD remains: the design is written, nothing is implemented. Original entry:
  design certificate pinning for the UniFi router client, filed 2026-09-15. Out of scope for the Hue
  bridge pinning design: `pns/crates/pns-adapters/src/unifi/client.rs` disables verification against
  `https://192.168.1.1` while sending a router API key, a more valuable credential than the lamp key, and
  it is a different device with a different certificate story that needs its own measurement and design.
  Follows the same shape as the Hue design once written: measure the live certificate, choose a pinning
  approach, decide where the pin lives (the vault convention that decided the Hue pin applies here too).
  Not started. Source: `docs/superpowers/specs/2026-09-14-hue-bridge-certificate-pinning-design.md`
  (out-of-scope section) and `docs/decisions/2026-09-15-pns-behavior-backlog-brief.md`.

- [x] 91. DONE 2026-09-16 in [PR #703](https://github.com/webdavis/dotfiles/pull/703), merged `314853e0`.
  `posture-producer-wire` was 16 files and 2,051 lines consumed only by `posture-adapters`, of which
  production needed exactly `RequestId`, `Status`, `decode_result` and the oversized-encode refusal. The
  two documents are now plain serde in `posture-adapters/src/wire/` (6 files, 648 lines) and the golden
  fixtures moved to `posture-adapters/fixtures/`, where a decode-and-re-encode round trip pins both
  directions field by field, so the wire contract is still held honest by documents rather than by a
  shared type. What went with the crate is the ENGINE'S half only: the depth-limiting parser, the
  duplicate-field refusal, the structural walk and the rejection taxonomy all guard input a program did
  not build, and posture never receives a request nor answers with a result. Every field survived, plus
  the byte cap on each direction and the text cap on `detail`. The config table is now `[notify]` with
  `hermes`, `command` and `off`, where `off` raises the finding on the local banner through the existing
  `IndependentAlarm` rather than discarding it, and `mode = "hermes"` still ships on dresden with its
  recorded reason intact. Net 2,216 deletions against 955 insertions. Original entry: drop
  `posture/crates/posture-producer-wire/` and converge posture on the same three-mode `[notify]` command
  shape (`desktop`, `command`, `off`) vpt uses, so posture stops carrying its own copy of pns's JSON
  envelopes. Approved by the operator 2026-09-15, not started. Surfaced while writing vpt's notification
  design: `posture/crates/posture-adapters/src/producer.rs`'s own header comment already states the
  target shape, "a JSON request goes in on standard input, a JSON result plus an exit code comes back,
  and the command and its arguments are both config", but posture still ships a dedicated
  `posture-producer-wire` crate for the envelope rather than the plain argv-and-stdin contract vpt's
  `[notify]` table covers with no crate at all. Source:
  `docs/decisions/2026-09-15-vpt-architecture-decisions.md`, decision 13.

- [x] Resolve the related B6/B20/B39 hook design: the answered-wait race, when `AskUserQuestion` should
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
  behaviors are listed to pin test-first. B39 is designed and approved to build now: sandbox network
  dialogs reach the dialog host directly with no `PermissionRequest`, their only hook-visible trace is a
  `Notification` whose type defaults to `permission_prompt` so no matcher can separate it from a tool
  approval, its payload cannot name the host, and no `sandbox` block exists in the managed template or
  the live settings, so exposure on dresden is currently zero (though flag and policy settings can open
  it without a local change). The interim wiring, written out in full in the design, is the build.
  Approved and ready to build 2026-09-15: `denied` no longer arms a wait, `SubagentStop` is added as a
  fifth declaration, and B39 is built now; the upstream report was declined. Full document:
  `docs/superpowers/specs/2026-09-14-hook-wait-events-design.md` and
  `docs/decisions/2026-09-15-pns-behavior-backlog-brief.md`. Operator steps: (1) Confirm or reject the
  six assumptions in 'Assumptions made in the operator's place'. (2) Once built, run the full
  `chezmoi apply` yourself; the declaration change only takes effect after a full apply. Open questions,
  four of five answered 2026-09-15: (1) Does `denied` belong in LAMP_BLOCKED? **Answered: no.** `denied`
  stops arming the waiting lamp and becomes an observation. `pns/crates/pns/src/hook_dispatch.rs` already
  treats a denial as a decision the harness has taken on its own, which is why it never forwards to the
  phone; the lamp had not caught up to that. (2) Should `SubagentStop` end a subagent's wait? **Answered:
  yes**, as a fifth declaration routed to `resolved`. (3) B39: build the text-allowlisted Notification
  arm now, or wait? **Answered: build now**, so it is ready if sandboxing is ever turned on. The alert
  stays approximate rather than specific, a property of the platform rather than a defect in the build,
  and exposure on this machine is currently zero. (4) Should the sandbox-network gap be reported
  upstream? **Answered: no.** The operator's words: the lack of distinction is fine. (5) Is the
  `[lights]` gate on arming a wait marker still right? Not answered, not one of the four filed rows. It
  is the only reason the state-based discriminator for B39 cannot be the recommendation, because on a
  machine with no lamps configured the dedup read always finds nothing. Nothing needs changing today.

  DONE 2026-09-17, merged as [PR #769](https://github.com/webdavis/dotfiles/pull/769) at `3bf72aca`. All
  six approved pieces built, ten behaviours pinned, six of them red first. Both `PostToolUse` matchers, a
  new asynchronous `ElicitationResult` entry and a new `SubagentStop` entry route to `pns hook resolved`;
  a second `Notification` matcher on the permission-prompt text runs the new `pns hook waiting`.
  `plan-ready` is deleted as an arm and as a state word. `denied` became an observation and left the
  blocked lamp set, so it neither arms nor takes a wait. An End now carries the caller's own moment and
  refuses to remove a marker armed after it, claimed by rename per decision record 0001. B39's
  text-allowlisted sandbox-network arm arms the marker and the nag with no moshi forward.

  ONE FINDING THE DESIGN DID NOT RECORD: `SubagentStop` carries `agent_id` in the installed 2.1.272
  bundle, so routing it to `resolved` unchanged would have hit that arm's subagent guard and cleared
  nothing, making the fifth declaration a no-op. The arm reads `hook_event_name` for that one case
  instead. Residual, commented at the arm: a subagent ending while the parent waits clears the parent's
  marker too, because one marker is keyed by the shared session.

  The event vocabulary was verified against the INSTALLED Claude Code 2.1.272, which matches the design's
  2.1.270 reading: the same thirty-four event names, the same `ElicitationResult` fields including its id
  and action, and the same static sandbox notification text.

  OPERATOR OWES: one full `chezmoi apply`. The declarations live in
  `private_dot_claude/modify_settings.json`, a modify template over the live settings file, so the new
  hook entries do not exist in `~/.claude/settings.json` until an apply runs, and until then the answered
  wait keeps racing as before.

- [x] Implement B18's decided behavior (2026-09-12): pause persistent agent-status lighting during
  `pns quiet` and macOS Focus. Pause the status effects, not ordinary room lighting. Preserve the settled
  security-banner and phone-alert mute bypass. Verify quiet/Focus transitions, including an effect
  already active when muting begins. B19/B25's nag tolerance and future-timestamp handling still need
  explicit disposition against current source; their conditional proposals are not automatic
  implementation work. Local implementation `cf7d4866` on `feat/pns-status-quiet` passed 16 focused
  checks, including eleven new cases, three mutation checks and package gates. Independent review passed
  17 checks. PR #536 contains this change and P4; combined full checks and the release build passed.
  Required checks passed and #536 merged. THE AGENT WORK IS DONE: what remains is the operator's own
  deployment and the lamp and Focus acceptance on their own devices, which no agent can run. B19/B25's
  disposition is the one part of this bullet still open to an agent.

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

- [x] Finish [6hR57XgFJxrgVFVM](https://app.todoist.com/app/task/6hR57XgFJxrgVFVM), the remaining
  nvim-mcp review. `pane_socket.lua` and `executable_nvim-mcp-connect.sh` validate the final runtime
  directory but leave replaceable ancestors unchecked. The resolver's `answers()` follows socket
  symlinks, and newline-containing runtime paths are accepted by the listener but split inconsistently
  during discovery. Source `bffa979a` and `e8bd14ac` fix replaceable ancestors, shared listener/resolver
  path validation, socket symlinks and newline pins. Independent review approved 23 listener cases, 51
  resolver cases, eight private socket drills and eight additional native path checks. Full `just ship`
  passed after integrating current main, and [PR #546](https://github.com/webdavis/dotfiles/pull/546)
  MERGED as `2fdf6e61`. CLOSED 2026-09-17. Both files (`dot_config/nvim/lua/custom_api/pane_socket.lua`,
  `dot_local/libexec/nvim-mcp/executable_nvim-mcp-connect.sh`) are chezmoi-managed and deployed with no
  drift. Live acceptance: with no Neovim in the pane, `nvim-mcp-connect.sh --diagnose` refused and named
  the socket it looked for; with the operator's Neovim open in the same herdr tab it resolved exactly one
  socket and exited 0 without delay, which is the fresh-connection check and the quiescent timing recheck
  together. Second-account and access-control-list behaviour was NOT tested and is recorded as not
  claimed, by decision rather than omission.
- [x] Resolve B103's same-workspace pane-move routing bug. The current integration validates workspace
  identity, while the agent resolver still uses the old `HERDR_TAB_ID`; the isolated review reproduction
  selected the old tab's agent. The cross-workspace refusal in `4c06b8ca` does not fix this case. Use
  supported Herdr interfaces and owned integration code; do not patch the third-party plugin. Commit
  `dfe28fd3` passed independent review with 94 private checks; full `just ship` and required continuous
  integration passed. [PR #543](https://github.com/webdavis/dotfiles/pull/543) merged and local main
  contains it. Deployed, and the live pane-move acceptance passed on 2026-09-15: drill one in
  [`docs/acceptance/nvim-acceptance-drills.md`](acceptance/nvim-acceptance-drills.md) ran with two Claude
  agents in two tabs of the same workspace, and moving Neovim's pane to the second tab made a resend
  follow that tab rather than the one it started in.
- [x] RESOLVED 2026-09-16, the pair is compatible now. Measured on dresden: `zig version` prints
  `0.16.0`, `zls --version` prints `0.16.0`, `zig env` exits 0 (it used to fail to locate its
  installation), `zls` is declared in `.chezmoidata/system_packages_autoinstall.yaml` beside `zig` since
  [PR #702](https://github.com/webdavis/dotfiles/pull/702) so the weekly bundle keeps it, Mason holds its
  own copy at `~/.local/share/nvim/mason/packages/zls`, and `dot_config/nvim/lua/plugins/lsp.lua:200`
  plus `blink-cmp.lua:60` already wire it. So the ZLS configuration is no longer unused and nothing is
  removed. The one part left is a preference, not a defect: whether a Zig neotest adapter is wanted at
  all, which is the operator's call and is filed as their decision rather than as work. Original entry:
  resolve B97's Zig tooling decision: supply a working, compatible Zig/ZLS pair or remove the unused ZLS
  configuration after that decision. At audit time Zig reported `0.12.0-dev.3158+1e67f5021`, Mason ZLS
  reported `0.15.1`, and `zig env` failed to locate its installation. The Zig neotest adapter is also
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

### herdr-todoist and todoist.nvim

Two Todoist plugins, filed 2026-09-17 and reshaped the same day on an operator ruling: the herdr plugin
is the home and the Neovim plugin is the editor, entered from the herdr pane. Each is its own public
repository, `webdavis/herdr-todoist` (Rust, a ratatui pane the way reviewr is) and
`webdavis/todoist.nvim` (Lua), the same way pns.nvim and neotest-bashunit are (operator ruling
2026-09-05); this repository carries only the herdr plugin config, the lazy.nvim spec and the keymaps.
Both talk to the Todoist API directly and share no code: Todoist is the only source of truth, and each
refreshes when it regains focus. The API token is a secret in both: it comes from a user-supplied command
(`token_command`, for example a `keepassxc-cli` call) or an environment variable, never from a value
written in a config file, and on dresden the chezmoi template names the KeePassXC entry. Filters use
Todoist's own filter query language, the one the app's Filters feature uses, so anything Todoist accepts
as a filter is a view in either plugin. Tasks 103 to 113 are the herdr plugin, 114 to 124 the Neovim
plugin.

The dresden wiring for both plugins, DONE 2026-09-18, merged as
[PR #785](https://github.com/webdavis/dotfiles/pull/785). Five files wire both finished plugins into
dresden. herdr-todoist joins `packages.herdr_plugins` at the pinned revision `bef263d7`, its config lands
under the manifest's own id with no owner namespacing, and one chord, `prefix+d`, opens it; `prefix+t`
and its variants were already taken by tab-smart-rename and tuicr. The Neovim half is
`dot_config/nvim/lua/plugins/todoist.lua`, pinned by commit and shaped after `pns.lua`, with a new
`<leader>T` group, because `<leader>t` is neotest's and the README's suggested keys could not be used
verbatim. Twelve config keys were enumerated on the herdr side and ten on the Neovim side, with the
report saying for each whether it was set or deliberately left at its default. Three views are shared by
name across both halves, so one word opens the same list in the pane and the editor.

THE REVIEW CHANGED THE TOKEN BOUNDARY, which is the operator-facing consequence of this pull request. The
implementer wrote a `token_command` calling `keepassxc-cli`, following the brief. The review MEASURED
that this can never resolve: keepassxc-cli 2.7.12 takes the database password on standard input only, and
both plugins spawn their token command without a terminal, so it exits 1 every time. It was reproduced
against the pinned binary, then fixed by pointing both `token_command` arrays at the macOS login keychain
(`security find-generic-password`), which is non-interactive after login, with KeePassXC remaining the
entry of record. The configuration reads the token from the keychain because of that measurement. The fix
was verified by seeding a throwaway keychain entry, watching token resolution succeed and the run proceed
into the network call, then deleting the entry. The real entry is the operator's to create, because it
needs the actual secret.

A second finding was a live ambiguity in the operator's own Todoist: the `#dotfiles` filter matches TWO
projects, one top level and one nested under `webdavis`, returning 58 tasks from two project ids. The
qualified filter `##webdavis & #dotfiles` returns only the 16 tasks of the subproject matching this
repository's path, and both config files now use it. No write was made to Todoist.

Two things were deliberately NOT wired and the report says why: the statusline component, because this
machine runs witch-line rather than lualine, and the reminder, because both are what start the plugin's
background poller and that poller would run the token command on a timer before the boundary is settled.
Each is one line to turn on.

- [x] 103. DONE 2026-09-17. Create the `webdavis/herdr-todoist` repository: a ratatui TUI in a
  plugin-owned pane with a `herdr-plugin.toml` manifest, `open`, `toggle` and `focus` actions, an async
  Todoist client with rate-limit and network errors shown in the pane's status line, and a `doctor`
  action that proves the token resolves and one request succeeds without printing the token. Installed on
  dresden through `herdr plugin install` and its config committed under
  `dot_config/herdr/plugins/config/`.

  `webdavis/herdr-todoist` created public; PR #1 merged `8aa21811`. A cargo workspace split into a client
  crate (async API v1: token resolution, one authenticated request, error mapping for 401, 403, 429 with
  Retry-After, network failure and unreadable bodies) and the plugin binary, which is both the pane and
  its actions. The manifest declares one pane entry point plus `open`, `toggle`, `focus` and `doctor`,
  with a build step producing the binary the pane execs. The pane is ratatui: a status line carrying
  either a connected reading or the API failure in its own words, `r` to refresh, `q` to close.

  THE TOKEN IS AN INDIRECTION ONLY, which is stronger than this entry asked for: `token_command` (argv,
  standard output is the token) or `token_env` (a variable name), the command winning, NO default, and a
  literal token key in the config file is a PARSE ERROR because the config struct denies unknown fields.
  The token type has no display form and a redacted debug form, and a failing `token_command` is reported
  by its program name alone, so neither its output nor its arguments can leak. Proven by running the
  binary: no token source refuses and names both doors, and a config holding a literal token fails to
  parse and lists the two legal keys. 33 tests, slowest target 0.06 seconds, every client case against a
  loopback double on an ephemeral port.

- [x] 104. DONE 2026-09-17. List view. Every open task grouped by project and section, each line carrying
  due date, priority, labels and subtask count, with subtasks folded under their parent, `R` to refresh,
  and the cursor kept on the same task across a refresh.

  PR #2 merged `df9700ff`. Each line carries its due date, its priority in the app's own wording, its
  labels and its subtask count, and a failed refresh keeps the last good rows with the API's words in the
  status line. THE CURSOR TRACKS A TASK'S IDENTITY rather than a row number, pinned across insertion,
  removal, reorder and total replacement; when the selected task is gone it takes the nearest survivor
  BELOW it in the previous order, scanning down then up, which is what a hand expects after deleting a
  line. The subtask relation is the task's own parent field, confirmed against the vendor documentation,
  and a subtask whose parent is not among the open tasks is drawn as top-level rather than disappearing.
  Paging walks the cursor at the documented maximum and stops if a cursor repeats. One review finding was
  real and would have lost data from the view: a task whose section id named no section in its project
  VANISHED, leaving a project heading with nothing under it; it now folds into the unfiled group. 43
  tests, every suite under a tenth of a second.

- [x] 105. Named filter views. `[[views]]` in the plugin config declares `name` and `filter` pairs
  (`today = "today | overdue"`, `work = "#Work & !@waiting"`), the pane switches between them with a
  picker and number keys, and each view is also a plugin action (`view:today`) so a herdr keybinding can
  open the pane straight onto it. A rejected filter shows the API's own message rather than an empty
  list.

  DONE 2026-09-17, merged as herdr-todoist pull request #3. `[[views]]` declares `name` and `filter`
  pairs in the plugin config, read through Todoist's own filter query endpoint on task 104's existing
  cursor walk rather than a second one. The pane switches with `v`, which draws a picker, and with number
  keys; view 1 is always the unfiltered list, so the pane works with no views configured.

  A PER-VIEW NAMED ACTION PROVED IMPOSSIBLE, established from herdr's own plugin documentation rather
  than assumed: runtime action registration is not part of plugin v1, and `herdr plugin action invoke`
  takes an action id with no trailing arguments. The manifest therefore carries nine numbered actions
  `view:1` to `view:9` over the same numbering the number keys use.

  A refused filter puts Todoist's own message in the status line and leaves the rows already on screen
  alone; an empty result empties the list and says so, so the two outcomes look different. Both are
  pinned against a loopback double at the client level and at the pane level. Config validation refuses a
  duplicate name, an empty name, an empty filter, and the reserved name `all`, which the unfiltered list
  already answers to. 100 tests pass, each suite under 0.12s, with no live API call.

  OPERATOR OWES: the dresden wiring is still a dotfiles pull request and is not filed. It would add the
  plugin's `[[views]]` table and `plugin_action` keybindings for the numbered views to
  `dot_config/herdr/config.toml`, with the token read through `token_command` naming a KeePassXC entry
  and never a value.

- [x] 106. Toggle pane. The `toggle` action opens the pane in the current workspace or closes it, `focus`
  jumps to it, both bindable in `dot_config/herdr/config.toml`; width, side and the view it opens on are
  config, and `auto_open = false` keeps it closed until asked.

  DONE 2026-09-18, merged as herdr-todoist pull request #4. `side`, `width`, `default_view` and
  `auto_open` are real config, and `auto_open` is a workspace-focused event hook that opens the pane in
  the workspace being entered without taking focus. A numbered `view:<n>` action still WINS over
  `default_view`, because the action writes the request note task 105 built and the pane reads that note
  before the config, so a keybinding asking for view 3 is never silently overridden.

  HERDR'S LIMITS WERE READ FIRST AND THEY CHANGED THE DESIGN. `herdr plugin pane open` accepts placement,
  workspace, target pane and direction only, where direction is right or down, with no ratio and no size
  outside a popup placement, and a popup pane has no pane id at all so it cannot be focused, closed or
  tracked. THE REVIEW THEN CAUGHT A SILENT FAILURE: a same-tab `herdr pane move` always answers unchanged
  with the reason `same_tab`, so the first attempt at width and a leading side did nothing and reported
  success. The fix drops the two sides herdr genuinely cannot split toward, and takes width by resizing
  the calling pane after reading its live ratio, checking the resize's own changed flag, so a refused
  placement now says so instead of passing quietly.

  Proven end to end against a herdr DOUBLE, a script on the binary-path variable keeping its pane list in
  a file: focus with no pane reports none, open opens, focus focuses, open again focuses, toggle closes,
  toggle reopens, auto-open opens once then reports the pane already open, auto-open with the default
  config makes no herdr call at all, and a zero width and a nonsense side are each refused by name.

  A RULE THIS LANE BROKE, recorded rather than excused: it created two probe workspaces on the LIVE herdr
  session to measure the same-tab move and the resize, which the brief forbade. Both probes are gone from
  the workspace list, so nothing was left behind, and the measurements are the reason the silent-failure
  fix is trustworthy. The brief for task 107 states the prohibition again.

  OPERATOR OWES: the dresden wiring is still a dotfiles pull request and is not filed. It would add the
  plugin's placement keys and `plugin_action` keybindings for toggle and focus to
  `dot_config/herdr/config.toml`.

- [x] 107. Completed tab. Completed tasks newest first, paged so the first screen is fast, with the
  completion date on each line and `u` to reopen one.

  DONE 2026-09-18, merged as herdr-todoist pull request #5, `f08a47d0`. `<Tab>` shows completed tasks
  newest first, one line each with the completion date, paged so the first screen is one request and
  reaching the bottom row asks for the next page. `u` reopens the task under the cursor. The completed
  list is a SEPARATE SCREEN rather than a tenth view, so the numbered actions, the view picker and the
  configured default view are untouched and view 1 is still the unfiltered open list.

  THE API WAS READ, NOT RECALLED, and it differs from every other list endpoint: `since` and `until` are
  BOTH required, the window is capped at three months, paging is by cursor with a default limit of 50,
  and the rows arrive under `items` rather than the `results` key everything else uses.

  TASK 104'S CURSOR WALK COULD NOT SERVE THIS, and the lane proved it rather than assuming: that walk
  loops until the cursor runs out before returning a single row, which is right for a few hundred open
  tasks and wrong for a history without end. A second single-request read path was added, with a test
  asserting the double saw exactly one request.

  THE REVIEW CAUGHT A REAL ONE: a null completion date rejected the WHOLE PAGE, and null is the shape the
  vendor schema actually sends. It is now optional, with the fixtures sending null rather than omitting
  the field. Two smaller fixes: reaching the bottom loaded a page but left the cursor on the old last
  row, needing a second key press; and the bottom-of-history sentence was far wider than the side pane it
  is drawn in, so both it and the hints were shortened and their render test now draws at 32 columns
  rather than 80.

  ONE THING DELIBERATELY NOT FIXED: an account whose recent windows are empty issues up to twelve
  sequential requests before the first draw. The code walks empty windows as designed, and capping it
  mid-key-press would change first-screen behaviour for a case only degenerate accounts hit. The README
  claim was corrected instead, so it now says the one-request first screen holds only when the newest
  window has rows.

- [x] 108. Quick edits in the pane. `x` completes, `X` reopens, `dd` deletes after a confirm, `p` cycles
  priority, `s` takes a natural-language due string (`tomorrow`, `next mon`, `every 2 weeks`) sent as
  Todoist's `due_string`, `l` toggles labels from a picker, `m` moves the task to a project or section
  from a picker, and `a` is Quick Add (`Pay rent tomorrow 9am p1 #Finances @home`). Each is a one-line
  input drawn by the pane, so no editor is entered.

  DONE 2026-09-18, merged as herdr-todoist pull request #6, `f9ea9020`. Eight keys, each a one-line input
  or picker drawn by the pane with no editor entered: complete, reopen, delete behind a confirm, cycle
  priority, a natural-language due string sent as the API's own field, toggle labels from a picker, move
  to a project or section from a picker, and Quick Add taking a whole line of the vendor's syntax.
  Endpoints were read from the API document, where Quick Add turned out to be `/tasks/quick` rather than
  the spelling a guess would reach for.

  PRIORITY NEEDED ADJUDICATING, because the vendor's own document contradicts itself. The authoritative
  prose and the sync view both say 4 is very urgent and 1 is natural, and state that very urgent is p1 on
  clients so p1 returns 4 in the API. One line of the REST update schema claims the opposite. It is
  contradicted everywhere else in the same document and by this repository's existing read model, so it
  was treated as a document error and recorded as such. The cycle steps up in urgency and wraps.

  AFTER EVERY WRITE THE PANE REFETCHES rather than updating a row optimistically, in all eight cases, so
  nothing on screen can diverge from the server and there is no recovery path to get wrong.

  THE REVIEW CAUGHT A SEVERITY-ONE: a label's order field could not parse the null the API documents, so
  the label picker failed outright on any real account holding one unordered label. It is now optional,
  unordered labels sort last, and the fixture was repointed at the real shape with a null-order case.
  Also fixed: a task with no labels opened a titled EMPTY picker that only escape could close, and now
  reports "no labels" in the status line instead.

  Task 105's overlay was generalised to one drawing taking entries plus a cursor, so all three pickers
  are one code path and the confirm and both inputs are one bordered box, held in a single prompt enum so
  only one is ever open. 171 tests, the whole workspace suite in 0.26 seconds, no live API call.

- [x] 109. Comments. `<CR>` on a task opens its detail with the description rendered as markdown and the
  comment thread, and `c` adds a comment from a multi-line box in the pane.

  Closed 2026-09-17, herdr-todoist pull request #7, merged `095c2f55`. `<CR>` on a task now opens a third
  screen carrying the task's title, its description rendered as markdown, and its comment thread oldest
  first; `c` opens a multi-line comment box the pane draws itself, `<C-d>` posts, and the thread is
  re-read afterwards rather than updated optimistically, following the refetch choice task 108 made in
  all eight of its cases. A third screen rather than a tenth view, following task 107's precedent, so
  view 1 is still the unfiltered open list and `view:1` through `view:9` and `default_view` are
  untouched.

  The markdown is a hand-written subset rather than a dependency, because the pane is about 32 columns
  wide. Rendered: headings (bold at one weight, since 32 columns cannot show six levels), bullet lists
  (every marker normalised to one dash), numbered lists keeping their written numbers, blockquotes,
  thematic breaks, bold, italics, inline code, and links, which draw their text followed by the target in
  angle brackets because a pane cannot be clicked. Shown as written: a table, which 32 columns cannot
  hold, and the body of a fenced code block, because reflowing code changes what it says. An underscore
  inside a word is not emphasis, which is what CommonMark says and what keeps `a_variable_name` as typed;
  a test caught the first cut eating it.

  The review found one SEV-1: `<C-d>` could never post, because `next_key` discarded the key modifiers,
  so the send key was unreachable. A fold step in `tui.rs` now maps a Ctrl plus letter key event to its
  control character, pinned by tests on the fold itself and on a plain letter and a non-letter Ctrl key
  passing through unfolded. Three quality findings were also fixed: `q` was missing from the detail
  screen's hint line, the draft module's doc claimed a caret that walks text no key binds, and the detail
  scroll had no lower clamp while its status line said "1 comments".

  The two-boolean screen state in `tui.rs` became one `Showing` enum. 208 tests pass workspace-wide, each
  new suite under 0.08 seconds, every case against a loopback double with no token and no live call. The
  dresden wiring is still owed as a dotfiles pull request.

- [x] 110. Send to the agent. `S` on a task sends a brief (title, description, due, priority, labels, the
  task's URL, and an optional note typed in the pane) into the workspace's agent pane, the way reviewr
  sends line comments, and a comment on the task records that it was handed to an agent and when. It
  never sends on its own.

  Closed 2026-09-18, herdr-todoist pull request #8, merged `a619ce81`. `S` on a task opens the multi-line
  note box task 109 built, `<C-d>` hands a plain-text brief to the workspace's agent pane, and a comment
  on the task then records the hand-off. It never sends on its own.

  The research half is settled, and the answer is that a surface DOES exist, so no honest-alternative
  fallback was needed. reviewr's installed source sends its line comments with
  `herdr pane send-text <pane> <bracketed paste>` followed by `herdr agent focus <pane>`, resolving the
  target from `herdr agent list` filtered to its own workspace and excluding its own pane. Its own notes
  record that herdr 0.7.5 replaced `agent send` with the logical-key `agent send-keys`, while
  `pane send-text` has carried literal-text, no-Enter semantics since 0.7.0. Both commands were verified
  present on the installed herdr 0.9.0, and one read-only `herdr agent list` confirmed the envelope
  shape. Pane selection uses that agent-status surface exactly as intended: a candidate is a row
  `herdr agent list` names an agent for, in this workspace, other than this pane; a pane with no agent is
  not a candidate.

  The review found a SEV-2 that would have been wrong on every single hand-off. `display_agent` is a
  pane's AUTH PROFILE, not its agent, and two panes running different agents can share one; proven live,
  where a codex pane and a claude pane both carried the same profile string. The code preferred
  `display_agent`, so every status line and every persisted Todoist comment would have named the auth
  profile instead of the agent. The order is now name, then agent, then display_agent, with a fixture
  taken from the live envelope shape that fails under the old order. Two quality findings were also
  fixed: a refused send put the entire bracketed-paste brief, escape sequences and all, into a status
  line a side pane draws at about 32 columns, and the list screen's draft branch was keyed on the
  presence of a draft rather than on the Note prompt, which would have quietly no-opped a future
  comment-on-list.

  The Todoist v1 task object has no `url` field: the migration guide says the REST v2 `url` "has been
  removed", so the brief's URL is built from the task id per the documented form
  (`https://app.todoist.com/app/task/<v2_id>`) rather than read from a field that is not there.

  The send-then-comment ordering was mutation-checked by swapping the two calls, which reddened three
  specs. 177 tests pass, no test reaches Todoist or herdr, and every herdr call in tests goes through an
  injected closure. The dresden wiring is still owed as a dotfiles pull request.

- [x] 111. Enter Neovim from the pane. `e` on a task runs the configured editor command (default `nvim`)
  in the same pane with `+"Todoist task <id>"`, blocks until it exits, then refreshes the list; the
  pane's own multi-line box is the fallback when no editor is configured. This is the seam with task 115.

  Closed 2026-09-18, herdr-todoist pull request #9, merged `e109e54`. `e` on a task runs the configured
  editor in this pane, waits for it, and re-reads the list once it has gone. The default command is
  todoist.nvim's own documented entry point, `nvim +"Todoist task <id>"`, so the herdr pane and the
  Neovim plugin are two halves of one workflow.

  `editor` is argv, a list rather than a command line: the program is one entry and each argument is its
  own, so nothing is split on spaces and a path containing one needs no quoting. Unset means `nvim`,
  chosen over unset-means-box because the ledger names nvim as the default and the alternative would make
  the documented default unreachable without config; `editor = []` is the explicit off, and there `e`
  opens the pane's own multi-line box over the task, first line the content and the lines under it the
  description. A refused save keeps the box open with every line still in it, which matters because the
  operator has just typed.

  The suspend reuses the seam the pane already had rather than growing a second one: `ratatui::restore()`
  before the child, `ratatui::init()` after, gated so the early-return path that never left the terminal
  does not try to re-enter it. That gate was a review finding. The restore therefore happens before the
  spawn is even attempted, so a binary that does not exist never touches the terminal and comes back to a
  drawn pane with its name in the status line. The exit code is ignored on purpose: someone who quit in a
  hurry may still have saved, so the re-read is what settles the task.

  This branch was cut before task 110 landed and both sides had added a multi-line box to the list
  screen, so the merge was resolved by hand: every `Prompt` match now carries both the `Note` and the
  `Edit` variant, and the list screen's draft branch keeps main's explicit keying on the variant rather
  than the earlier test for the presence of a draft, routing the send key per variant. That keeps the
  SEV-3 task 110 had already fixed. The hint line holds both new keys inside 32 columns by spending the
  view numbers, which the picker lists anyway. 238 tests, fmt and clippy all green on the merge commit,
  and the refresh-after-exit mutation reddens exactly one spec.

  The `apply` and `edit` test modules moved into `apply/tests.rs` and `edit/tests.rs`, the pattern
  `detail` already used, because both files crossed the 500-line cap with the new tests. Pure moves, no
  assertion changed.

- [x] 112. Pretty UI. Nerd Font icons for priority, due state (overdue, today, upcoming, none), labels
  and recurring tasks, a palette that follows reviewr's theme names so both panes match, and a
  plain-ASCII fallback set by config.

  Closed 2026-09-18, herdr-todoist pull request #10, merged `b0100b96`. Marks for priority, due state,
  recurrence and labels, a palette that matches reviewr, and a plain-ASCII set for a terminal without a
  Nerd Font.

  The palette half was research and it landed exactly as the ledger asked. reviewr's installed source
  resolves a theme NAME to a compiled-in palette, taking the name from its own config and saying in its
  own doc comment that "names match herdr's so the value a user copies from their herdr config resolves
  to the same palette". It reads no herdr-provided theme at runtime and uses no terminal ANSI slots, and
  it explicitly refuses `terminal` as a palette name. This pane now carries the same slot names, the same
  eight anchor values, the same derivation fractions and the same pinned literals for the default theme,
  so a colour named `blue` in the Todoist pane IS the colour named `blue` in reviewr. Six
  diff-and-search-only slots were dropped as having nothing to paint here. The config key is `theme`,
  spelled the same, and an unknown name is refused with the names that do resolve, because this
  repository's config is strict everywhere else even though reviewr logs and falls back.

  The 32-column problem was solved by making the marks LEAD and the title follow, so the pane's width
  costs the end of the TITLE, elided, and never a badge. Labels are counted rather than named, since
  three names cannot fit, and the detail screen still names them. The longest realistic task was drawn
  into a 32-column backend and measured in terminal cells rather than counted in characters.

  ONE REAL BUG was found and fixed in its own commit, and it was already live before this task: the
  delete confirm, the edit box and the detail screen took a task's words from its DRAWN LINE rather than
  from its title, so a save out of the edit box wrote the indentation, the due date and the labels back
  into the task's own content. Putting badges in front of the title would have made it far worse. All
  three now read the row's own content field.

  The lane reported its mutation check honestly rather than to the brief: flipping the
  overdue-versus-today boundary reddens five specs, not the one the brief asked for, because the
  line-building specs read real due states too. 215 tests pass.

- [x] 113. Cache and background refresh. The pane opens from a local cache so the first render is
  instant, refreshes on an interval and after every write, and marks itself stale with the cache age when
  the network is down. Writes made offline are queued and replayed in order once a refresh succeeds.

  Closed 2026-09-18, herdr-todoist pull request #11, merged `bef263d7`. The pane opens from a per-view
  local cache before any request, refreshes on a configurable interval and after every write, marks
  itself stale with the cache age when the network is down, and queues writes made offline to a file that
  survives a restart, replaying them sequentially oldest first once a read reaches the API.

  Nothing is drawn optimistically, which keeps the rule every earlier feature in this plugin follows. A
  queued write leaves its row saying what the API last said and adds a `+` mark, and the status line
  counts what is waiting, so it reads `stale 5m +2`. That is how the operator tells a queued change from
  a confirmed one, and it was checked at 32 columns like every other drawing here.

  Both review findings were SEV-1 and both were the data-loss shape the brief warned about. A replay
  DROPPED THE WHOLE OFFLINE QUEUE on a rate limit, a 5xx or any token trouble, because it treated every
  failure as a refusal; the fault type is now split three ways, so a replay stops and keeps everything on
  a transient fault and drops only a write the API actually refused. Separately, `refresh_seconds = 0`
  removed the opening READ rather than just the interval, so a pane configured not to poll never loaded
  at all; the opening read is now its own step that runs before the loop regardless.

  Two quality findings were also fixed. The waiting mark was deduplicated by sniffing the rendered line,
  so a task titled `+1 follow up` read as already marked and never got one, which is the same class of
  bug task 112 found and fixed; the row now carries the flag as state. And two `too_many_arguments`
  allowances threading the cache and queue through were removed by bundling the pane's state into one
  struct.

  The lane reported its mutation counts honestly rather than to the brief: the replay-order mutation
  reddens four specs, not one. 293 tests pass across the workspace in 0.4 seconds, the slowest single
  test at 0.08 seconds, with no sleeps and no wall-clock waits.

  **herdr-todoist is now complete: tasks 103 through 113 are all merged. With todoist.nvim complete as
  well, PRIORITY 4 IS DONE, all twenty-two tasks.** What remains for both is the dresden wiring, which is
  a dotfiles pull request, and their lack of any CI, filed separately.

- [x] 114. DONE 2026-09-17. Create the `webdavis/todoist.nvim` repository with the Lua client and the
  token boundary: async through `vim.system` and `curl`, no blocking calls on the UI thread, errors
  through `vim.notify` with a retry, `:checkhealth todoist` that proves the token resolves and one
  request succeeds without printing it, a `lazy.nvim` spec in `dot_config/nvim/lua/plugins/`, and busted
  specs run the way the other custom plugins run theirs.

  `webdavis/todoist.nvim` created public; PR #1 merged `77c822ed`. Repository shape, style config, MIT
  licence and the headless runner copied from the operator's own `pns.nvim`. One curl process per request
  through `vim.system` with a scheduled callback and NO synchronous variant. THE TOKEN NEVER ENTERS THE
  PROCESS TABLE: it reaches curl on its STANDARD INPUT as a configuration file rather than as an
  argument, and only `token_command` or `token_env` can produce it. Errors are typed, carry the API's own
  wording, raise one notification unless the caller asks for quiet, and a TRANSIENT failure is retried
  once (Retry-After honoured, bounded at ten seconds) while a REFUSED TOKEN is not retried and is dropped
  from the cache. Endpoint facts were read from the vendor documentation rather than recalled, and the
  two endpoints task 115 needs were built for it. checkhealth was proven against a local double: it
  reports that the token resolved and that one request succeeded, and prints nothing about the token.

  TWO REVIEW FINDINGS WERE REAL. The documented async contract was BROKEN ON EVERY PRE-SPAWN ERROR PATH,
  because a token-resolution or spawn failure ran the callback synchronously inside the request; that
  path is now scheduled like the rest. And NOTHING PINNED THE RETRY LOOP: deleting it left all 29 specs
  green. Two specs now count spawns through a fake, and the fix was verified BY MUTATION.

- [x] 115. DONE 2026-09-17. `:Todoist task <id>`, the whole task as a buffer. A scratch buffer with the
  fields as a small header (content, due string, priority, labels, project, section) and the description
  as a markdown body below it; `:w` validates and writes the task back, `:q` on an unwritten buffer asks,
  and the buffer reports the API's message on a rejected write. This is what the herdr pane enters (task
  111\) and what `<CR>` opens in every list below.

  PR #2 merged `c77bcb59`. The header is MARKDOWN FRONTMATTER, six single-line fields between two fences
  with the description as the markdown body, chosen because a person already knows the shape and can
  retype it by hand. Round-trip is exact and a re-parsed render produces no fields to send. Local
  refusals each name their line; everything else, the due string above all, is left to the API so its own
  wording surfaces, and a changed project or section is REFUSED rather than silently dropped because
  moving a task is a different call. THE WRITE SENDS ONLY WHAT CHANGED, so an unchanged due string is
  never sent and a recurrence is never reparsed away. The buffer stays modified until the API answers, so
  a rejected write leaves the operator's text where they can fix it.

  THE SEAM WAS CHECKED FOR REAL, not just in process: in a fresh Neovim with nothing else loaded, the
  launch form the herdr edit key uses opened the buffer, `:q` after an edit gave Neovim's own
  unsaved-changes error, and `:w` sent exactly the one changed field. That is the path task 111 and every
  list's return key depend on. One SEV-1 was a genuine data-loss bug: reopening a task SILENTLY DISCARDED
  unsaved edits and marked the buffer written. Two SEV-2s were also real: buffer lookup matched names by
  substring so one task could hijack another's buffer, and the write autocmd was re-registered on every
  open so one save fired one request per open. 51 specs, 0.58 seconds.

- [x] 116. DONE 2026-09-17. `:Todoist` and named views on keymaps. `:Todoist` lists every open task
  grouped by project and section; `setup({ views = { today = "today | overdue" } })` declares named
  views, `:Todoist today` opens one, and `require("todoist").open("today")` is what a keymap calls. View
  names match the herdr plugin's by convention so the same word opens the same list in both.

  PR #3 merged `3569ebcb`. `:Todoist` with no argument lists every open task, a declared name opens that
  view, the declared names complete alongside `task`, and the public open function is what a keymap calls
  (an absent or empty name means every open task, and it returns the buffer). Return on a task line opens
  task 115's buffer through the existing module rather than duplicating it, and `R` re-asks the API for
  the view on screen. A LINE MAPS BACK TO A TASK THROUGH A TABLE the renderer builds, never by parsing
  display text, so the rendering can change freely and a heading or blank line says there is no task here
  rather than opening the nearest one. A refused filter is drawn in the buffer in the API's own wording
  under its own heading while an empty result says there are no tasks, so the two answers look different.
  One review finding was a real race: a LATE ANSWER FROM A PREVIOUS VIEW overwrote the view now on
  screen, fixed by guarding the callback on the view still being shown, with a loopback regression test
  that delays one answer behind a faster one. One SEV-3 was skipped for a reason worth keeping: pinning
  the list envelope and cursor parameter against the real API needs a live authenticated call, which the
  standing rules forbid, so it stays unpinned until the operator or a token-holding session runs it. 69
  specs, 1.05 seconds, stable across four runs.

- [x] 117. A toggleable sidebar inside Neovim. `:Todoist toggle` opens a fixed-width split on the
  configured side showing one view (default `today`), closes it on a second call, and survives layout
  changes the way nvim-tree and neo-tree do.

  DONE 2026-09-18, todoist.nvim pull request #4, merged `c646c8c7`. `:Todoist toggle` opens one view as a
  fixed-width vertical split on the configured side, closes it on a second call, and holds its width
  through layout changes.

  THE MECHANISM WAS READ FROM THE TWO PLUGINS THAT ALREADY SOLVED IT rather than invented. Both nvim-tree
  and neo-tree set the window-fixed-width and window-fixed-height options on the window they create, and
  NEITHER has a width-restoring autocommand: a grep across both trees for the resize and new-window
  events returns nothing but one tab-bookkeeping handler. Measured on Neovim 0.12.5, a window with the
  fixed-width option keeps its absolute width through a vertical split, a horizontal split, an equalizing
  command, and a terminal width change from 200 to 100 to 60 and back.

  ONE PLACE THE OPTION DOES NOT HOLD, which the lane found and handled: while the sidebar is the ONLY
  window in its tabpage, Neovim must give it every column, so the next split divides evenly and the
  sidebar comes back at half the screen. A new-window and resize autocommand puts the configured width
  back whenever the tabpage has company. The new-window event is in there because the resize event does
  not fire during a headless script, so the resize event alone left that case unpinned.

  The sidebar's open state IS the window, held as a window-local flag, so closing it with a quit command
  behind the plugin's back cannot make the plugin and Neovim disagree. The buffer-fixed option keeps
  other files out of the sidebar, and THE REVIEW CAUGHT WHAT THAT COST: opening a task by id from inside
  the sidebar died with a raw Neovim error, because the escape path was wired into only one of the two
  callers. The guard moved to the single choke point both callers route through, reproduced before and
  confirmed after, with two new specs covering it.

  Defaults are the left side at 40 columns: left because every file tree in this ecosystem sits there, so
  the operator's window-movement habits survive having both open, and 40 because that is what a task line
  plus its due date, priority and one label needs before it truncates.

  Each mechanism was mutation-checked: removing the fixed-width option, the fixed-buffer option, or the
  autocommand each reddens exactly one spec. 81 specs pass in 1.53 seconds with no network and no token.

  OPERATOR OWES: the dresden wiring is still a dotfiles pull request and is not filed. It would add the
  lazy.nvim spec and the toggle keymap.

- [x] 118. Completed view. `:Todoist completed`, newest first, paged, completion date on each line, `u`
  reopens.

  Closed 2026-09-17, todoist.nvim pull request #6, merged `17eb066f`. `:Todoist completed` draws the
  completed history newest first, one page at a time, each line leading with the completion date, and `u`
  reopens the task under the cursor. Every API fact was re-verified against the vendor OpenAPI document
  rather than trusted: the path is `/api/v1/tasks/completed/by_completion_date`, `since` and `until` are
  both required with `since` inclusive and `until` exclusive, the window is capped at three months,
  paging is by cursor with a default limit of 50, the rows arrive under `items` rather than `results`,
  reopen is `POST /api/v1/tasks/{task_id}/reopen` with no body, and `completed_at` is nullable. All seven
  hold. This repository's cursor walk had the same eager shape the sibling plugin's did, looping until
  the cursor ran out before answering, so it could not serve this screen; a second single-request read
  path was added beside it. The walk itself is pure state over the pages handed to it, stepping twelve
  ninety-day windows (about three years) and skipping empty ones until a row appears. Two review findings
  were fixed: a row the endpoint repeats across a page boundary was listed twice, and a stale walk's
  answer could clear the in-flight flag belonging to the walk that replaced it. Proven in a fresh Neovim
  against a loopback double as well as in the specs, and mutation-checked on the ordering comparator and
  the paging boundary, each reddening exactly one spec.

- [x] 119. Quick edits in the list. `x`, `X`, `dd`, `p`, `s`, `l`, `m` and `a` do what task 108's keys
  do, so a hand that learned one plugin knows the other, and `u` undoes the last complete or reopen
  within the session.

  Closed 2026-09-18, todoist.nvim pull request #7, merged `5ebc339f`. The eight keys `x`, `X`, `dd`, `p`,
  `s`, `l`, `m` and `a` now work in the list buffer, and `u` reverses the last complete or reopen within
  the session. The key set was read out of the sibling plugin's merged task 108 source rather than from
  the ledger text, so a hand that learned one plugin knows the other: `x`, `X` and `p` act at once, `dd`
  confirms first, `s` and `a` take a typed line, and `l` and `m` offer a picker. `list.lua` now keeps the
  API's own task objects by id, so a key never parses the rendering to find out what it is acting on.

  One key could not carry over. In the herdr pane the confirm IS the second `d`, because that pane reads
  keys one at a time; in Neovim `dd` is already one mapping, so the confirm became its own question
  through `vim.fn.confirm`, defaulting to No. That is the only interaction that differs, and only in how
  the yes is given. `l` also toggles one label per press where the pane keeps its picker open for
  several, because `vim.ui.select` closes on a choice and reopening it behind the operator would fight
  whatever picker they have configured.

  Undo is one level, not a stack, and covers only a complete and a reopen, the one pair with an exact
  opposite. A `p` press does not cost the undo of the `x` before it. A reversal the API refuses keeps the
  write remembered so `u` can be pressed again, and re-reads the view so the buffer holds what the server
  holds; nothing is ever drawn optimistically. The two `u` bindings coexist because they live in two
  different buffers, `todoist://list` and `todoist://completed`, each with its own buffer-local map, and
  both buffers are nomodifiable so neither steals Vim's own undo.

  The review found a SEV-1: every successful write cleared the undo regardless of whether it was
  undoable, so a delete, a priority cycle, a schedule, a label toggle or a move silently threw away the
  remembered complete. Fixed and covered by a new case. It also found the null bug this repository had
  already been warned about: `task.labels or {}` does not survive a JSON null, because `vim.json.decode`
  maps null to the truthy `vim.NIL`. Fixed to the `type(...) == "table"` idiom `task_format.lua` already
  used.

  147 specs pass in 1.77 seconds, the two new files adding 21 cases, and both required mutations reddened
  exactly one spec each. The dresden wiring is still owed as a dotfiles pull request.

  VENDOR DOCUMENTATION CONTRADICTS ITSELF ON TODOIST PRIORITY, found while verifying the API rather than
  trusting it. In https://developer.todoist.com/openapi.json the update-task parameter description says
  priority is "1-4, where 1 is highest", while the same document's task schema and the Sync tables say "4
  for very urgent and 1 for natural". The app agrees with the schema. Both plugins follow the schema,
  which is what the existing rendering already did, so nothing is wrong in this repository's code; this
  is recorded so the next lane that reads that parameter description does not believe it. This is the
  third vendor document inconsistency found this week: task 107 found the completed endpoints using
  different field names than the open ones, and task 108 found one line of the update schema
  contradicting the rest.

- [x] 120. Capture a task from code. `:Todoist capture` creates a task whose description carries
  `path:line` and the repository name from the current buffer, a visual selection of a `TODO` or `FIXME`
  comment becomes the task's content, a task with a location shows a location icon in the list, and `gd`
  on it jumps to the file and line.

  Closed 2026-09-17, todoist.nvim pull request #5, merged `25651a3`. `:Todoist capture` in todoist.nvim
  now makes a Todoist task out of the code in front of the cursor: the description carries `path:line`
  and the repository name read from the current buffer, and a visual selection over a `TODO` or `FIXME`
  comment becomes the task's content with the marker word stripped. A task that carries a location shows
  a location icon in the list, and `gd` on that row jumps to the file and line, opening the file when it
  is not already in a buffer. Two defects the review found were fixed before merge: a bare number in the
  location field parsed as a path, and the success notice was tangled with the refusal path. Proven with
  the repository's own runner, `nvim --headless --clean -l tests/run.lua`, plus stylua and luacheck, and
  mutation-checked by breaking the location parser and watching exactly one spec redden. The dresden
  wiring (the lazy.nvim spec, the token entry and the keymaps) is still owed as a dotfiles pull request.

- [x] 121. Picker integration. A source for `fzf-lua` (the operator's picker) and a generic
  `vim.ui.select` path for everything else: fuzzy-search open tasks, `<CR>` opens the task buffer,
  `<C-x>` completes from the picker, and the picker respects the current view's filter.

  Closed 2026-09-18, todoist.nvim pull request #8, merged `ec6b54bb`. `:Todoist pick [<view>]` and
  `require("todoist").pick(name)` fuzzy-search open tasks through fzf-lua when it is installed and
  through `vim.ui.select` otherwise. `<CR>` opens task 115's task buffer for the picked task and `<C-x>`
  completes it, routed through task 119's own complete call so that task 119's `u` reverses a complete
  made from inside the picker; that routing is pinned by a mutation test, since calling the client
  directly would have silently made one of the two ways to complete a task unundoable.

  The filter clause was decided deliberately. With no argument the picker follows what is on screen
  through a new `list.current_spec()`, which answers only when the list buffer sits in a window of the
  CURRENT tabpage: a filtered view searches inside that filter, the unfiltered list searches every open
  task, and with no visible list buffer it searches every open task, which is the same answer a bare
  `:Todoist` gives. A hidden list buffer does not steer the search, because it is not what the operator
  is looking at. The prompt always carries the view's own title, so a filtered search says which filter
  it is inside, and an empty result says "no open tasks in Todoist: today (today | overdue)" rather than
  opening an empty picker, so a filter that matched nothing cannot be mistaken for an empty account.

  fzf-lua stays optional: `pcall(require, "fzf-lua")` at call time, never at load, so installing it later
  needs no restart and its absence is not an error. A new `picker` option pins a path (`auto`, `fzf-lua`
  or `select`), and an unrecognised value warns once rather than silently meaning auto. The fzf-lua API
  was read from that repository at commit 02bc882f, since the project publishes no releases and a commit
  is the only pin available.

  The review found a SEV-1 whose root cause was older than this task: `list.open` moved the current
  window to the list buffer as part of loading it, so a `<C-x>` from the picker hijacked whatever window
  the operator was in. `list.open` was split into `load` (fetch and redraw, no window call) and `open`
  (window move plus load), and refresh now calls `load`. A regression case opens a list, swaps the window
  to a scratch buffer, completes from the picker and asserts the current buffer is unchanged; it was
  verified to fail without the fix.

  The null bug was found a fourth time and fixed: `list_format`'s task line used `task.labels or {}`,
  which hands `ipairs` a `vim.NIL` userdata when the API answers with a JSON null. 164 specs pass, 15 of
  them new, the picker spec running in under a hundredth of a second, and three mutations each reddened
  exactly one spec. The dresden wiring is still owed as a dotfiles pull request, and should record that
  fzf-lua commit as what it was proven against.

- [x] 122. Subtasks as a fold tree. Tasks with children render as a tree, `za` folds a task's subtasks,
  `>` and `<` indent a task under the one above it or promote it, and completing a parent asks before
  completing its open children.

  Closed 2026-09-18, todoist.nvim pull request #9, merged `a4289d6`. Tasks with children render as a tree
  at two spaces per level, `za` folds a task's whole subtree, `>` and `<` reparent, and `x` on a parent
  asks before taking its open subtasks.

  Four API facts were established by reading the document rather than assuming. The parent field is
  `parent_id` on `ItemSyncView`, `anyOf: [string, null]` and in the schema's required list, so always
  present and often null; the completed endpoints return the same object, so it reads identically there,
  and the difference those endpoints carry is the envelope, which the plugin already handled. Reparenting
  is a MOVE and not an update: the update body has no `parent_id` at all, and `POST /tasks/{id}/move` is
  the only endpoint that takes one. Completing is ONE call and not a loop, because
  `POST /tasks/{id}/close` is documented as marking a task complete along with its subtasks, so the
  server cascades and the confirm is a warning rather than a plan. The priority contradiction found by
  earlier lanes is still in the document, unchanged, and the plugin still follows the schema rather than
  the parameter description.

  The renderer buckets only the roots of each tree and walks each root's descendants in place, building
  the line-to-id table on the same walk. That is what kept every other feature working from an indented
  row: `<CR>`, `R`, `gd` and the eight quick-edit keys resolve a row through that table rather than by
  reading its text, so none of them needed changing, and a spec pins it. Task 117's sidebar was
  untouched, because indent is added to the left of the content and the location icon still goes last.

  The review found a SEV-1: `>` on the first task of any project or section silently moved it into the
  PREVIOUS project, because the row above was taken without checking that it belonged to the same group.
  Fixed at the root in `tree.indent_to`, which now refuses and says why. A SEV-2 was also fixed, where a
  folded badge counted direct children while the fold hid the whole subtree.

  199 specs pass in 2.20 seconds, `tree_spec` being 14 pure cases in under a hundredth of a second, and
  two mutations each reddened exactly the expected specs. The null field is pinned twice, once by every
  top-level fixture carrying `parent_id = vim.NIL` and once by asserting a flat list renders byte for
  byte the same with null parents as without.

  Merged by hand: task 121's picker had landed on main and touched the same two functions.
  `list_format.render` keeps this branch's `collapsed` argument and main's public `names_by_id`, and
  `quick_edit.complete` keeps the subtask confirm and then calls main's extracted `complete_task`, so a
  complete made from the list still goes through the one write that `u` remembers.

  COMPLETING A PARENT FROM THE PICKER DOES NOT ASK, found while resolving this merge. `x` in the list
  asks before completing a parent that has open subtasks, because `POST /tasks/{id}/close` cascades to
  subtasks server side. `<C-x>` from task 121's picker calls the write directly and so asks nothing,
  which means completing a parent from the picker silently closes its subtasks. The picker builds its
  entries from tasks rather than from the list buffer, so it has no tree to count children in; giving it
  one is its own change and is filed as task 158.

  MY OWN MISTAKE: A LANE BRANCHED OFF A STALE MAIN. This branch's `herdr worktree create` inherited the
  launching checkout's HEAD, and the launching checkout's local `main` sat one merge behind because only
  `git fetch` had been run, never `git merge --ff-only`, so this lane branched from before task 121's
  picker had landed and the ship stage hit a merge conflict resolved by hand above. Fix, now part of the
  launch routine: fast-forward the target repository's local `main` immediately before launching any lane
  into it, in every repository, not just dotfiles. Task 111's earlier conflict was not this cause: that
  branch was cut before task 110 existed and genuinely raced it.

- [x] 123. Send to the agent from Neovim. `S` on a task sends the same brief as task 110 into the
  workspace's agent pane through the `herdr` CLI when `HERDR_ENV` is set, and copies it to the clipboard
  with a notice otherwise.

  Closed 2026-09-18, todoist.nvim pull request #10, merged `f828d362`. `S` on a task hands it to the
  workspace's agent pane through the herdr CLI, and falls back to the clipboard with a notice whenever
  herdr cannot take it. The brief is herdr-todoist's own text character for character, asserted against
  the sibling's format in a spec, so a hand that learned one plugin knows the other. Both facts the
  sibling had learned the hard way were carried over rather than rediscovered: the URL is built from the
  task id, because the v1 task object has no `url` field, and the agent is named from `agent` rather than
  `display_agent`, pinned by a fixture in which two panes share one auth profile.

  The clipboard half is this plugin's alone. Both the unnamed register and `+` are written, unnamed first
  and always, so a machine with no clipboard provider still gets the brief; where there is no provider
  the notice says so at warning level rather than reporting a whole copy. Herdr set but unusable falls
  back to the clipboard rather than failing, in all three of its cases (binary missing or listing
  refused, no agent pane in this workspace, refused send), each naming itself before the same clipboard
  sentence. A refused `agent focus` deliberately does NOT fall back, because the text is already in the
  agent's input and reporting a hand-off that happened as one that did not would be worse.

  The comment is written on the agent path and not on the clipboard path, because a clipboard copy is not
  a hand-off and a comment saying otherwise would be a false record on the task. The two paths cannot be
  confused, because every clipboard notice ends in "no hand-off comment written". The comment names this
  plugin rather than copying the sibling's wording, so the record says honestly which of the two wrote
  it.

  The review found a SEV-1: `vim.system` raises synchronously for a missing binary, so a stale
  `HERDR_BIN_PATH` threw and LOST the brief rather than falling back. The spawn is now wrapped and routed
  through the same path every other refusal uses. Its second finding was sharper than the fix: every spec
  doubled the host, which is exactly where the only defect lived, so a case was added that exercises the
  real host against a nonexistent binary. A third dropped a `name` field the code asked for that herdr's
  agent listing does not emit.

  214 specs pass, the 14 new ones together in under a hundredth of a second, and both required mutations
  reddened exactly one spec each. `HERDR_SOCKET_PATH` and `HERDR_BIN_PATH` were unset for every test run
  and nothing in the live herdr session was touched.

- [x] 124. Statusline component and due reminders. `require("todoist").status()` returns a short string
  (`3 due, 1 overdue`) for lualine or a custom statusline, and an opt-in reminder raises `vim.notify`
  when a task with a time comes due while Neovim is open.

  Closed 2026-09-18, todoist.nvim pull request #11, merged `f52aee60`. `require("todoist").status()`
  returns a short statusline string and an opt-in reminder raises `vim.notify` when a task with a time
  comes due while Neovim is open. The plugin had no periodic refresh at all before this, so this adds the
  ONE poller, and `status()`, the reminder and the fetch all read a single stored task set. `status()`
  does no work on a redraw: it returns a string built when the last answer arrived, which is the
  constraint that decided the whole design, since lualine evaluates a component many times a second.

  The string has three readings and never leaves a stale number standing: empty before any fetch has
  finished and whenever nothing is due, since an empty component simply draws nothing; `todoist !` when
  the last fetch failed or the token would not resolve; otherwise the count. The poller starts lazily, so
  a configuration that neither puts the component on a statusline nor turns reminders on polls nothing,
  and it is stopped on `VimLeavePre` so nothing outlives the editor.

  The review found a SEV-1 worth recording because it is a class of bug, not a typo: `clock()` was AN
  HOUR WRONG UNDER DAYLIGHT SAVING, so every task stamped in UTC read as overdue an hour early. The cause
  was an `os.time` round trip over a broken-down UTC time with `isdst` forced false, which re-interprets
  it as local standard time. It was reproduced as a measurement, minus 25200 against the correct minus
  21600 under America/Denver, fixed by differencing the two civil stamps directly, and pinned by a spec
  that compares against `os.date("%z")` as an independent read of the same value.

  Two more findings were fixed: `start()` registered a fresh `VimLeavePre` autocommand on every
  stop-and-start cycle and had no way to refuse restarting once Neovim was exiting, so a redraw after
  `VimLeavePre` could undo the stop and outlive `:qa`; and a failed fetch cleared the statusline but left
  the stale task set behind the public counts. Both were pinned by specs verified to fail against the
  pre-fix code.

  246 specs pass in 2.19 seconds, 32 of them new, every one with the clock injected and the fetch
  doubled, no sleeps and no network. Both required mutations reddened exactly one spec each.

  **todoist.nvim is now complete: tasks 114 through 124 are all merged.** What remains for it is the
  dresden wiring, which is a dotfiles pull request, and its lack of any CI, filed separately.

### Daily operations tools

Two tools filed 2026-09-17 from the operator's own pain points, approved the same day.

- [x] 125. `morning`, a Rust tool with a `/morning` command in every harness. One command that answers
  "where do I start today": the last apply's result and date from
  `~/.local/state/chezmoi-apply/latest.apply.log`, every apply the ledger says the operator owes, the
  open pull requests with their CI state (through `gh`), the newest overnight recap, the ledger's
  operator-owned items, and today's Todoist tasks through `td`. It prints one framed page and exits; it
  never applies, merges or edits anything. Its own cargo workspace at the repository root like the other
  four, installed to `~/.cargo/bin`, declared in `.chezmoidata/rust_tools.yaml`, and it never hardcodes
  this repository's path (the ledger path and the log path are config). Ships with a `/morning` command
  for Claude Code (`private_dot_claude/commands/`), Codex and hermes that runs the binary and hands the
  page to the agent as the day's brief, so the operator's first message of the day is "morning" in any
  harness. Operator ruling 2026-09-17: a binary plus an agent command, not a `just` recipe.

  Closed 2026-09-18, merged as [PR #784](https://github.com/webdavis/dotfiles/pull/784), `5bbed3a5`.
  `morning` is a sixth Rust workspace at the repository root, four crates in the shape the clean-code
  skill sets out, with a 26-line `main.rs`. It prints one framed page and exits: the last apply's result
  and finish time, the applies the ledger owes, the open pull requests with their CI state, the newest
  overnight recap, the operator's own items, and today's tasks. It reads and prints only, applies and
  merges and edits nothing, and reads no secret.

  The design problem was that every source can be absent, unauthenticated, broken or slow, and a command
  answering "where do I start today" is worthless if it blocks for ninety seconds. Sources are therefore
  read CONCURRENTLY and each is killed at its own deadline, and a source that is missing, unconfigured,
  failing or too slow gets a section that SAYS SO rather than being silently dropped. Every path is named
  in `~/.config/morning/config.toml`, the ledger path included, so the tool works on a machine where this
  repository does not exist, which is the rule for every tool here.

  Four review findings were fixed, each a real misreading rather than a style point: ledger continuation
  lines were absorbing unindented prose that followed them; the pull-request reader called a run green
  when its CI was not; the Today section was spending its row cap on metadata instead of tasks; and a
  negative `timeout_seconds` panicked. A fifth removed a type that was a byte-for-byte copy of another.

  Proven end to end on the real machine rather than only in tests: the rendered builder compiled and
  installed the binary, and the binary run against the rendered config produced a page from the real
  apply transcript, the real ledger, real `gh` output and real `td today` output. `~/.cargo/bin/morning`
  is installed and working now; the apply is still owed for the config file and the three harness
  commands.

  OPEN QUESTION RAISED, not answered: the tool spawns `gh` rather than `gh-axi`, and ledger task 145 is
  the unanswered operator decision on exactly that, whether the gh-axi rule binds agents only or shipped
  products too. The lane picked the one it could defend and flagged it rather than deciding for the
  operator.

- [x] 126. Quiet follows the calendar. pns turns `pns quiet` on for the duration of a Google Calendar
  event marked busy and off when it ends, so a meeting never gets a banner and the operator never toggles
  quiet by hand. The calendar is read through a producer command the config names (the operator's `gog`
  CLI is the first implementer), polled by the pns daemon on its own clock, read-only, and a manual
  `pns quiet` always wins over the calendar. Ships off by default with one `[quiet.calendar]` table.
  Approved 2026-09-17.

  DONE 2026-09-20: the pns side (the opt-in `[quiet.calendar]` table, the leased `mute calendar` job, the
  calendar adapter and the hand-set-mute-wins rule) had already landed with the mute rename; the producer
  landed as [PR #867](https://github.com/webdavis/dotfiles/pull/867) and its fix round
  [PR #869](https://github.com/webdavis/dotfiles/pull/869), merged `af23b6813`:
  `~/.local/libexec/pns/calendar-busy-window.sh` runs `gog calendar events` read-only for the next hour
  and emits only start, end and busy per timed event (transparent and cancelled events are free, all-day
  events are skipped), ten bashunit cases over a fake gog. The table ships with `enabled = false` until
  the operator runs the script by hand once, because gog's keychain read cannot be exercised by an agent;
  flipping `enabled` is a one-line values change after that run.

  Amended 2026-09-20 by two operator rulings: pns ships no bash, and the calendar reader is a compiled-in
  source inside pns rather than a producer command. The bash producer is retired (the operator ran it
  once by hand, and the deployed copy is already gone from `~/.local/libexec/pns/`). Two pull requests
  replace it: the first adds `[quiet.calendar] type = "google"`, a Google Calendar freeBusy reader over
  pns's own HTTP client with a refresh-token OAuth exchange, credentials as KeePassXC-backed values in
  `config-values.toml`, a one-hour window and the same fail-closed rule as the command source, leaving
  the operator's values inert; the second adds the one-time consent verb that mints the refresh token,
  deletes the bash producer and its bashunit test, and arms the poll. `command` stays as the escape hatch
  for any other calendar.

- [x] 132. `pns resume`, the "where was I" answer. A subcommand that prints, framed, the herdr workspace
  the operator was last in, the agent pane waiting on them if any, the branch and worktree of that pane,
  and the last command it ran, read from the state pns already keeps plus the herdr CLI. Plain
  `pns resume` prints to the terminal; `pns resume --notify` sends the same page through the engine as a
  banner (and the phone when away). The unlock automation is nothing more than a LaunchAgent or Shortcut
  that calls `pns resume --notify`; the subcommand is the API and the automation only uses it. Operator
  ruling 2026-09-17.

  DONE 2026-09-17: `pns resume` answers "where was I": it prints, framed in the house style, the focused
  herdr workspace, the session waiting on the operator with its branch and worktree, and the newest
  command the shell notifier timed, all read from the state pns already keeps plus one
  `herdr workspace list`. `--json` writes the same answers as a `pns.resume/1` object whose fields carry
  the page's own names, and `--notify` renders the page plain and submits it through the same internal
  path the GitHub poll uses, as an `observation` from the `pns` producer, so the engine's existing
  presence gate decides banner-only or banner-and-phone and no new delivery rule was added. The waiting
  session is the one with the newest `blocked_since`, where the stale escalation takes the oldest, and a
  machine with nothing waiting says so in one line instead of printing an empty section. Two read-only
  store queries were added for it, one over `sessions` and one over the newest `shell` ledger event, each
  reading every failure as "not known" rather than refusing to print, alongside a parser for herdr's
  workspace listing that answers each workspace's label, focus and checkout path. No config key was
  added. The command surface is documented at `pns/docs/specs/resume.md` with a glossary row beside it,
  and thirteen tests pin the page, the JSON fields, the single recorded event and the usage text. The
  unlock automation stays out of scope: it is a caller of `--notify`, and the subcommand is the API.
  [PR #841](https://github.com/webdavis/dotfiles/pull/841), merged `b56b75254`.

- [x] 133. Nightshift, WITHDRAWN 2026-09-22. Operator ruling: pns is a notification tool, not a task
  runner, and its recap reports what happened and nothing more, so a verb that launches overnight work
  does not belong in it. No code was written, and the design and plan documents were removed. The entry
  as approved read: the one-word overnight handoff, paired with gnhf. At bedtime one command composes the
  overnight goal from the ledger (every open task that is unblocked, not operator-owned and not in an
  excluded section, in ledger order, with the standing rules attached), launches it through gnhf's loop
  (see `docs/runbooks/local-agents.md`) or the harness's own goal, silences the personal channels for the
  night, and hands the morning to `pns recap`'s overnight window (slice 54 retires morning). Exclusions
  and rulings are config, never retyped. The 2026-09-17 overnight prompt, written by hand, is the first
  fixture. Approved 2026-09-17; if SP8 turns out to be this, fold it there. Ruled 2026-09-20: the morning
  is `pns recap`, not morning.

- [ ] 139. pns profiles. Operator ruling 2026-09-17, answering "would profiles help": yes, and the gaps
  below were filled by the agent's best judgement on the same day and are part of the ruling until the
  operator changes one.

  WHAT A PROFILE IS. A named bundle of the settings that decide what reaches the operator: quiet on or
  off, which channels are on (banner, Discord, phone, lights), and which pages still get through. It
  replaces nothing in the event pipeline: hooks still record every event and the ledger still fills; a
  profile changes DELIVERY only. Three profiles ship, each fully visible in the config at its default
  (operator ruling 2026-08-31): `default` (today's behaviour), `work` (quiet, no lights, no Discord,
  phone for priority only) and `night` (quiet, lights off, phone for priority only, everything else held
  for the morning). A `meeting` profile is the operator's to add; task 126's calendar input is what would
  select it.

  THE PRIORITY FLOOR. A profile can choose HOW a priority page arrives (phone or banner) and never
  WHETHER. The `priority` route is health and security only (operator ruling 2026-09-14) and no profile,
  rule or manual override can silence it. posture posts its own pages through its own client and is not
  touched by pns profiles at all, by design.

  HOW ONE IS CHOSEN. `[[profiles.rules]]` is an ordered list; each rule names a profile and any of these
  inputs, and the first rule whose inputs all match wins. Inputs: `days` (weekday names), `hours` (a
  local-time window, midnight-crossing allowed, `22:00-06:00`), `location` (a named network the machine
  is on, see below), `focus` (a macOS Focus mode by name, through the probe pns already has in
  `pns-adapters/src/macos/focus.rs`, which is also how B18 finds Focus), and `calendar_busy` (true during
  a busy event, the input task 126 supplies). No rule matching means `default`. A rule that names a
  profile the config does not define is a load error that names the rule, never a silent fall-through.
  Overlapping rules are fine; order decides.

  MANUAL OVERRIDE. `pns profile <name>` wins over every rule until cleared with `pns profile clear`, and
  `pns profile <name> --for 2h` or `--until 17:30` clears itself. Bare `pns profile` prints the active
  profile, what chose it (the rule's index and the inputs that matched, or "manual, until 17:30") and
  what it is suppressing, so the operator can always answer "why is it quiet". The existing quiet expiry
  in `pns-adapters/src/persistence/sqlite/settings.rs` is the model for the override's storage. A
  Shortcut over SSH can run the same command from the phone.

  LOCATION. A location is a named network fingerprint. On this machine the Wi-Fi name is NOT readable:
  `ipconfig getsummary en0` prints `SSID : <redacted>` (measured 2026-09-17), because macOS gates the
  name behind Location Services. So the fingerprint is the default gateway's MAC address
  (`route -n get default` for the gateway, `arp -n <gateway>` for the MAC), optionally plus the subnet,
  and `pns profile learn <name>` records the current network under a name so nobody types a MAC. An
  unknown network matches no location rule. No GPS, no phone tracking, nothing leaves the machine.

  WHEN IT IS EVALUATED. The pns daemon resolves the active profile on its own clock (every tick),
  immediately on a network change, on a Focus change and on every override command. Each transition is
  one line in the ledger with the old profile, the new one and the reason. The resolver is a pure
  function (inputs in, profile out) so every rule combination has a deterministic unit test with no
  daemon.

  WHAT HAPPENS TO WHAT WAS SUPPRESSED. Nothing is dropped. An event a profile held back is recorded as
  held, and on the transition to a profile that allows it the engine delivers ONE roll-up ("held while
  `work`: 3 done, 1 blocked, 1 failed", with the blocked one first) through the existing replay path
  (`pns/crates/pns/src/return_replay.rs`), never the backlog one by one. Under `night` that roll-up is
  what `morning` (task 125) shows.

  RELATION TO OTHER TASKS. 126 becomes a rule input, not a switch. B18 (pause status lighting during
  quiet and Focus) becomes "the active profile decides whether status lighting runs", and the `default`
  profile keeps B18's decided behaviour. `pns quiet` stays as the short manual hush it is today and is
  documented as a temporary override of the active profile's quiet setting.

  DONE MEANS: the three shipped profiles and the rules table are in `dot_config/pns/config-values.toml`
  and regenerated into the template; the resolver has a test per input and per precedence case;
  `pns profile` explains its choice; the priority floor has a test that no profile can silence a priority
  page; a network-change transition is observed live on dresden with the transition line in the ledger.

### Fitness tracking Obsidian views (Scalebar)

Three changes to the Obsidian workout views of the fitness tracking system, requested by the operator
2026-09-17. Scalebar is its own repository at `~/workspaces/Ivy/webdavis/scalebar`; the widget code is
`obsidian/dataview-scripts/workflow-ui.js` with `timer-widget.js` beside it, and its tests are
`Tests/workflow-ui.test.js` and the other `Tests/workflow-*.cjs` files. Work lands as pull requests on
that repository, one per task, and this ledger only records them. The vault copy under `~/workspaces/Ivy`
is what the operator sees; deploy it the way Scalebar's own docs say, never by hand-editing the vault.

- [x] 129. The rest timer turns red while it counts down and returns to its normal color the moment it
  reaches zero. Nothing else about the timer changes.

  DONE 2026-09-17, merged as scalebar pull request #5. The countdown reads red while it counts and
  returns to its normal color at zero, and nothing else about the timer changed. The countdown is a bare
  span with no color of its own, and every color in the widget is a class in the injected style block
  rather than an inline style, so the change is one rule beside the existing error rule plus one class
  toggle appended to the existing tick, driven by the same remaining value the label reads. Returning to
  normal is removing the class, so the span goes back to the inherited value rather than to a literal,
  and red is the theme's own error variable the widget already used.

  Three facts established from the code rather than assumed: there is no paused rest timer, because the
  rest block offers only an extend control and a finish control; the timer cannot go negative, because it
  is clamped at zero, so zero and past-zero are one state that already renders its completion text; and
  extending a rest that had reached zero turns it red again.

  PROVEN RED FIRST: the test failed against the committed widget, passed after, and failed against a copy
  carrying the stylesheet rule but not the class toggle, so neither assertion can pass on half the
  change.

- [x] 130. Carry the previous set's weight forward. When a set is logged, the next set of the same
  exercise starts with that weight already filled in (bench at 135 lbs, log, the next set reads 135). The
  "Use previous" button stays and, when pressed, overrides the carried-over weight with the previous
  session's weight. The button is restyled as two lines inside one button: the first line reads
  `Use previous weight from <MM-DD-YYYY, Day>` in text smaller than the widget's normal button text, and
  the second line, smaller again, shows the weight that session used.

  DONE 2026-09-17, merged as scalebar pull request #6. Each new set's weight input is seeded from this
  session's last logged set of the same exercise, falling back to the template target when nothing has
  been logged for that exercise yet. The carry is derived from the logged rows on every render rather
  than stored, so leaving an exercise and returning shows that exercise's own last weight and never
  another's. The "Use previous" button stayed and became two lines in one button, the session date over
  the weight that session used, the first line smaller than normal button text and the second smaller
  again. The two weights stay distinct, the carry being this session's last set and the button being the
  previous session's, and pressing the button overrides the carried value.

  THE REVIEW CAUGHT A REAL DEFECT: the carry ignored set type and template weight, so a WARMUP set's
  weight seeded the next WORKING set and a drop set inherited the full working weight, which is a wrong
  number in front of someone mid-lift. The filter now requires both to match, with its own red-first test
  showing 95 seeded into a working set whose template said 185. A smaller find: the history disclosure
  showed the raw date while the button beneath it showed the formatted one, so one card displayed the
  same session two ways; both now use the formatter that already existed.

  OPERATOR OWES: deploy the widget the way Scalebar's documentation says, so the vault copy picks up
  tasks 129 and 130 together.

- [x] 131. Show the workout duration on the time button. After Start Workout and End Workout, the button
  on the right reads `<start_time> - <end_time>`; print the duration in smaller text between the two
  times, on that button, so the length of the workout is readable without opening anything. The inner
  view that opens from that button (the one that clears each time and lists the duration at the bottom)
  does NOT change in any way. Operator ruling 2026-09-17: that view stays exactly as it is.

  DONE 2026-09-18, merged as scalebar pull request #7. The button label now reads the start time, the
  duration in smaller muted text, then the end time. A workout with a start and no end reads as it did,
  with no duration span, so nothing appears mid-workout. The duration string the timing view already
  computed inline became a formatter both callers share, and its output is byte-identical to the old
  expression: same computation, same modulo-midnight wrap, same spelling.

  THE OPERATOR'S RULING THAT THE INNER VIEW DOES NOT CHANGE WAS PROVEN, NOT ASSERTED. The timing view has
  its own guard, and that guard passes on untouched `main` before any edit, so it describes existing
  behaviour rather than new behaviour; mutating the shared formatter to minutes only makes it fail, which
  is what pins the inner view's string rather than merely asserting it.

  Evidence beyond the usual: three mutation halves each fail on the half they should, all eight
  pre-existing regression tests pass individually against the changed widget, all seven lifecycle cases
  pass individually for 52 checks with zero failures, and a layout probe over the session card's own loop
  with real CSS reports the card height stable across all four time states, the action bounds unchanged
  to within one pixel, and no overflow with the longest cross-noon times. That probe fails exactly the
  two duration checks against untouched `main`, so it is not vacuous, and it is the only available
  evidence that the widened button does not break the layout suite, which cannot be run whole in this
  environment.

  OPERATOR OWES: deploy the widget the way Scalebar's documentation says. Tasks 129, 130 and 131 are all
  merged and all three reach the vault in that one deploy.

### Recover the remaining design from PR #24

- [x] Review #24 before deciding its disposition. Recovered on 2026-09-13 from head `2202dcbf`; four
  historical documents and thirteen upstream snapshots passed all 17 recorded hash checks. Retain its
  independent alert, restricted evidence and advisory-only requirements in the section below. Supersede
  the digest trigger and obsolete recipes in a smaller reviewed plan after the security decisions are
  settled. #24 remains open; do not merge its old instructions unchanged.

- [x] Reconcile the approval interface separately. DONE 2026-09-17. Butters tap-to-approve scoped to
  pending findings and the `/osquery allow|deny|list` Hermes skill. Verify the current posture command
  and trust contracts; investigation must not grant the analyst approval authority. On 2026-09-14 the
  reconciled scope was written to
  `docs/superpowers/specs/2026-09-14-osquery-approval-authority-design.md`. Verified on the host:
  `posture allowlist add|deny|list` is the writer, it refuses a label with no installed launchd agent,
  pins the plist hash with SHA-256, writes the chezmoi source and then refreshes the root-owned manifest;
  the allowlist is manifested (`0600 501`) and `allowlist_verdict` spends a vouch before it ever
  suppresses, so the 2026-07-26 option B plus D-prime both shipped, while option E did not. Nothing in
  posture, pns or the state tree has a pending-findings concept. Butters is now a Hermes profile (a
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

  RECONCILED AND BOTH PIECES RETIRED OR SHRUNK, 2026-09-17, as
  [PR #760](https://github.com/webdavis/dotfiles/pull/760), merged `a38fece5`, one document at
  `docs/superpowers/specs/2026-09-17-posture-approval-interface-reconciliation.md`. No Rust file, no
  route, no allowlist entry and no line of this ledger was touched by that pull request.

  PIECE 1, tap to approve scoped to pending findings: NOT BUILDABLE WITHOUT BREAKING THE TRUST BOUNDARY,
  and on two independent boundaries rather than one. The DELIVERY boundary, because a deliver-only
  route's delivery path hands a plain string to a dispatcher that takes no components and no view
  parameter, and hermes's one button surface is session-keyed and approves a COMMAND an agent already
  proposed rather than a FINDING, so a button on a page would need third-party code changed, which this
  repository never does. The INVESTIGATION boundary, because every route that could carry a tap is served
  by an agent, so the tapper is the investigator, which is the one thing this bullet forbids outright.

  PIECE 2, the `/osquery allow|deny|list` skill: BUILDABLE SMALLER, and only the `list` third of it. Both
  write verbs shrink away, and NOT because a skill would be unsafe. The reason is the live security fact
  this bullet already suspected on 2026-09-14 and which was MEASURED AGAINST THE INSTALLED HERMES on
  2026-09-17: the live Discord platform toolset carries `terminal` with a local backend, `sudo -n true`
  exits 0, `dot_bashrc.tmpl` puts `~/.cargo/bin` on the PATH a hermes shell sources, and BOTH approval
  gates allow the allowlist writer's add verb, so with no warnings collected the approval path returns
  approved WITH NO PROMPT. Every Discord hermes agent therefore already holds exactly the authority the
  proposed skill would have named, so the skill would be a label on an open door. THE BOUNDARY THAT NEEDS
  AN OPERATOR DECISION IS THE DISCORD PLATFORM TOOLSET ITSELF, not either piece of this design, and that
  supersedes open question 3 above: gating the recovered investigator on a pending scope does not close a
  door that is already open to every agent on that platform.

  Three further findings RETIRE design rather than shape it. A PENDING FINDING DOES NOT EXIST IN CODE:
  the finding type is a borrowed transient over one results-log row, the only persisted progress is an
  inode plus an offset, and the digest spool holds only digest outcomes, which is the COMPLEMENT of the
  set an approval would act on, so the derived pending set the 2026-09-14 document recommends cannot be
  built from what is stored. That answers assumption one of operator step 7 in the negative. A GRANT
  THROUGH THE WRITER IS SILENT: the allowlist file is watched, the integrity verdict is log-only when the
  manifest vouches for the new content, and the publisher refreshes that manifest as its last step, so a
  hand edit pages and the SUPPORTED path does not, which is the opposite of the announcement this design
  assumed it would get for free. And the allowlist write lock is a blocking exclusive lock recording NO
  identity, timestamp or verb, so the thread this repository has around it is a concurrency contract
  rather than an audit trail; that gap is filed as task 146.

  Two of the three properties "scoped to pending findings" needs ALREADY HOLD: the allowlist is consulted
  in exactly one gate branch, and an entry pins label plus path plus program plus plist hash together so
  a changed path or program is reported as a reused label rather than suppressed, which is precisely what
  stops one approved finding widening into an approved class. The missing third, a record that a finding
  is actually outstanding, needs new persisted state and is the only genuinely new thing either piece
  would require.

  Tonight's own merges changed two answers relative to the 2026-09-14 reading, and both simplify operator
  step 5. The webhook platform toolset is declared as the no-MCP sentinel, which resolves to an EMPTY
  toolset, so the `explain` agent route is fenced from the allow path by an empty toolset rather than by
  prose. And the bare `posture` route's 404 recorded above is GONE, because task 99 renamed it to
  `posture-pages`, which the gateway already serves.

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

- [x] 81. DONE 2026-09-15 in [PR #696](https://github.com/webdavis/dotfiles/pull/696), merged `b5761206`.
  The model was already largely expressed in `dot_config/pns/config-values.toml` and read by
  `pns_domain::channel_map::channel_for`, so that pull request closed the three remaining gaps rather
  than rebuilding it. `[plugins.discord.channels]` names one entry per project as `#<project>-dev` across
  all eleven projects the bullet lists, with `#github-notifications` as the `default` catch-all,
  `#priority` as the severity channel and `#pns-events` for an event carrying no project at all; every
  entry names its KeePassXC record rather than holding an id. The two-axis routing consults the route
  severity already chose, then the project as `owner/name`, then its bare name, then the default route,
  then the catch-all, and four behaviors are pinned: a subject reaching its own project channel, an
  unmapped repository reaching the catch-all, a critical severity overriding a mapped project's channel,
  and a critical severity for an unmapped repository or for no repository at all still reaching the
  severity channel rather than falling through. Two integrity gaps closed with it: an armed
  `[plugins.discord]` whose map names no channel under `[routes] urgent` is now refused at load, because
  the lookup would otherwise send a critical page to the project's own routine channel without failing;
  and every key of the open channels table is now secret-bearing in the values-file check rather than
  only the fixed `default` one, so a pasted channel id under any project's key is refused before it can
  render into the committed template. `#general` and `#uu-runs` deliberately get no pns entry: pns
  selects neither route, uu posts to its own with its own key, and nothing in this repository produces to
  general. The Discord `Projects` category is server-side and has no configuration surface. THE CONSUMERS
  of the one shared map arrive with the Discord bot destination in task 82 and the GitHub source in task
  85, which is those tasks' scope rather than a remainder of this one. Original entry: settle the Discord
  channel model in configuration (operator rulings 2026-09-14 and 2026-09-15). Every channel is
  `#<project>-<stream>` and never a bare project name, and each project gets `#<project>-dev` for
  continuous integration, pull requests, GitHub notifications and agent session threads, covering
  dotfiles, pns, uu, posture, homelab, justdavis-ansible, essential-feed-case-study, scalebar, netpulse,
  plantpulse and casually-concerned. `#github-notifications` is the catch-all, and `#priority` is the
  severity channel for anything critical from any source, which means rare, actionable and worse if
  ignored, posted once with the subject in its header. The notification channels are `#pns-events`,
  `#uu-runs`, `#posture-pages` and `#general`, while `#pns-recap` and `#uu-failures` were deleted.
  Routing is two-axis: the subject picks the channel and severity overrides it to `#priority`. The
  Discord category is `Projects`, and one `repo -> channel entry` map in the pns config is shared by the
  GitHub source and by session events.

- [ ] 82. Give pns its own Discord destination. THE DESIGN'S FOUR PULL REQUESTS ALL MERGED (#639 the
  plugin and table, #648 the channel map, #650 session threads, #654 the recap plus the GitHub source
  plus the runbook, #696 the channel model fixes), and the one behaviour the ladder declared but left
  untested was pinned 2026-09-17 by [PR #712](https://github.com/webdavis/dotfiles/pull/712): the thread
  name is the header's first line, capped at 100 characters. Dropping that cap was a silent mutant, since
  the thread creation would earn a 400 and the session would get no thread while the message still landed
  and the verdict still read Delivered; the test writes the ceiling out as the literal 100 rather than
  reading the constant, so raising the constant past what Discord accepts also fails, and it was
  mutation-checked both ways.

  REMAINING: the slash commands `/pns pending`, `/pns approve` and `/pns reject`, which the design places
  deliberately out of scope and which are BLOCKED ON OPERATOR INFRASTRUCTURE: a public interactions
  endpoint with its URL registered in the Discord application, plus a pns write path that does not exist
  yet. A command registered against a dead endpoint is worse than no command, so nothing ships until the
  endpoint does. Two smaller pieces were considered and deliberately not built: a doctor preflight of the
  disabled `[plugins.discord]` table, which would contradict the 2026-08-31 ruling that a disabled table
  is inert, and logging the 429 `Retry-After` header, which the design says is logged rather than obeyed
  and which today is never read at all, since `DiscordReply` carries no headers.

  Repository channels are delivered by this bot and never by a hermes route. Its secrets are the
  KeePassXC entries `Discord (Uriel) :: Bot Token (pns)`, `Discord (Uriel) :: Public Key (pns)`,
  `Discord (Uriel) :: Application/User ID (pns)`, and one
  `Discord (Uriel) :: Channel ID (#<project>-dev)` per project plus `(#github-notifications)`. The bot
  stays offline and `[plugins.discord]` ships disabled until the destination ships.

- [x] 83. CLOSED 2026-09-16, merged as [PR #707](https://github.com/webdavis/dotfiles/pull/707). Version
  1 of `pns.request` now carries `kind` (`agent` or `health`), a health event reaches `[routes] urgent`
  only when its state is one that waits on the operator, and uu gained a test over its own argv proving
  it names no route, channel or gateway. The `#uu-failures` channel was dropped on 2026-09-15. Operator
  step, still owed: delete the two vault entries `Hermes :: Webhook Secret (#uu-failures)` and
  `Discord (Uriel) :: Channel ID (#uu-failures)`.

- [x] 84. ALL THREE PULL REQUESTS MERGED 2026-09-17. Build the posture critical-page explainer on the
  design merged in [PR #618](https://github.com/webdavis/dotfiles/pull/618), amended by
  `docs/superpowers/specs/2026-09-15-posture-explainer-amendment.md`.
  [PR #711](https://github.com/webdavis/dotfiles/pull/711) declared the hermes `explain` agent route and
  the webhook sandbox (`platform_toolsets.webhook: ["no_mcp"]`) and carved it out of the route-status
  check, which now checks six routes with per-route expectations and an inverted `deliver_only` assertion
  for `explain`; every load-bearing claim was verified against installed hermes 0.17.0, including that
  the signature is validated before an agent task is created, so the checker's unsigned probe starts no
  model run. [PR #714](https://github.com/webdavis/dotfiles/pull/714) built posture's copy leg: a
  critical page is posted a second time, verbatim, to the route `notify.hermes.critical_copy_route`
  names, after its own post came back delivered, signed with that route's own key and carrying a request
  id derived from the page's but never equal to it; the hour is counted by distinct finding, capped at
  twenty, in one timestamp file under `~/.local/state`, failing open. Fourteen hand mutants were run
  against the new logic and the one survivor was fixed. The explanation posts in the same channel as the
  page it explains and directly under it, moving into the page's own thread once the pns Discord bot of
  task 82 exists. The `#explain` Discord channel and its `Discord (Uriel) :: Channel ID (#explain)` vault
  entry exist but stay UNUSED by decision 7: delivery reuses the `priority` channel entry, and the one
  new vault entry the route consumes is `Hermes :: Webhook Secret (#explain)`. PR 3 closed it on
  2026-09-17 as [PR #728](https://github.com/webdavis/dotfiles/pull/728), merged `db769f18`: the hermes
  section of `docs/runbooks/local-daemons.md` gained the `explain` route in the routes table, its
  KeePassXC entry titles, posture as the route's sender, and a subsection on the copy leg with the
  rolling-hour cap and the restart step. Three gotchas were documented: an empty webhook toolset list
  leaves every Model Context Protocol server live where the `no_mcp` sentinel is required, the gateway's
  duplicate-delivery cache is keyed on the request id alone across every route, and an absent placeholder
  renders as literal braces and is sent as written. Stale route counts in the same file were corrected in
  the same change. Operator step: after the next apply, `hermes gateway restart`, so the gateway loads
  the new route.

- [x] 85. ALL FOUR PARTS ON MAIN 2026-09-17. PARTS 1 AND 2 OF 4 MERGED 2026-09-17,
  [PR #713](https://github.com/webdavis/dotfiles/pull/713) and
  [PR #740](https://github.com/webdavis/dotfiles/pull/740); parts 3 and 4 remain. The polling baseline
  polls `GET /notifications` once per `X-Poll-Interval` with a `Last-Modified` conditional request,
  dedupes by a durable seen-set with a 24-hour expiry, submits through the ordinary producer API so task
  81's channel map decides the channel, and treats a 401 or 403 as a configuration refusal rather than an
  empty listing. Four defects were fixed while finishing it: the server's interval is now clamped to the
  key's bounds in a pure `job_interval` beside those bounds, so nothing hands the scheduler an unchecked
  header; the settings reader re-exports the registry's `GITHUB` name instead of declaring a second
  literal; the first-poll backlog guard reads BOTH halves of the stored state, because a 200 carrying no
  `Last-Modified` would otherwise make every later tick read as another first poll and silence the source
  permanently; and `pns github poll` joined the usage text. Built on the design merged in
  [PR #620](https://github.com/webdavis/dotfiles/pull/620), whose channel names `#github-<repo>` and
  `#github` are superseded by `#<project>-dev` and `#github-notifications` under task 81. The token is
  the classic `GitHub (Webdavis) :: Personal Access Token (pns notifications)`, `notifications` scope
  only, no expiry. REMAINING, one pull request each: the push receiver (through the existing Cloudflare
  tunnel with a separate receiver process), the lamp colours (configurable in the pns config, defaulting
  to purple for a pass and orange for a failure), and the lamp wiring (three dedicated lamps,
  `3F - Studio - HCL2`, `3F - MBedroom - HCL2` and `2F - Kitchen - HCD5`, each with `shows = ["github"]`
  and nothing else). On GitHub itself, Actions notifications are set to On GitHub with failed-only off,
  and Dependabot alerts to On GitHub plus CLI. STATUS 2026-09-17: part 1 is SHIPPED, not merely built. It
  merged as [PR #713](https://github.com/webdavis/dotfiles/pull/713) at `33e38442` and the code is on
  main (`pns/crates/pns-adapters/src/github/notifications.rs`, `github/client.rs`, `config/github.rs`),
  so the earlier note about an unshipped worktree with half-applied uncommitted fixes is stale and
  nothing is at risk. The operator's GitHub-side settings are DONE and confirmed by screenshot on
  2026-09-17: Actions notifications are `on GitHub` with `Only notify for failed workflows` unchecked,
  which is what makes a passing run observable at all and therefore what makes the purple-for-pass lamp
  possible, and Dependabot alerts are `on GitHub, CLI`. Both matter because pns reads the notifications
  inbox through `GET /notifications`, and an item delivered only by email never enters that inbox for pns
  to read. REMAINING: three pull requests, none of them blocked on the operator, in this order: the push
  receiver through the existing Cloudflare tunnel as a separate process, then the GitHub lamp colours
  (configurable, defaulting to purple for a pass and orange for a failure), then the lamp wiring for
  `3F - Studio - HCL2`, `3F - MBedroom - HCL2` and `2F - Kitchen - HCD5`, each with `shows = ["github"]`
  and nothing else. PART 2 SHIPPED 2026-09-17 as
  [PR #740](https://github.com/webdavis/dotfiles/pull/740), merged `7cadb16b`, and it DEPARTS FROM THE
  MERGED DESIGN DELIBERATELY. The receiver is a DOORBELL, not a second source: `pns github receive` binds
  `127.0.0.1:<webhook_port>`, verifies the delivery, answers it, and runs ONE immediate poll through the
  same `poll_once` the scheduled job runs. It never parses the webhook body. The design's own shape,
  where the receiver maps the payload and submits, was rejected on evidence: the poll's identity for a
  `ci_activity` thread is built from the thread id and the thread's `updated_at`, and a webhook payload
  carries neither, so the two transports would have minted different identities and double-posted in
  production while passing any same-identity test. Authentication is HMAC (hash-based message
  authentication code) SHA-256 over the exact body bytes against `X-Hub-Signature-256`, compared with
  `verify_slice` rather than `==`, which GitHub's documentation asks for by name; POST to
  `/webhooks/github` plus both `X-GitHub-Event` and `X-GitHub-Delivery` are required first, a stated
  `Content-Length` over one mebibyte is refused without allocating, and an empty configured secret
  verifies nothing rather than trivially passing. Every refusal answers 403 with the reason only in the
  receiver's own log, so nothing says which check failed. A receiver that is off, unreachable or crashed
  changes nothing, because the scheduled job is the only delivery path and reads none of the receiver's
  config. It lives in the pns workspace as a subcommand of the one binary but runs as its own process
  under `com.webdavis.pns-github-receiver`, so the always-on daemon still never listens on a socket.
  Proven against fixture requests with a fake secret and an obviously fake token: signed 204, unsigned
  403, wrong secret 403, wrong path 403, GET 403, and the decisive pair, an unsigned request created no
  state directory while a signed one did, which is proof the doorbell rang. A local port scanner probed
  the port twice unprompted during the work and was refused as not a POST. A CORRECTION THIS TURNED UP:
  part 1's docblock on `submitted` claims the delivery ledger is a second guard behind the seen-set. It
  is not. `DecisionOutcomes::begin` only refuses to insert a second decision row; `attempt_live` still
  attempts every leg, so the seen-set is the ONLY dedupe, which is why the doorbell and the tick
  serialise on a lock in the state directory and a poll that finds it held stands down. That comment
  needs correcting in a follow-up. Deliberately not built: posture's declared-hostname control that the
  design folded into this part (posture's own workspace and tests), and any doorbell throttle, marked in
  code with a note naming the five-thousand-an-hour ceiling. PART 2 IS ARMED ONLY BY THE OPERATOR, and
  nothing is listening until they do it: create a read-only GitHub App subscribed to `workflow_run`,
  `check_suite`, `pull_request`, `release` and `dependabot_alert` with a webhook secret, store that
  secret as `GitHub (Webdavis) :: Webhook Secret (pns receiver)`, add
  `webhook_secret = { keepassxc = ... }` to `[plugins.github]` in `dot_config/pns/config-values.toml` and
  run `just pns-config-render`, add a second ingress hostname to `~/.cloudflared/config.yml` above the
  catch-all pointing at `http://127.0.0.1:8648` and restart cloudflared (that file is not
  chezmoi-tracked), give that hostname one Cloudflare Access Bypass policy scoped to GitHub's webhook
  ranges read from `GET https://api.github.com/meta` and no Allow rule, point the App's webhook URL at
  `https://<hostname>/webhooks/github`, apply, then press Redeliver and expect 204 plus a notification
  within seconds. TWO QUESTIONS LEFT OPEN: whether a hand-pressed redelivery should force a submission
  the seen-set would otherwise refuse (under the doorbell it looks like nothing happens, which is
  probably correct), and whether posture's declared-hostname control gets its own pull request now that a
  second hostname reaches the internet or waits behind parts 3 and 4.

  PART 3 SHIPPED as [PR #747](https://github.com/webdavis/dotfiles/pull/747), merged `eee9cb0c`, and the
  investigation found MOST OF IT ALREADY BUILT. The colour machinery had shipped with the design: both
  keys parse as coordinate-validated pairs, default to the locked purple and orange, and already render
  UNCOMMENTED at their defaults in the generated template, which satisfies the 2026-08-31
  visible-defaults ruling. So nothing in the config changed and nothing needed to. NOTHING EVER PRODUCED
  A FLASH, because the poll compiled a neutral outcome in, and the lamp therefore stayed dark for every
  run the inbox reported. The one real gap was the pass-or-fail judgement, and that is the whole diff:
  two files, 99 insertions.

  HOW THE JUDGEMENT IS MADE, and the limitation is the operator's to rule on. The notifications endpoint
  carries NO documented field holding a run's conclusion: a thread gives a reason, a subject type, a
  subject title, a subject URL, a repository and a timestamp, and the CI reason plus the check kind say a
  run FINISHED but never how it ENDED. The only statement about the ending is the subject TITLE string,
  which the vendor does not document and has reworded before (checked against the notifications reference
  and the Actions notifications page; two searches found no published format). So the judgement reads
  that title FAIL CLOSED: only a workflow-run or check kind is judged at all, the title is split on
  whitespace with each word trimmed of punctuation, and exactly one of two conclusion words present as a
  WHOLE WORD yields a colour. Everything else stays neutral and reaches no lamp: a cancelled or skipped
  run, both words at once, no word at all, a workflow NAMED something like `failed-login-tests`, and any
  thread a human raised, so a pull request titled "fix the failed retry path" never flashes orange. A
  rewording upstream therefore makes the lamp GO QUIET rather than show a colour nobody's inbox stated.
  Five tests pin it, no spawn and no clock.

  PART 4 NEEDED NO PULL REQUEST. The three lamp declarations it was going to add are already in
  `dot_config/pns/config-values.toml` and in the generated template, verified by grep at three hits each,
  so the wiring this task assumed was outstanding had already landed.

  OPERATOR STEPS: one full apply, which is what rebuilds pns so the judgement reaches the deployed
  binary; until then every polled GitHub event still reports neutral and the lamps stay dark. Then the
  first CI notification should light a lamp. TWO OPEN QUESTIONS for the operator: whether to pay one
  extra request per CI thread to read the subject's real conclusion field instead of the title heuristic,
  which is a rate-limit budget nobody has set; and whether a cancelled or timed-out run deserves a third
  colour or should keep reaching no lamp as it does now.

- [x] 86. Finish the live coverage of the five hermes routes. CLOSED 2026-09-17 on the two answers it was
  waiting for. The 2026-09-15 check covered `pns-events`, `priority` and `posture-pages` with real posts.
  `uu-runs` gets its first live post at the next weekly `uu` run, and `general` has no producer in this
  repository, so it stays unproven until something posts to it. MEASURED 2026-09-17, four of five now
  proven. `pns doctor` confirmed `general`, `posture-pages` and `priority` as served by the gateway, and
  its Channels section reported hermes `sent, posted HTTP 200`. `uu-runs` was proven separately by
  `uu run rotate-logs`, the cheapest lane, which rotated one log and emitted its run record
  (`unattended / uu / completed / dresden`, `rotate-logs: 0 failure(s)`, one log rotated at 33931949
  bytes and eleven under threshold); that record also reported
  `last successful run: NEVER RECORDED on this machine`, so it is the first uu run this machine has
  recorded. `pns-events` is the only one of the five not directly exercised by either run. Doctor also
  named two routes the gateway does not serve, `pns-recap` and `posture`, both of which the operator
  ruled retired rather than missing; that is task 99 and not a gap in this one. RULING 2026-09-17:
  `general` is recorded as INTENTIONALLY UNUSED, the same disposition as the `#explain` channel. It is
  proven served by the gateway and no producer in this repository posts to it, so it needs no producer
  and its silence is not a gap. That leaves `pns-events` as the only route of the five neither the doctor
  run nor the uu run exercised. CLOSED 2026-09-17. `general` is recorded as INTENTIONALLY UNUSED by
  operator ruling, the same standing as the `explain` channel entry, so it needs no live post and its
  absence from the coverage table is the answer rather than a gap. `uu-runs` gets its first live post at
  the next weekly `uu` run and needs no agent action; when that run happens its record is the coverage.
  The other three routes were already covered with real posts on 2026-09-15, and tonight's evidence pass
  for task 50a added three more heartbeat and two more digest deliveries on `posture-pages` (2026-09-15
  through 09-17), so that route's coverage is now repeated rather than single.

- [x] 88. Give a storm one combined explanation instead of one per finding. DONE 2026-09-17. Approved by
  the operator 2026-09-15, alongside the answers recorded in
  `docs/superpowers/specs/2026-09-15-posture-explainer-amendment.md`. Task 84's per-finding cap cannot
  solve spam by itself: any number low enough to avoid spam is low enough to hide findings, and twenty
  distinct failures already means the machine is in trouble. The useful message at that point is one that
  says so and lists them, not twenty separate explanations. posture's own pages already arrive uncapped
  today, so a storm already reaches the operator on that leg, and a combined message would improve it
  too. SHIPPED 2026-09-17 as [PR #730](https://github.com/webdavis/dotfiles/pull/730), merged `02f59103`.
  The existing critical-copy leg in `posture-adapters/src/hermes.rs` was extended rather than given a
  second counter: the rolling-hour window file now stores what each distinct finding says beside its key,
  and `window::claim` answers four states, Granted (copy the page verbatim as before), AlreadyCopied (a
  repeat, silent), Storm (this finding crossed the threshold, so post ONE combined message listing every
  distinct finding of the hour, oldest first) and Storming (the hour's one message is sent, nothing
  further is explained, sticky for the full hour; the review caught that the first cut was not sticky and
  it was fixed before merge). The combined message goes to the same copy route, signed with that route's
  own key, with its own derived request id, in the same body shape as a page. ONE NUMBER:
  `STORM_THRESHOLD = 5` replaces the twenty-per-hour cap, because after the crossing no per-finding copy
  is posted, so five copies plus one combined message is the hour's ceiling. The withheld-copy banner is
  gone; the combined message is what says the machine is in trouble. DECISION RECORDED: the combined form
  applies ONLY to the explanation leg. posture's own pages stay uncapped and unchanged, because combining
  pages means holding a critical security page back until its neighbours arrive. 29 hermes tests pass in
  0.01 s with no spawn and no wall clock. Two questions for the operator: (1) should the pages leg also
  get an ADDITIVE storm summary page beside the individual pages (one more post in the record channel,
  withholding nothing)? (2) is five the right threshold once the detector set grows past the eight
  declared controls? OPERATOR STEP: a full `chezmoi apply` picks up the rebuilt binary and a comment
  change in `~/.config/posture/config.toml`; no new vault entry and no route change.

- [x] 94. Fix the 500 ms spawn deadline in
  `channel_dispatch::tests::environment::the_public_factory_preserves_blank_override_and_backend_refusal_before_dispatch`,
  filed 2026-09-17. It reddened MAIN on the #711 merge run with `forced: ` and an empty message, and main
  went green again on the next merge eleven minutes later, so it is a load-sensitive flake rather than a
  real defect. The mechanism is in the test itself: it spawns a real process, gives it
  `Duration::from_millis(500)`, KILLS it at the deadline, and then asserts `output.status.success()`. On
  a loaded runner the child is not finished in 500 ms, so the test kills it and then asserts the killed
  process succeeded; the empty message after the scenario name is the killed child's empty stderr. This
  is the same bug CLASS as task 92 but NOT the same bug: 92 was a real product race in the dispatch write
  path, this one is a fixture budget. See the standing note that tight fixture budgets flake continuous
  integration. Either raise the bound well past the noise floor or stop asserting success on a process
  the test itself killed; the second is the honest fix, since a killed process succeeding is not a
  behaviour anyone wants. DONE 2026-09-17 in [PR #718](https://github.com/webdavis/dotfiles/pull/718),
  merged `b64c2a08`. The flake was REPRODUCED first, one failure in forty runs of the single test at load
  average 29, roughly two to three percent, consistent with a 500 ms budget for a spawn a warm run
  finishes in about 10 ms. The fix removes the spawn rather than widening the budget: the process existed
  only to control `PNS_CHANNELS_DIR` for one `std::env::var` read, so that read moved one level up and a
  new private `destinations_for_override` takes the value as a parameter, which the test now calls in
  process. Net 19 lines removed, the public signatures unchanged, and the launchctl-style re-exec of the
  test binary gone. Measured after: 60 of 60 passes under the same 48-spinner load, about 43 ms per run.
  Mutation-checked, deleting the blank-override filter turns it red. Two review findings were fixed in
  the same pull request, both naming: the test no longer claims the public factory it stopped calling,
  and a comment no longer overclaims that the environment read is the only thing above the seam. See task
  101 for the two further flakes this work exposed.

- [x] 95. Make the herdr configuration survive an apply, filed 2026-09-17. `~/.config/herdr/config.toml`
  and `~/.config/herdr/plugins/config/**` are PLAIN chezmoi targets that parties other than chezmoi
  write: zoetrope's `setup-keys` writes a marked key block, the `herdr-agent-quota` `configure` action
  rewrites the sidebar row and its own setting files, and the operator hand-edits them. Every apply
  therefore reverts whatever arrived that way. Measured 2026-09-17: an apply destroyed the operator's
  hand-written clauth block (the `prefix+alt+a` binding plus the `[ui.sidebar.agents.rows_by_agent]`
  claude row using `$clauth` and `$clauth_delegate`) and they retyped it, and four `herdr-agent-quota`
  settings plus eight brand-coloured provider rows would have gone the same way had they not been
  captured into source minutes earlier. `~/.claude/settings.json` and `~/.codex/config.toml` face
  identical pressure and are SAFE, because both are `modify_` templates that declare the stable fields
  and read the app-written state back out of the live file. The fix is to give herdr's targets the same
  treatment, on the pattern of `private_dot_codex/modify_private_config.toml`. Until then every plugin
  toggle needs a manual capture into source before the next apply, which is exactly the manual step the
  design bar rejects. CLOSED 2026-09-17 by operator ruling, with the modify template REVERTED.
  [PR #720](https://github.com/webdavis/dotfiles/pull/720) converted `dot_config/herdr/config.toml` to a
  `modify_` template; the operator ruled that the file is rarely overwritten and is to stay a plain
  tracked target, so the template, its declared partial and the quarantine script that only existed to
  stop an unparseable live file aborting the apply were all removed again. What survives from that pull
  request is the one piece that stands on its own: the eight one-line `herdr-agent-quota` plugin config
  leaves are no longer tracked, because the quota plugin regenerates them on every apply and a tracked
  snapshot fought that. Standing consequence, accepted: an apply still overwrites
  `~/.config/herdr/config.toml` from source, so a live edit must be committed before the next apply to
  survive it.

- [x] 96. Point moshi at dresden's tailnet name, filed 2026-09-17. The operator cannot reach dresden from
  moshi since the SSH hardening, and the card reads
  `DNS resolution failed: failed to lookup address information: nodename nor servname provided`. Two
  causes stack. First, the phone `mister` reads offline in `tailscale status` (last seen a day before
  filing, key good until 2026-09-24), and a MagicDNS name only resolves for a device actually on the
  tailnet, which is the resolution failure verbatim. Second, and the part the hardening owns: the
  `Match LocalAddress` block from [PR #609](https://github.com/webdavis/dotfiles/pull/609) refuses every
  connection that did not arrive on loopback or in the tailnet address space, measured with
  `sshd -G -T -C` on 2026-09-17 as `refuseconnection yes` for a LAN source reaching `192.168.1.26` and
  `refuseconnection no` for a tailnet source reaching `100.77.192.92`. So a LAN path that used to work is
  refused by design and the tailnet path is the only one left. Port 22 answers on loopback, the LAN
  address and the tailnet address, so sshd itself is healthy. Operator steps: (1) reconnect Tailscale on
  the phone; (2) set moshi's host for dresden to `dresden.tail2f2430.ts.net` or `100.77.192.92`, never
  `192.168.1.26` and never a bare or `.local` name, which is the same trap already recorded for the
  Shortcut's Hostname variable under the SSH exposure entry. CLOSED 2026-09-17: the operator reconnected
  Tailscale on the phone, which restored MagicDNS resolution, and moshi was already pointed at
  `dresden.tail2f2430.ts.net` rather than a LAN or `.local` name, so no host change was needed.

- [x] 97. posture hardcodes the operator's launchd labels, filed 2026-09-17. `posture-domain` carries
  five job labels as literals (`watchdog/agents.rs:19-23`, for example
  `Self::ResultsAlerter => "com.webdavis.osquery-results-alerter"`) plus the prefix they are matched on
  (`page/header.rs:15`, `OUR_AGENT_PREFIX = "com.webdavis.osquery-"`). posture is a product installed
  with `cargo install`, so a stranger's watchdog searches for LaunchAgents named after this repository's
  operator, finds none, and either pages on every tick or reports all clear falsely. Three faults sit in
  that one string: the operator's handle in a shipped binary, against the user-agnostic naming rule; the
  dependency `osquery` naming the job, which is the axis the libexec directory rule already rejects; and
  a closed enum, so no installer can choose different names. Fix: the job labels move to
  `~/.config/posture/config.toml`, read by the watchdog and by task 98's writer from the one place, with
  reverse-DNS defaults (`dev.posture.watchdog` and siblings) that name posture rather than osquery.
  Renaming the six deployed jobs on dresden reaches six plists, six `run_onchange_after_60` loaders,
  `dot_config/osquery/private_page-launchd-allowlist.txt`, the `~/.local/log/osquery/` log paths, the
  CLAUDE.md LaunchAgent table, and five test files. Two consequences: a renamed label does not replace
  the old one and this repository builds no removal mechanisms, so the operator owes one
  `launchctl bootout` per retired job; and the plists sit in the known-good manifest, so the rename ships
  on a full apply rather than a by-name one. DONE 2026-09-17 in
  [PR #721](https://github.com/webdavis/dotfiles/pull/721), merged `3599a3bb`. The labels now come from a
  `[jobs]` table in `~/.config/posture/config.toml`, read by both the watchdog and the page header,
  defaulting to `dev.posture.<key>`. `Agent` was rebuilt around posture's own six subcommands with
  `Agent::MONITORED` naming the five the watchdog can judge, since it cannot judge itself, and the
  launchctl adapter now knows no names at all. The plist match is whole-name rather than prefix, so a
  label like `dev.posture.digest.extra` cannot be swept in. Every test goes red if a label returns to
  source. The rename of the six jobs deployed on this machine was deliberately NOT done: the shipped
  config states the existing `com.webdavis.osquery-*` labels verbatim, so no `launchctl bootout` is owed,
  and the rename stays follow-up work. Confirmed live after the 2026-09-17 apply:
  `~/.local/state/osquery-watchdog-state.json` is now keyed by job (`alert`, `poll`, `funnel`, `digest`,
  `heartbeat`) rather than by label, which only the new binary writes, with every streak at 0 and no page
  raised.

- [x] 98. DONE 2026-09-17. `posture jobs`: let posture install and verify its own scheduled jobs, filed
  2026-09-17. posture is a one-shot by design (every subcommand samples current state and exits; there is
  no daemon and, ruled 2026-09-17, there should not be, because the two daily jobs rely on launchd
  starting a missed calendar fire on wake, which `man 5 launchd.plist` documents and a sleep loop does
  not do, and because the watchdog must not share a process with the monitors it reports on). The timers
  therefore live outside the binary, and on dresden they live in THIS repository, so a stranger who
  installs posture gets the checks and no schedule at all. Close that with a subcommand group, the verb
  set already used by `posture ssh`: `posture jobs install` writes the units and loads them;
  `posture jobs verify` asserts each one exists, is loaded, and matches what posture would write;
  `posture jobs list` prints what posture expects beside the live state of each; `posture jobs print`
  dumps a unit to standard output without writing it. `verify` stops at installed-and-loaded and never
  grows a liveness check: whether a job actually ran and exited zero is `posture watchdog`'s job, and two
  answers to that question would eventually disagree. Six jobs are in scope, five plain timers (`poll`
  and `funnel` at 60s, `alert` at 300s, `watchdog` at 900s, and `digest` and `heartbeat` on a daily
  calendar), plus `alert`'s `WatchPaths` trigger on `~/.local/log/osquery/osqueryd.results.log`, which is
  a seventh unit file on Linux because systemd splits that into a `.path` unit. `converge` is not
  scheduled and stays out. The two daily jobs read their hour and minute from `.chezmoidata` today, so
  those values move into posture's own config with task 97's labels. Depends on task 97: the writer and
  the watchdog must read the same label list. Naming: the group is `jobs` rather than `timers` because
  one of the six is a file watch, rather than `agents` because that word now means something else and
  collides with `io.osquery.agent`, and rather than `service` because there are six jobs and not one
  service.

  SHIPPED as [PR #764](https://github.com/webdavis/dotfiles/pull/764), merged `148df91f`. The four verbs
  exist as asked. Job plans (label, subcommand, program, trigger, watch path, log, run-at-load) and the
  launchd rendering live in `posture-domain` as total functions over task 97's label list, so the writer
  and the watchdog read the one list. The two daily times moved into posture's own config as a new
  `[jobs.daily]` table read through the same schema as the labels, which is what a stranger installing
  with `cargo install` needs. Drift is reported as the unit lines each side holds alone, whitespace and
  order insensitive, so verify is actionable against units something else wrote.

  NOTHING IN THE SUITE WRITES OR LOADS A REAL UNIT: every path hangs off an injected home directory and
  every launchctl call goes through the existing command seam, so all ten command tests drive a temporary
  home and a recorded command list. Twelve domain cases plus ten command cases plus four new config
  cases.

  THE MOST VALUABLE OUTPUT IS THE LIVE DRIFT READING, taken read-only against dresden, where
  `posture jobs verify` EXITS 1. Every label, interval, calendar time and run-at-load value AGREED, which
  confirms posture's plan is faithful to the deployed schedule. Three things differ and each needs an
  operator ruling on which side is right: the deployed plists log to `~/.local/log/osquery/<varied>.log`
  where posture would use `~/.local/log/posture/<key>.log` (the lane's reasoning: the log holds posture's
  own output and osquery is a dependency, the same axis the libexec directory rule uses, and this is the
  largest share of the drift); four of the six deployed plists carry NO environment PATH block where
  posture writes one uniform block, because a launchd job inherits almost no PATH and several subcommands
  shell out; and the deployed funnel plist carries an osquery tailscale binary variable posture does not
  write, which the lane read as a per-machine override of a dependency path rather than part of the
  schedule.

  SCOPE DEPARTURE, recorded because the task asked otherwise: the entry asks for a seventh systemd path
  unit on Linux, and the lane shipped LAUNCHD ONLY. Its reasoning: posture has no Linux support anywhere
  today (the osquery pipeline, launchctl, codesign and the sshd paths are all macOS), so a systemd writer
  nothing runs would be untested surface, and the `jobs` group name already accommodates it when a Linux
  port arrives. Accept or reject that when the Linux port is real.

  One review finding was a genuine correctness bug: verify compared unit lines as an UNORDERED multiset,
  so a daily job with its hour and minute SWAPPED read as no drift. Fixed by joining each key with its
  value on one line before sorting, with a test named for the swap.

  OPERATOR STEPS: a full apply (the posture config template gained `[jobs.daily]`), then
  `posture jobs verify` on dresden and a ruling on the three drifts above.

- [x] 99. Stop naming two retired hermes routes. DONE 2026-09-17. Filed the same day on two operator
  rulings. `pns doctor` on 2026-09-17 reported
  `route pns-recap: THE GATEWAY HAS NO SUCH ROUTE; a page sent here is lost` and the same for
  `route posture`, against `general`, `posture-pages` and `priority` which it confirmed served. Neither
  route should be added. The operator ruled both retired: `pns-recap` and its Discord channel went with
  task 81 on 2026-09-15 and a recap now posts on the default route, which
  `pns/crates/pns-application/src/post_return_recap.rs` already implements; and there is no `#posture`
  channel any more, only `#posture-pages`, which the gateway already serves. So the fix is subtractive on
  both names rather than the additive one the earlier design assumed. Two changes: doctor stops checking
  `pns-recap`, because a checker that warns about a route nobody targets trains the operator to ignore
  it; and posture's six hard-coded `posture` call sites collapse to one overridable default naming
  `posture-pages`. This SUPERSEDES the additive half of the unnumbered item under
  `### Hermes security investigation, recovered from #24`, which proposed declaring a `posture` route in
  the age-encrypted hermes config with its own secret and channel, and it answers that design's first
  open question (which channel security pages land in) with `#posture-pages`. The measured damage that
  item records still stands and is the reason this is not cosmetic: the live gateway answers 404 for
  `posture`, `ledger_legs` holds eight dead-lettered legs on it with `http_status = 404` whose banner
  legs all delivered, so eight daily digests reached the operator locally and never reached Discord. It
  also stays a blocker on `feat/posture-alert-cutover` for the same reason that item gives: when that
  cutover merges and is applied, the CRITICAL security page moves onto the 404 route. Do NOT read
  `~/.hermes/.env` and do not print any channel id while fixing it. SHIPPED 2026-09-17 as
  [PR #737](https://github.com/webdavis/dotfiles/pull/737), merged `6b957e4e`. Both names are gone from
  both tools. `pns doctor` filters its ledger-derived roster through
  `pns_domain::doctor::routes_to_check`, which drops a retired route before the probe asks the gateway,
  so neither unclearable warning can be printed again; the roster itself still comes from
  `SqliteStore::posted_routes()`, which is why a retired route was reported forever with nothing the
  operator could do. THE REVIEW CAUGHT THAT THE FIRST CUT RETIRED ONLY ONE NAME, leaving `route posture`
  warning on; both are retired now. posture's untiered page route is stated ONCE as `route` in the
  `[notify]` table, defaulting to `posture-pages`: `severity_route` names a route only for Critical
  (`priority`) and returns `None` for every lesser tier, `notify.rs` holds the single default,
  `schema.rs` validates the key as a wire `Name` at parse time, and `alert_sink` falls back to the
  default instead of panicking on a hand-built `Notify`, which the review also found.
  `dot_config/posture/private_config.toml.tmpl` ships the key uncommented at its default. The bare
  `posture` literal had already been renamed once on 2026-09-15 by `b89e1cc9`, so what remained of the
  six sites was two literals for one decision. Both new tests run in under 10 ms. No pns name entered
  posture, no channel id or secret was read or printed, and no live job was run. This UNBLOCKS
  `feat/posture-alert-cutover`: its CRITICAL page keeps `priority` and every lesser page now rides the
  configured route rather than the 404 one that dead-lettered eight digest legs. OPERATOR STEP: a full
  `chezmoi apply` writes the new `route` key and rebuilds both binaries.

- [x] 100. `pns doctor` ends with a false all-clear. DONE 2026-09-17. Filed the same day. The 2026-09-17
  run printed two `THE GATEWAY HAS NO SUCH ROUTE` warnings,
  `1 notification still waiting to reach a channel`, `17 notifications given up on after retrying`, and
  `the daemon log shows it recently failed to record a delivery, so these counts may be low`, then closed
  with `nothing to act on`. The cause is already diagnosed in the unnumbered hermes-security item:
  `RouteVerdict::Missing` maps to `Mark::Warn`, and the summary escalates only on an error, so every
  warning passes through as all clear. In a tool whose only job is to say when a notification did not
  arrive, that is the worst available failure, because it is the line an operator reads when skimming.
  Fix the verdict so any warning in any section, and any non-zero dead-letter or undelivered count,
  withholds the all-clear and names what to look at. Pin it with a test that hands doctor a report
  carrying exactly one warning and asserts the summary is not the all-clear. Note that task 99 removes
  the two route warnings this run produced, so the all-clear bug must be fixed on its own evidence rather
  than waiting for a route to reappear. Side measurement: the dead-letter population was 11 on 2026-09-13
  and is 17 now, so it is growing, and the watchdog reports only an increase rather than the standing
  count.

  SHIPPED 2026-09-17 as [PR #763](https://github.com/webdavis/dotfiles/pull/763), merged `191f2c59`, and
  the defect turned out to have TWO halves rather than the one the filing named. The first is the place
  the filing pointed at: the report's closing decision counted only error rows, so every warning row (a
  missing route, an unreadable or unanswered pairing, an unknown phone tap, an import failure) passed
  straight through and the report closed with nothing to act on. The closing list now collects warnings
  beside issues, COUNTS THE TWO KINDS APART, and numbers every one of them, so the summary names the row
  to look at rather than merely refusing the all-clear. They are counted apart deliberately: an operator
  can FIX an issue and may only be able to READ a warning, since a route retired on the gateway cannot be
  cleared from here.

  The SECOND half was not in the filing and is the more interesting one: the delivery section emitted its
  counts as plain notes, a reading NO summary could ever escalate, so even a correct closing decision
  would have passed a dead-letter population through. That function now returns a mark beside each line,
  with a warning per count and fault (pending, dead-lettered, a growth streak, an unacknowledged alarm, a
  recording gap, an unreadable record), the pointer to `pns failures` as a detail so it is not counted as
  a second finding, and the healthy sentence as good.

  TEST FIRST, and the red runs are recorded: two new closing-list tests failed with the all-clear line on
  the left against the warning count on the right, and the delivery-mark test failed to COMPILE against
  the old string-only return. Both green after the fix. THE DEFECT COULD NOT BE REPRODUCED FROM A LIVE
  RUN, for the two reasons the filing predicted: task 99 had already removed the two route warnings that
  produced the original reading, and a real `pns doctor` run sends a test notification down every
  channel, so the evidence is a report constructed in tests plus the code path read end to end. THE EXIT
  CODE WAS DELIBERATELY LEFT ALONE, so automation reading the exit status is unaffected; withholding the
  all-clear is now visible in the report text instead.

  ONE VISIBLE BEHAVIOUR CHANGE TO EXPECT: every warning producer now withholds the all-clear, and that
  set includes the pairing check's no-answer verdict, so on a machine where moshi-hook is simply not
  installed doctor will from now on close by naming the unchecked approval path rather than saying there
  is nothing to act on. That reads as correct, since the check did not pass, but it is a change and it
  should not be a surprise. The side measurement needed no work: doctor already printed the standing
  dead-letter count and now marks it as a warning that reaches the closing list. What remains is the
  WATCHDOG's alarm shape, which is a design question and was not guessed at: whether it should page on a
  standing population above a threshold as well as on growth, and what that threshold is. The population
  grew from 11 on 2026-09-13 to 17 on 2026-09-17. Filed as task 147.

  Operator step: nothing to arm. The change is Rust source only, rebuilt by the apply-time builder, so
  the next full `chezmoi apply` is what puts it on the deployed binary.

- [x] 101. Make the Rust suites deterministic around real process spawns. DONE 2026-09-17. Filed on the
  operator's ruling to treat this as one task rather than one per test. Task 94 fixed one flake and
  exposed two more of the same shape on the same day, so the defect is the pattern and not the three
  tests. The pattern: a test spawns a real process, waits on a wall-clock budget, and asserts success, so
  under the concurrent agent load this machine actually runs the wait expires and the assertion reads a
  kill or a timeout as a failure. It reddens `main` on code the pull request never touched, which is the
  worst kind of red because it trains everyone to rerun rather than read. This repository already records
  the finding that sub-second waits around real spawns flake continuous integration. The two known
  survivors:
  `channel_dispatch::tests::a_new_registered_destination_dispatches_without_editing_a_name_switch` in the
  `pns` workspace, which failed once under 48 synthetic CPU spinners and passed on every run after the
  load was removed; and
  `uu/crates/uu/tests/interruption.rs:104 sigterm_cleans_owned_children_before_unlocking_and_records_interruption`
  in the `uu` workspace, a signal-timing case that failed once during a `just test-rust` run under load
  and passed three consecutive reruns alone. Neither is tracked anywhere else, checked 2026-09-17.
  Method, following what worked in [PR #718](https://github.com/webdavis/dotfiles/pull/718): audit all
  four workspaces for a test that spawns a process and bounds it by wall-clock time, and for each one ask
  what the spawn is actually there for. Where it exists only to control the child's environment or
  arguments for a decision the parent could make, lift that decision behind a seam and call it in
  process, which is what removed the race in 94 rather than making it cheaper to wait on. Where a real
  spawn is genuinely the behaviour under test, such as the `sigterm` case which is about signal handling
  in a real child, the budget cannot simply be deleted: drive the wait on an observable event rather than
  a duration, and if a duration is unavoidable say so and justify the number. DO NOT simply raise a
  deadline; this repository deletes a test that cannot pass within a second rather than tolerating a slow
  one, so a bigger number trades a flake for a suite that fails the speed rule. Reproduce each flake
  under load before changing it, or state plainly that it could not be reproduced and that the fix is
  reasoned from the code, and prove each fix with at least fifty loops under comparable load plus a
  mutation check. Expect to find candidates beyond the two named; report the full audit even for tests
  left alone, with the reason each was judged safe. SHIPPED 2026-09-17 as
  [PR #731](https://github.com/webdavis/dotfiles/pull/731), merged `7a82c906`. All four workspaces were
  audited and twelve tests that spawn a process and assert success inside a wall-clock bound were fixed
  in three commits. BOTH NAMED SURVIVORS WERE REPRODUCED UNDER 48 SYNTHETIC SPINNERS FIRST, and neither
  root cause was a slow spawn. uu's
  `sigterm_cleans_owned_children_before_unlocking_and_records_interruption` failed 1 in 40 because its
  wait gated on `grandchild-group` existing and then read `child-group`, which the child writes later,
  and `fs::write` publishes a path before its bytes; it reproduces only with its sibling case running
  concurrently, which is how `just test-rust` runs it. pns's
  `a_new_registered_destination_dispatches_without_editing_a_name_switch` failed 2 in 60 because its
  fixture directory is named after `std::process::id()` and never removed, so a run under a recycled id
  met its predecessor's and raised `AlreadyExists`. The fixes: wait on the event the assertion actually
  needs (and replace a 500 ms poll of `/usr/bin/true` with a blocking wait); clear a pid-named fixture
  path before reuse, in three fixtures; and turn the remaining seven sub-second fixture budgets into
  documented fifteen-second LIVENESS bounds, the treatment posture's `usage.rs`, `heartbeat.rs` and
  `funnel_fixture` already carried. No deadline any assertion reads was raised. Proof: 550 runs (55 each
  of ten targets) plus 120 runs of the named pns test at load average 170 to 255 on eight cores, zero
  failures, and eleven mutation checks, every one red. A mutation run showed the liveness bound is paid
  only by a regression: the gutted `recap_with_deadline` took 15.01 s where the passing test takes
  milliseconds.

- [x] 140. One wall-clock budget assertion outside task 101's spawn scope, filed 2026-09-17 from a lane
  failure the same night.
  `busy_ledger_writes_refuse_within_the_budget_without_recording_sensitive_content`
  (`pns/crates/pns-adapters/src/persistence/sqlite/ledger/tests/failures.rs:16`) asserts
  `started.elapsed() < Duration::from_millis(100)` and failed a `just ship` run on a branch that never
  touched that file, while six lanes were compiling at once. Task 101 audited tests that SPAWN a process;
  this one spawns nothing, so it was out of that audit's scope and is the same family by a different
  route: a wall-clock number an assertion reads, under load it cannot control. The behaviour worth
  pinning is that a busy ledger write REFUSES rather than blocking, and that it records no sensitive
  content; the 100 ms is a proxy for refusing promptly. Replace the proxy with the refusal itself (assert
  the error the busy path returns, and that no sensitive content reached the store), or bound it the way
  101 bounded a liveness case, and sweep for any sibling that asserts a duration without a spawn. Do not
  simply raise the number.

  RECURRED ON CI, 2026-09-17.
  `cli::run::tests::pending::the_parsed_pending_threshold_uses_its_own_file_and_resets_after_completed_work`
  in the uu workspace failed CI on the slice 3 pull request, panicking at
  `uu/crates/uu/src/cli/run/tests/support.rs:68` with `1.013007167s` against a one-second budget. The
  rerun passed. This is the same class exactly, a wall-clock budget assertion outside task 101's spawn
  scope, and it is now observed on a GitHub runner rather than only under sibling-lane load, so the
  budget is too tight for CI and not only for a loaded laptop.

  DONE 2026-09-19 together with task 152, [PR #807](https://github.com/webdavis/dotfiles/pull/807),
  merged `b9d2f6a12`. Posture fixtures across posture-adapters and the posture crate that built a scratch
  directory from the process id alone, or from a pid plus an in-run counter but no epoch nanosecond,
  collided with AlreadyExists on a recycled process id and never removed their directory; every affected
  fixture root now folds in the epoch nanosecond and is removed by a Drop guard when its test ends. A
  wall-clock budget assertion in the pns ledger's busy-write test, and a sibling with the same shape in
  the ledger health test, were replaced with the typed refusal already available from each call, and a
  sweep of both workspaces found no other duration assertion outside a process spawn or an
  already-documented liveness bound.

- [x] 141. A future timestamp reads as maximally fresh instead of unknown, filed 2026-09-17 out of B25's
  disposition. `age_of` (`pns/crates/pns-domain/src/decision/reading.rs:48`) `saturating_sub`s a
  `taken_at` that is in the future, which yields age 0, the freshest possible reading, where the
  function's own stated policy for a clock it cannot read is `None`, meaning unknown. In a presence
  engine that matters in one direction: a future marker or input timestamp makes the surface look freshly
  touched, so an alert that should have gone to the phone stays on the banner. A clock that went
  backwards, a file restored from a backup, or a marker written by a device with a skewed clock all
  produce it. The fix is stated with its scope and its cost in
  `docs/superpowers/specs/2026-09-17-nag-tolerance-and-future-timestamp-disposition-design.md`: return
  `None` for a future `taken_at` rather than saturating to zero. THE DISPOSITION IS SCOPED TO `age_of`
  ALONE and deliberately does not sweep the other `now.saturating_sub(timestamp)` sites in the crate,
  because no evidence was gathered that a future timestamp is reachable at those sites or changes their
  verdict. WAITS ON THE OPERATOR: confirm or reject that disposition before a pull request implements it,
  since it changes what an unknown reading does to a delivery decision.

  DONE 2026-09-19, [PR #805](https://github.com/webdavis/dotfiles/pull/805), merged `fe8ab37c0`. `age_of`
  in pns-domain's surface reading (`pns/crates/pns-domain/src/decision/reading.rs`) returned `Some(0)`,
  the freshest possible age, for a `taken_at` timestamp later than the decision clock, instead of the
  function's own stated policy for an untrustworthy clock, `None`, unknown. A future marker or phone
  timestamp, from a clock that went backwards, a marker restored from a backup, or a device with a skewed
  clock, made the desk or phone look freshly touched, holding an alert on the banner that should have
  reached the phone. The fix returns `None` when `taken_at` is later than now, scoped to `age_of` alone
  per the disposition note, with unit tests for the future, equal and past `taken_at` cases plus a
  decision-level test proving a future phone timestamp now routes to the phone getting carded instead of
  being suppressed as already watching. The presence-and-visibility spec's readings section got one added
  sentence naming the future case.

- [x] 142. The four posture test dispositions 60a deliberately deferred, filed 2026-09-17 with named
  triggers so they stop reading as a vague remainder. All four are recorded in
  `posture/docs/test-baseline.tsv` with their own disposition field. (1) FIVE JQ AND PIPE FAULT-INJECTION
  PROPOSALS, disposition `proposed-disposition`, and NONE OF THEM HAS A RUST MECHANISM LEFT TO TEST:
  B140/S123 assumed a jq encoder process that no longer exists, and S123 explicitly allows a fragment, so
  it must not be restated as an atomic-write guarantee; B170/S068's stdout page-candidate pipe is a typed
  `JudgedBatch` now and a failed sink returns `Retained` with no checkpoint; B171/S042's severity is
  in-process. Each needs a fresh disposition against the Rust shape or an explicit retirement, not a
  port. (2) THIRTEEN LEGACY QUEUE LEAVES, disposition `retained-legacy`, keeping their Bash owner: four
  drain-continuation integration cases (an undecodable poison row, a permanent poison row, errexit on a
  failing first row, and a mixed-batch full drain) and nine alert-dispatch unit cases (a count probe
  never creating the database it reads, a counter reading zero while its table is un-bootstrapped, the
  apostrophe cases through dead-letter reason, page URL, request id and drain SELECT, and an unreadable
  store failing the probe rather than reporting a false zero). TRIGGER: task 49's acceptance. (3) B001, a
  detached child never wedging the lock through a leaked descriptor, has no native assertion for
  `SingleRunLock` inheritance; the similarly named exec test covers `AllowlistWriteLock`, a DIFFERENT
  lock, so the coverage that looks present is not. (4) B002, two parallel runs delivering a batch exactly
  once, has no retained two-process test asserting one notification and the shared final cursor together.
  Both of those need a second real process, which is the only way the assertion means anything; TRIGGER:
  the same task 45b caller and exit acceptance the jq and pipe rows wait on, because that is where a
  second process gets a defined exit contract.

  DONE 2026-09-19, [PR #815](https://github.com/webdavis/dotfiles/pull/815), merged `e4e19eaf1`. Task 49
  deleted the Bash queue on 2026-09-15, so every `retained-legacy` row named a test owner that no longer
  exists, and each of the twenty rows was decided against posture's Rust shape rather than ported by
  habit. Two rows became real two-process tests, because that is the only way their assertion means
  anything: B001 now spawns a live child while `SingleRunLock` is held and proves a second run takes the
  released lock while that child still runs
  (`posture/crates/posture-adapters/src/results_cursor/tests.rs`), and B002 spawns two `posture alert`
  children over one seeded results log and asserts exactly one producer call, exit 0 from both, and one
  shared final cursor (`posture/crates/posture/tests/alert_contention.rs`). B002 did not stay `partial`:
  task 45b closed and `posture alert` is a real subcommand with a documented exit contract, so the second
  process could be driven to a defined exit. Five counter and probe rows map to the read-only queue
  reader's own tests, whose successors the baseline had already recorded as pending. The other thirteen
  retire with the Rust shape that made the Bash assertion meaningless named in each row: the four
  drain-continuation rows because there is no drain loop, the four apostrophe rows because the queue
  reader opens read-only and binds its one value as a parameter leaving no writer to corrupt, and the
  five jq and pipe proposals because the encoder process, the candidate pipe and the severity subprocess
  are all gone, replaced by one write of a complete line, a typed batch whose failed sink returns
  retained with no checkpoint, and an in-process severity gate. B140 was recorded as a mechanism
  retirement rather than restated as an atomic-write guarantee. The posture workspace is green on its
  own.

- [x] 143. The heartbeat cannot tell you it failed to deliver, filed 2026-09-17 from task 50a's evidence
  pass. `posture-application/src/heartbeat.rs` discards the submission result
  (`let _ = self.sink.submit(...)`), so the job exits 0 whether the page reached its destination or not,
  and `launchctl`'s last exit code is therefore NOT evidence of delivery. Both job logs are 0 bytes and
  predate the cutovers, so the log says nothing either. That is the same class of defect as task 102, a
  job that is silent about its own failure, and it is worse here than elsewhere because the heartbeat
  exists precisely to prove the pipeline is alive: a heartbeat that cannot deliver and exits 0 reports
  health it did not verify. Make the result reach something the operator can read, a non-zero exit or the
  local banner or both, and pin it with a test that a refused submission does not look like a successful
  run. Check `digest` and the other one-shot jobs for the same discarded result while you are in there.

  DONE 2026-09-19, [PR #816](https://github.com/webdavis/dotfiles/pull/816), merged `a43bf469e`. Made
  every posture one-shot job answer for its own delivery. `Heartbeat::run` returns its submission instead
  of discarding it, and `posture heartbeat` lends its last-resort banner to the sink and takes it back: a
  refused or failed submission writes one stderr line naming the route and the reason, raises that line
  on the local banner, and exits 1, so launchctl's last exit code is now evidence of delivery rather than
  of the process having started. A delivered heartbeat still exits 0 with empty streams. The cursor-reset
  warning in the results judge now travels out beside the batch outcome and `posture alert` writes a line
  when it reached nobody, keeping its documented exit 0 because the batch was judged and checkpointed on
  its own terms. The funnel's corrupt-baseline warning is carried to the end of the run, so the repair
  happens whatever the warning did and the job exits 1 with a line when the warning was lost.
  `posture digest` and `posture alert` already branched on their sink's answer, so neither changed.
  Pinned by library tests over a fixture producer command and a fixture alarm: a refused heartbeat exits
  1 with the exact stderr line and the banner, a delivered one exits 0, a refused reset warning is
  reported out, and a refused corruption warning fails the funnel run after the baseline was repaired.

- [x] 144. Adopt the three native macOS probes the 2026-09-17 measurements recommend, filed the same day.
  The measurements and the per-probe verdicts are in
  `docs/superpowers/specs/2026-09-17-native-macos-probe-evaluation.md` and are not re-derived here: adopt
  the IOKit idle property in place of `ioreg -c IOHIDSystem` (44.47 ms to 0.0100 ms), the registry Root
  node's `IOConsoleLocked` in place of `ioreg -n Root -d1`, and a libproc walk in place of `pgrep -P`
  (26.34 ms to 1.0360 ms, with no new dependency since libc already declares the calls).
  `pgrep -x mosh-server` and `ps -o tty=` stay shelled, and the second disappears on its own once the
  walk lands. Expected stage cost falls from about 73 ms to about 27 ms. WAITS ON ONE OPERATOR DECISION,
  which no measurement can make: a native in-process call cannot be killed by the forked cleanup child
  that bounds every probe today, so adopting these trades the interruptible five-second probe deadline
  for the speed. If that trade is refused, this task is closed as declined rather than left open. If it
  is accepted, four acceptance gates are still unexercised and belong to the build: a real lock and
  unlock transition, an unreadable device, a multi-user session, and a stalled native call. One
  prerequisite for ever taking the NAME-based pgrep native, which this task does not: read out of source
  which field macOS pgrep itself matches against, since only the manual page's wording and one observed
  agreement support the current reading.

  DONE 2026-09-19, [PR #821](https://github.com/webdavis/dotfiles/pull/821), merged `d2a3f36f1`. Adopted
  the three native macOS probes the measurements recommended, after the operator accepted the
  interruptible-deadline trade on 2026-09-19. The desk idle reading now comes from the IOKit
  `HIDIdleTime` property on the `IOHIDSystem` service and the console lock from `IOConsoleLocked` on the
  registry Root node, both in process and neither through `CGSessionCopyCurrentDictionary`; the phone's
  clients and their controlling terminals come from one libproc walk comparing `pbi_ppid` and resolving
  `e_tdev` with `devname`, which retired the `ps -o tty=` spawn along with `pgrep -P`.
  `pgrep -x mosh-server` stayed shelled as the evaluation asked. No dependency was added: five IOKit
  symbols, six Core Foundation symbols and `devname` are declared in two extern blocks smaller than the
  binding crates would have been. The five-second deadline was kept rather than traded away, by running
  each native read on its own thread and taking the answer with a timed receive on the probe thread, so a
  wedged call leaks one stack in a short-lived process and never holds the notification; the fall
  direction is unchanged, with an unreadable property, a wrong type, a short record or a blown deadline
  all reading as unknown. The four gates the evaluation left open were exercised: the lock transition
  live in the locked state and both ways through an injected registry, an unreadable device and a stalled
  call through injected readers, and a multi-user session against five concurrent mosh sessions whose
  five terminal names the walk and the retired commands agreed on exactly. Measured on the same loaded
  machine, the desk pair fell from 90.6 ms to 0.13 ms and the phone chain shed the 48.5 ms of its two
  removed spawns.

- [x] 146. Give the allowlist write lock an audit trail, filed 2026-09-17 from the approval-interface
  reconciliation. `AllowlistWriteLock` takes a blocking exclusive lock on a sidecar of the deployed
  allowlist and records NOTHING: no identity, no timestamp, no verb. That makes it a concurrency contract
  rather than an audit trail, which matters because the same reconciliation established that a grant
  through the SUPPORTED writer is SILENT (the integrity verdict is log-only when the manifest vouches for
  the new content, and the publisher refreshes that manifest as its last step) while a HAND EDIT pages.
  So the only path that suppresses a security finding is also the only path that leaves no trace. DONE
  MEANS: every acquisition that goes on to WRITE records who asked, when, and which verb with which
  label, somewhere durable enough to answer "who suppressed this finding and when" weeks later, and a
  test pins that a write with no such record cannot happen. What it must NOT become is a liveness or
  approval mechanism; this is a record of what happened, not a gate on what may happen. Read the
  reconciliation document first, because it names the exact call sites and says why the announcement this
  was assumed to get for free does not exist.

  DONE 2026-09-19, [PR #817](https://github.com/webdavis/dotfiles/pull/817), merged `f43b297f7`. The
  allowlist write lock now carries an audit trail. A held `AllowlistWriteLock` guard appends one JSON
  line to a sidecar audit file beside the existing lock sidecar, before the write it covers is published:
  the RFC 3339 UTC instant, the verb (allow or deny), the label, the invoking uid and passwd name, and
  the parent process name where the kernel still has one. The line is flushed to disk and a failed append
  refuses the curation before the publisher runs, so a published write with no record cannot happen. The
  record is a bound on the lock's own guard rather than a separate collaborator, so a caller cannot hold
  the lock and skip it. It gates nothing: no acquisition is refused for want of an approval and no verb
  is newly blocked, and `posture doctor` is unchanged. The audit file is not manifested, so its own
  writes fall to a log-only integrity verdict rather than paging as tampering, exactly like the lock
  sidecar. Four tests pin the behavior, two in the adapter and two in the application, all well under a
  second.

- [x] 147. Decide the watchdog's dead-letter alarm shape, filed 2026-09-17 out of task 100's evidence.
  The watchdog reports only an INCREASE in the dead-letter population, never the standing count, so a
  population that stops growing stops being mentioned however large it is. It was 11 on 2026-09-13 and 17
  on 2026-09-17. Task 100 fixed the half that was a defect: `pns doctor` already printed the standing
  count and now marks it as a warning that withholds the all-clear and names it. THE REMAINING QUESTION
  IS ALARM POLICY AND IS NOT A ONE-LINE CHANGE, which is why task 100 deliberately left it alone: should
  the watchdog page on a standing population above a threshold as well as on growth, and what is that
  threshold? A wrong answer here is expensive in both directions, since a low threshold pages nightly
  about a number nobody is going to drain and a high one restores exactly the silence task 100 was filed
  against. Decide the policy before writing any code, and settle in the same breath whether the 17
  currently dead-lettered legs are drained, retried or accepted as a permanent floor, because the
  threshold means nothing until that is known.

  Decided 2026-09-20. Policy: the watchdog keeps paging on GROWTH only; no standing-population threshold,
  because the standing count now has two other homes: `pns doctor` (task 100) and one line in `pns recap`
  (task 170, its own small PR once slice 53 has landed). The 20 dead-lettered legs measured on 2026-09-20
  are an apply-window artefact, not a floor: all 20 are `bad URL` (`TransportOutcome::NoStatus`, empty
  route and destination), 18 of them from 18:11 to 18:42 on 2026-09-19 inside the config-rename window
  before the 01:26 apply and 2 from 09-17, with none since. They are drained, not retried: nothing they
  carried can be delivered to an empty destination. `pns failures drain`
  ([PR #861](https://github.com/webdavis/dotfiles/pull/861), merged `382a4dc04`) acknowledges
  dead-lettered legs only and keeps every attempt row; the operator runs it once after the next apply.
  The listing's id column now derives its width from the widest id on the page, in the same PR.

- [x] 148. Decide whether the Discord platform toolset keeps unprompted shell authority, filed
  2026-09-17. THIS IS THE BOUNDARY THE RETIRED APPROVAL DESIGN WAS ACTUALLY ABOUT, and it was measured
  against the installed hermes rather than reasoned about: the live Discord platform toolset carries
  `terminal` with a local backend, `sudo -n true` exits 0, the managed shell puts the Rust tools' install
  directory on the PATH a hermes shell sources, and BOTH approval gates allow the posture allowlist
  writer's add verb, so with no warnings collected the approval path returns approved with no prompt at
  all. Every Discord hermes agent therefore already holds the authority the proposed `/osquery allow`
  skill would have named, which is why that skill shrank to its `list` verb: it would have been a label
  on a door that is already open. The 2026-09-14 design's instinct to gate the recovered security
  investigator on a pending scope does not close this, because the door is open to every agent on the
  platform and not only to that one. DONE MEANS a decision, not code: either the toolset is narrowed (and
  this ledger records what it loses, since narrowing it changes the scope of every hermes slash command,
  not just this one), or the open authority is ACCEPTED IN WRITING with the reason, so no later task
  re-derives the same finding and no later design assumes a boundary that is not there. Until it is
  decided, treat any agent on that platform as able to suppress a security finding. Evidence:
  `docs/superpowers/specs/2026-09-17-posture-approval-interface-reconciliation.md`.

  Decided 2026-09-20: ACCEPTED. The Discord hermes platform toolset keeps unprompted shell and sudo
  authority, unchanged. The operator's reason, verbatim: "accept, don't narrow. i'm okay with the agents
  having access." Nothing is narrowed and no scope changes for any hermes slash command. Consequence
  recorded so no later design assumes a boundary: every agent on the Discord platform can run shell
  commands, including the posture allowlist writer's add verb, without a prompt, and the recovered
  security investigator is not gated any differently from its neighbours.

- [x] 145. Say in the shared agent rules who the gh-axi preference binds, filed 2026-09-17 because TWO
  LANES HAVE NOW HAD TO DERIVE IT FROM FIRST PRINCIPLES. `.chezmoitemplates/global-agent-rules.md` says
  to prefer the `gh-axi` skill over the raw `gh` CLI for every GitHub operation, and never to invoke `gh`
  directly. Read literally that sentence reaches the Rust products this repository ships, and twice now
  it has been read that way and sent a lane to move a product's own subprocess onto `npx -y gh-axi`: once
  inside task 36a's own history, which recorded the disagreement as an open question for the operator,
  and once in the 2026-09-17 brief that tried to settle it in that direction. Both times the lane worked
  out on its own that the rule governs what an AGENT invokes and not what a binary other people install
  with `cargo install` spawns, and the second time it measured that obeying the literal reading would
  lose capability (`gh-axi pr list` has no `--json` and no window flag, `gh-axi search prs` no
  output-format flag at all). The fix is one sentence in the partial saying the preference binds agents
  doing GitHub work, and that a Rust product this repository ships chooses its own subprocess on its own
  merits, the way it chooses `git`. DONE MEANS: that sentence is in
  `.chezmoitemplates/global-agent-rules.md`, both rendered copies carry it, and neither
  `pns/crates/pns-adapters/src/recap/github_cli.rs` nor a future lane needs to re-derive it. THE WORDING
  IS THE OPERATOR'S CALL, since it is their ruleset and the reading above was inferred rather than
  stated: if they intend the rule to reach shipped products too, the seam at `github_cli.rs` is the one
  place that would change, and the capability loss above is the price.

  DONE 2026-09-19, [PR #814](https://github.com/webdavis/dotfiles/pull/814), merged `f072ba291`, per the
  operator's 2026-09-19 ruling that the gh-axi preference binds only the agent invoking a GitHub command,
  not a Rust product this repository ships on its own merits. Two clarifying sentences were appended to
  the gh-axi bullet in `.chezmoitemplates/global-agent-rules.md`.

- [x] 149. The test capture helper takes its request count positionally behind the status, filed
  2026-09-17 from slice 6's evidence. `Capture::start` in the pns test support takes an optional status
  first and an optional request count second, so a call meaning "no status, two requests" passed in the
  other order makes the capture answer HTTP status 3 rather than refusing the call. It is silent: the
  test then asserts against a status nobody chose, which is how slice 6 met it while moving a wire test
  from the home command to doctor. Give the two a shape that cannot swap, a named argument or a builder,
  and pin the refusal of a status that no caller asked for.

  DONE 2026-09-19, [PR #813](https://github.com/webdavis/dotfiles/pull/813), merged `17a7aff0a`. The pns
  test capture helper no longer takes its status and request count as a swappable positional pair.
  `Capture::start(&sandbox, name, Option<&str>, Option<&str>)` became
  `Capture::builder(&sandbox, name).status(u16).requests(usize).start()`, so each value is named by the
  method that sets it and neither can take the other's place. The builder refuses a status outside 100 to
  599 at the call site with a panic naming the value, and a capture that names no status still answers
  200 as every previous caller relied on. Both arguments are passed to http-capture on every start, so
  its positional pair is never read one argument short. All eight callers moved, and two twin tests pin
  the refusal and the accepted status.

- [x] 150. One scalebar regression test hangs the whole suite, filed 2026-09-17 while building tasks 129
  and 130. `node Tests/workflow-regressions.test.cjs` and `Tests/workflow-lifecycle.test.cjs` never
  return. The cause was bisected and it is NOT the size of the argument payload, which an earlier reading
  guessed: handing the identical payload to Obsidian from a file hangs the same way, and a 120 second
  timeout does not help. Every test passes when run alone. The test covering replacement discarding the
  original draft and its timers opens a modal dialog in its frame, and it hangs whenever it shares one
  evaluation with any other test. So the bridge is sound and the suite has a test-isolation defect around
  that modal. Until it is fixed, the suite's documented entry point cannot be used, and tasks 129, 130
  and 131 were each verified through a filtered payload that runs one test at a time. Work lands as a
  pull request on `~/workspaces/Ivy/webdavis/scalebar`.

  DONE 2026-09-19, [webdavis/scalebar PR #9](https://github.com/webdavis/scalebar/pull/9), merged
  `1333384`. The root cause was Chromium's hidden-window nested-timer ceiling, not the modal: `settle()`
  now drains microtasks, and the whole scalebar suite is green.

- [x] 151. The scalebar Swift catalog helper aborts on a nil unwrap, filed 2026-09-17 while building task
  129\. `node Tests/workflow-core.test.cjs` reports 36 pass and 1 fail on untouched `main`, and the
  failure is the compiled helper `catalog-check` aborting with
  `main/main.swift:16: Fatal error: Unexpectedly found nil while unwrapping an Optional value`, killed by
  SIGTRAP. It is unrelated to the widget work and it means the one plain node test file in the suite
  cannot go green, so no scalebar change can be gated on a clean run of it. Find what line 16 unwraps and
  why it is empty here, and either give it a real value or a refusal that names what is missing. Work
  lands as a pull request on `~/workspaces/Ivy/webdavis/scalebar`.

  DONE 2026-09-19, [webdavis/scalebar PR #8](https://github.com/webdavis/scalebar/pull/8), merged
  `f9be72b`. The catalog check asked for a frozen fixture date and now binds `todayDate`. The follow-up
  [webdavis/scalebar PR #10](https://github.com/webdavis/scalebar/pull/10), merged `8d97082`, stopped the
  hidden-window timers and `requestAnimationFrame` in the schedule-widget and run-obsidian-tests entry
  points, pumps the frame queue explicitly and dispatches close events for closed panels.

PROCESS NOTE, 2026-09-17: CI never starts on a head pushed while GitHub had the pull request marked
dirty. Measured on pull requests 770 and 772. Both lanes hit a conflict after a sibling merge, merged
`origin/main` in their worktree, and pushed. GitHub then reported `mergeable_state: blocked` with ZERO
check runs on the new head: `repos/webdavis/dotfiles/commits/<sha>/check-runs` returned `total_count: 0`,
and `gh-axi pr checks` printed "this PR has no CI checks configured", which is the same string a
repository with no workflow prints. Because `lint` is a required status check on `main`, the pull request
stays blocked with nothing to wait for, and a ship agent polling checks waits forever on a queue that
will never fill. `gh-axi run rerun` cannot help, because a rerun needs a run id and no run exists for
that head. Closing and reopening the pull request fires the `reopened` activity type, the workflow runs,
and the pull request unblocks. Both recovered that way, from zero checks to two pending within 25
seconds. Any ship stage should therefore read the check runs for the NEW head sha after resolving a
conflict and pushing, rather than the pull request's check summary, and treat "no CI checks configured"
on a repository that HAS a workflow as a missing trigger rather than as an absent pipeline.

- [x] 152. A pns rust test fixture names its directory by process id and never removes it, filed
  2026-09-18 from the slice 2 lane's evidence. `posture/crates/posture/tests/usage.rs:147` names its
  fixture directory after `std::process::id()` with no cleanup, and 685 stale `posture-metadata-*`
  directories had piled up in the system temporary directory (counted 2026-09-17); a REUSED process id
  collides with `AlreadyExists`, which is why `just test-rust` and `just test` can go red on a clean
  tree. Task 101 fixed the same shape in three pns fixtures but did not reach this one. Give the
  directory a unique name and remove it on completion, the way task 101's three fixtures were fixed; the
  685 existing directories need one `trash` by the operator, run non-destructively rather than by an
  agent.

  DONE 2026-09-19 together with task 140, [PR #807](https://github.com/webdavis/dotfiles/pull/807),
  merged `b9d2f6a12`. The posture fixture roots now fold in the epoch nanosecond and are removed by a
  Drop guard when their test ends; see task 140's DONE line for the full evidence. The 685 existing stale
  directories still need one `trash` by the operator, run non-destructively.

- [x] 153. posture hardcodes an engine's argv verb in its own source, filed 2026-09-18 from the slice 2
  lane's evidence. `posture/crates/posture/src/lib.rs:90` composes a producer command by name, which the
  name-no-engine ruling argues against; it predates slice 2, which only renamed the word rather than
  redesigning it. posture should take the whole producer argv from its own config rather than composing a
  verb it knows by name, the way `[notify] mode = "command"` already lets it hand a page to a producer
  command its own config names.

  DONE 2026-09-19, already satisfied on main: posture takes the whole producer argv from its own config,
  `[notify.command] path` plus `arguments` passed verbatim
  (`posture/crates/posture-adapters/src/notify/schema.rs`, `into_mode`), and a command mode with no
  command is refused rather than defaulted to an engine's name. The only `send --json` left in posture
  source is the `cfg(test)` fixture helper in `posture/crates/posture/src/lib.rs`, which drives an owned
  fixture command, not an engine. The `pns.result/1` schema tag in fixtures is the wire contract's name
  and is covered by the wire-crate task above.

- [x] 154. The three extracted herdr plugin repositories and the two Todoist plugin repositories have no
  CI, filed 2026-09-18. `just test-rust` used to run cargo test, fmt, clippy and doc over
  herdr-smart-nav, herdr-workspace-jump and herdr-process from this repository, and after task 68a's
  extraction nothing does. `webdavis/herdr-todoist` and `webdavis/todoist.nvim` never had a dotfiles gate
  either, and all five repositories' pull requests merge with "this PR has no CI checks configured";
  every gate run against them tonight was run by hand in a lane's worktree, which is real evidence but is
  not a gate anyone else's change has to pass. Each needs its own workflow.

  DONE 2026-09-19: CI added in all five repositories. `webdavis/herdr-smart-nav PR #1`,
  `webdavis/herdr-workspace-jump PR #1` and `webdavis/herdr-process PR #1` each added a workflow on
  `macos-latest`. `webdavis/herdr-todoist PR #12` added one with `--workspace` so `crates/todoist` is
  covered. `webdavis/todoist.nvim PR #13` added lint and test, with Neovim 0.12.5 pinned by sha256
  tarball.

- [x] 155. A pin bump in the herdr plugin roster does not move an already-installed plugin, filed
  2026-09-18, raised as an open question by the task 68a lane. herdr v1 has no `plugin update`, so
  refreshing a plugin means reinstalling, and today a changed pin in `.chezmoidata` lands only in uu's
  weekly herdr-lane drift report. If that report goes unread, the two plugins the operator owns sit at
  whatever commit they were installed at regardless of what the roster says.

  DONE 2026-09-17: a `ref:` bump in `packages.herdr_plugins` now moves the plugin. The apply-time
  installer had been gated on the plugin id alone, so an already-registered plugin was skipped whatever
  revision it sat at, and a changed pin lived only in uu's weekly drift report. `run_after_53` now reads
  the revision `herdr plugin list --json` records for each roster plugin (`source.requested_ref` first,
  then `resolved_commit` by prefix from seven characters, the same rule uu's herdr lane applies to the
  same roster) and reinstalls whatever no longer matches its pin, printing one line per plugin it moved
  and nothing when every plugin is where the roster says. herdr v1 still has no `plugin update`, and none
  is needed: herdr's own documentation states that an install over a GitHub-managed plugin replaces that
  managed checkout, so nothing is uninstalled and no removal mechanism entered the repository. The
  comparison rides the existing run_after loop rather than a new run_onchange script keyed on the roster
  hash, because chezmoi records a run_onchange script as satisfied on any exit 0 and every
  herdr-unreachable branch here exits 0, so one scripted apply with the server down would have consumed
  the trigger and lost the pin bump; running every apply also costs no extra socket call, since the ref
  arrives in the listing the presence probe already fetched. An unpinned entry is still left where it is
  and refreshed by uu's weekly run. Five bashunit behaviours in
  `test/unit/herdr-plugin-pin-reinstall.test.sh` drive the rendered script against a stubbed herdr: a
  matching pin installs nothing and prints nothing, a moved pin reinstalls with the pinned ref, a plugin
  no listing reports is installed, an unpinned entry is left alone, and a pin naming the recorded commit
  is held. [PR #839](https://github.com/webdavis/dotfiles/pull/839), merged `dc5d58d34`.

- [x] 156. herdr-workspace-jump's public manifest is one action per workspace, filed 2026-09-18, raised
  by the task 68a lane. Every new project workspace means editing a file in another repository, pushing,
  bumping the pin in dotfiles and reinstalling, which is strictly more work than the vendored build this
  repository used to ship. Worth revisiting if the workspace set churns.

  DONE 2026-09-20: [PR #862](https://github.com/webdavis/dotfiles/pull/862), merged `056b5dd8d`, with the
  plugin's own PR webdavis/herdr-workspace-jump #2 merged `37d8a3104`. herdr-workspace-jump moved onto
  the link path alongside herdr-process, closing task 156. The plugin's own pull request
  (feat/generate-manifest-from-config) adds a `generate` subcommand that renders its herdr-plugin.toml
  from the operator's own [workspaces] config, so keys and workspace routes are no longer baked into a
  committed manifest. In dotfiles, packages.herdr_linked_plugin became packages.herdr_linked_plugins (a
  two-entry list), herdr-workspace-jump's entry left the herdr_plugins install roster, a new
  dot_config/herdr/plugins/config/herdr-workspace-jump/config.toml declares the nine workspace labels and
  directories, and run_onchange_after_58-build-herdr-process-plugin.sh.tmpl was renamed to
  run_onchange_after_58-build-herdr-linked-plugins.sh.tmpl and generalized to loop the checkout, build,
  manifest and registration phases over both plugins. CLAUDE.md's herdr sections were updated to match.
  This dotfiles PR must not merge before the plugin's own pull request merges, since the pinned revision
  is the tip of that still-open branch.

  The operator runs `herdr plugin uninstall herdr-workspace-jump` before the next apply so the linked
  build can own the id.

- [x] 157. A low-severity dependabot alert on herdr-todoist, GHSA-rhfx-m35p-ff5j, filed 2026-09-18. `lru`
  before 0.16.3 has a soundness issue in `IterMut`, which violates Stacked Borrows by invalidating an
  internal pointer. It is not a quick lockfile bump: the dependency is transitive through
  `ratatui v0.29.0`, which pins `lru = "^0.12.0"`, so `cargo update -p lru --precise 0.16.3` is refused
  outright. Closing it means upgrading ratatui, which is the whole drawing surface of the pane, so it
  wants its own task and its own test run rather than a drive-by. Severity is low and the crate is used
  only inside ratatui's own rendering.

  DONE 2026-09-20: webdavis/herdr-todoist PR #15, merged `f45887b`. ratatui moved to 0.30.2, which
  resolves lru at 0.18.4, past the 0.16.3 fix for GHSA-rhfx-m35p-ff5j; 299 tests green; dependabot PR #14
  closed as superseded.

- [x] 158. Give todoist.nvim's picker a subtask count so completing a parent can ask before it cascades,
  filed 2026-09-18 out of task 122's merge. `x` in the list asks before completing a parent with open
  subtasks, because `POST /tasks/{id}/close` cascades to subtasks server side; task 121's picker calls
  the complete write directly and asks nothing, so completing a parent from the picker silently closes
  its subtasks. The picker builds its entries from tasks rather than from the list buffer, so it has no
  tree to count children in; giving it one is its own change.

  DONE 2026-09-20: webdavis/todoist.nvim PR #14, merged `49f0ea2`. The picker asks before completing a
  parent with open subtasks, through the same confirm the list path uses; the review's five findings were
  fixed in place, 259 tests green.

- [x] 159. A sandbox wall-clock assertion is measured tight under concurrent load, filed 2026-09-20.
  `dispatch::hermes_lines::every_hermes_outcome_an_event_can_reach_prints_exactly_what_it_printed_before`
  (`pns/crates/pns/tests/support/sandbox.rs`, 5000 ms ceiling) measured 5.1 to 9.6 seconds under fourteen
  concurrent cargo runs on 2026-09-19, and passes in CI. Re-measure once the lanes are quiet and either
  fix the fixture or give it an `allow_slow` reason; never raise the number.

  DONE 2026-09-20: measured with one lane running,
  `dispatch::hermes_lines::every_hermes_outcome_an_event_can_reach_prints_exactly_what_it_printed_before`
  ran in 0.72, 0.74 and 0.79 s alone against the 5000 ms ceiling, so the fixture needs no change and the
  2026-09-19 reading of 5.1 to 9.6 s was load; the class of load-sensitive budgets is task 164.

- [x] 160. Collapse the hand-copied posture fixture guards onto the shared Sandbox type, filed
  2026-09-20. Skipped by the PR #807 reviewer as a real refactor rather than a fix-round item: several
  posture fixtures now carry their own epoch-nanosecond scratch-directory guard, duplicating logic the
  shared pns Sandbox type already owns.

  DONE 2026-09-17: collapsed onto the shared Sandbox type. Thirty-five test files across `posture` and
  `posture-adapters` carried their own scratch-directory helper, a pid-and-epoch-nanosecond name or an
  `impl Drop` that removed the tree; each now takes a Sandbox instead, the helpers that returned a bare
  path hand the sandbox back beside it so the caller holds the guard, and the helpers that leaked their
  directory outright gained one. The five integration tests under `posture/crates/posture/tests` share a
  new `tests/sandbox/mod.rs`, because an integration test cannot reach the crate-private copy, and it
  carries the same two assertions pns's copy has, including that a fixture directory is gone once the
  guard drops. Test behaviour is unchanged: the fixtures that need a canonical path canonicalize the
  sandbox, the spool fixtures still let the appender create the directory whose 0700 mode they read back,
  the converge scratch keeps its explicit 0700 chmod, and the two tests that assert a path is absent
  point at a child of the sandbox. Net 593 deletions against 390 insertions, with `cargo fmt`,
  `cargo clippy -D warnings`, `cargo test --workspace`, `just test-rust` and `just lint-check` all green.
  The three remaining test `impl Drop`s in `posture-application` are in-memory test doubles rather than
  directory guards and were left as they are. [PR #842](https://github.com/webdavis/dotfiles/pull/842),
  merged `39cf74644`.

- [x] 161. Sweep every shelled CLI across pns, uu, posture and lights for native replacements, filed
  2026-09-20 under the operator's 2026-09-19 prefer-native ruling. Task 144 was the first instalment,
  replacing three shelled macOS probes with native IOKit and libproc calls; find the rest.

  DONE 2026-09-20: swept every command the four shipped Rust tools spawn for native replacements, the
  second instalment of the operator's 2026-09-19 prefer-native ruling after task 144 took posture's idle,
  lock and process probes to IOKit and libproc.
  [docs/research/2026-09-20-native-call-sweep.md](https://github.com/webdavis/dotfiles/blob/main/docs/research/2026-09-20-native-call-sweep.md)
  greps `Command::new(`, every absolute program path and every configured program constant across
  `pns/crates`, `uu/crates`, `posture/crates` and `lights/crates`, and tables the 57 distinct commands
  that survive with the file, the reason for the spawn, the native replacement and its concrete Rust
  binding, whether the replacement can block and so needs an interruptible deadline, the per-call saving
  and a verdict each. posture holds nine of the replaceable commands (the three `pgrep` shapes become one
  libproc walk, both `plutil` shapes plus the `defaults read` become one in-process property list read,
  `kill -0` becomes `libc::kill`, `readlink -f` becomes `std::fs::canonicalize`, `file` becomes a
  four-byte Mach-O magic read and `xattr -p` becomes `libc::getxattr`); lights loses the `gtimeout`
  wrapper that a PATH-resolved Homebrew binary silently makes optional; uu loses `/usr/bin/env` from two
  lanes and `/bin/sh -c` from the npm lane once its `CommandRunner` port takes an environment.
  `codesign`, the tailscaled status read, the guest account and the `open` click are deferred with their
  reasons, and every product CLI (`git`, `gh`, `herdr`, `brew`, `npx`, `osqueryi`, `sshd`, the configured
  producer commands) is a keep by construction, as is `launchctl`, whose own SDK header states there is
  no replacement for listing, starting or stopping jobs. Each native claim was read out of a local SDK
  header or the shipped `libc` 0.2.189 source rather than from memory, the two that could not be are
  marked unverified, and the report ends with eight proposed slices ordered by value, each sized for one
  pull request. [PR #838](https://github.com/webdavis/dotfiles/pull/838), merged `c5fa98990`.

  Of the eight proposed slices, four have merged. Sweep slice 1, the pgrep family becomes one libproc
  walk, is [PR #850](https://github.com/webdavis/dotfiles/pull/850), merged `8f02363a6`. Sweep slice 2,
  reading property lists in process, is [PR #844](https://github.com/webdavis/dotfiles/pull/844), merged
  `d656e9a43`. Sweep slice 4, bounding the lights-to-pns spawn in process and dropping `gtimeout`, is
  [PR #849](https://github.com/webdavis/dotfiles/pull/849), merged `6d6c16998`. Sweep slice 5, giving
  uu's `CommandRunner` port an environment, is [PR #847](https://github.com/webdavis/dotfiles/pull/847),
  merged `65c3349ac`. Sweep slice 3, the four one-call replacements, is
  [PR #852](https://github.com/webdavis/dotfiles/pull/852), merged `e9c73273f`: posture's pid liveness
  read moved from `kill -0` to `libc::kill` with signal 0, its LuLu launcher resolution from
  `readlink -f` to `std::fs::canonicalize`, its Mach-O object test from `file` to a four-byte magic read,
  and its quarantine read from `xattr -p com.apple.quarantine` to a sized `libc::getxattr`. Sweep slices
  6, 7 and 8 closed as tasks 165, 166 and 167 below.

- [ ] 162. The pns daemon log on dresden carries recurring state-error lines in bursts, cause unknown,
  filed 2026-09-20. `state error (delivery ledger: database refused the operation)` and
  `(decision: unreadable state record)` appear in bursts; eight parallel probes of
  `sample_delivery_health` against the live store on 2026-09-20 never reproduced it, and deliveries are
  healthy throughout. [PR #833](https://github.com/webdavis/dotfiles/pull/833)
  (`fix/pns-state-diagnostics-name-the-error`), merged `45f82efc7`, makes each line carry the store
  error's own sentence, so the next occurrence after the operator's apply names the refused operation.
  Investigate from that line. The first step landed as
  [PR #833](https://github.com/webdavis/dotfiles/pull/833), merged `45f82efc7`, which makes every
  state-error log line carry the StoreError text, so the next apply names the refused operation.

- [x] 163. Orphaned `pns failures serve` processes hold the port every new daemon's child needs, filed
  2026-09-20. Three `pns failures serve` processes from 2026-09-16 and 2026-09-17 outlived their daemons
  (parent pid 1), and the oldest holds `127.0.0.1:8646`, so every new daemon's child logs
  `could not bind` every 30 seconds. A fix lane (`fix/pns-failures-page-dies-with-its-daemon`) had no
  open or merged pull request as of 2026-09-20
  (`gh-axi pr list --state all --head fix/pns-failures-page-dies-with-its-daemon` returned none), so this
  is IN FLIGHT rather than done. The operator step either way is `kill <pid>` for each orphan and
  `pns gateway restart`.

  DONE 2026-09-17: the failure page stopped outliving its daemon. The page is spawned detached in a
  process group of its own, so a daemon stopped by launchd left it running under pid 1 still holding
  127.0.0.1:8646, and every daemon started afterwards spawned a child that could never bind while the
  page actually served came from a days-old binary. The child now watches the process that started it
  with a kqueue EVFILT_PROC NOTE_EXIT registration and exits once a later getppid reports a different
  parent or the reaper, which also covers a child orphaned before it ever read its own parent; the kqueue
  is only the wake-up and the ppid read is what decides, and a watch that has fired falls back to a
  one-second poll because the one-shot registration is spent. The daemon catches SIGTERM and, on the pass
  after it arrives, signals its page child's group and waits a bounded two seconds before killing it,
  which covers the gateway stop and restart paths since both reach it as launchctl's own SIGTERM; no
  process is ever matched by name, only the pid the daemon spawned. Reading the same code turned up the
  source of the sixteen thousand bind refusals in the log: the page carried the generic delivery bound,
  so the reap killed and respawned the listener twice a minute at the production clock and each new child
  wrote the line once. A child whose work is to stay up now carries no bound, and the refusal itself is
  written once and then only every ten minutes. Three integration tests in the existing daemon-guard
  style and one unit test pin the behaviours, each confirmed red beforehand.
  [PR #836](https://github.com/webdavis/dotfiles/pull/836), merged `59bdf0c0a`. The operator step of

- [x] 164. Four wall-clock budget assertions reddened lanes under sibling-lane load during the 2026-09-20
  overnight run and each passed alone, filed 2026-09-20 under the same task 101 pattern:
  `posture-adapters command::tests::grace::the_deadline_sends_term_before_kill_and_retains_timeout_outcome`
  (the TERM-before-KILL grace test),
  `uu-adapters lanes::nvim::smoke_test::tests::isolation::the_real_smoke_children_keep_external_home_and_discovery_unchanged`
  (a 600 ms lane-isolation deadline, which also failed once in CI on PR #849 and passed on rerun),
  `pns --test daemon lifecycle::a_hung_child_does_not_stall_the_tick_and_is_killed` (a process-readiness
  poll, failed three times in a row on one loaded run of PR #844 and passed alone), and
  `test/unit/pns-shell-notifier-engine-choice.sh` (a 30 s wall-clock elapsed assertion, 29 s measured).
  Widen or restructure each so a loaded machine cannot fail it, following the task 101 method; never
  raise a number blindly.

  DONE 2026-09-20: [PR #866](https://github.com/webdavis/dotfiles/pull/866), merged `aac0e2a68`. All four
  assertions were reproduced or measured under synthetic CPU load first, and each now fails on the
  behavior it pins rather than on the machine's speed, proven by one mutation apiece and ten green runs
  under eight spinners. posture's `the_deadline_sends_term_before_kill_and_retains_timeout_outcome`
  failed 4 runs in 30 under 64 spinners, seven of ten captured failures with no marker at all (the 60 ms
  deadline signalled before `sh` reached its `trap` line) and three with an empty one (the 30 ms grace
  killed the handler mid-write); the root cause was in the product, so `OwnedChild::stop` now ends its
  grace on the child's exit instead of sleeping the whole of it, and the test waits for the fixture's own
  ready file behind a 30 s hang guard before entering the same stop path, keeping the deadline half over
  a child that never exits. uu's `the_real_smoke_children_keep_external_home_and_discovery_unchanged` ran
  its inner lane under a 600 ms budget that also covers the parent's snapshots and acknowledgement
  between phases; the case measured 0.12 s alone against 0.59 s under 192 spinners and had already failed
  once in CI, so the lane now waits on the same 30 s liveness bound its parent uses. pns's
  `lifecycle::a_hung_child_does_not_stall_the_tick_and_is_killed` spent the whole of the harness poll's
  10 s hang guard on three of four runs of the daemon suite under 256 spinners and reported that the hung
  job never started; `poll_until` now waits on the same evidence behind a 30 s guard, and the sandbox
  ceiling note drops the poll from its list because one hung poll now exceeds the CI line as well. The
  shell case in `test/unit/pns-shell-notifier-engine-choice.sh` asserted an exact 29 s elapsed across two
  windows of bash's whole-second `SECONDS`, which reports a second more when a tick lands inside one; the
  shell now reads `SECONDS` back after each window, exits 9 when it moved, and the caller reruns the case
  up to twenty times, failing on the crossed tick rather than on the elapsed value. The CI failure on the
  way in had its own root cause: the daemon SIGKILLs a job child after thirty ticks of its running tick,
  so the suite's 25 ms tick capped a delivery child at 750 ms on a loaded runner; the suite tick is now
  100 ms.

- [x] 165. posture's `-fq` osqueryd liveness read, held out of sweep slice 1 because `-f` matches the
  whole command line rather than the executable, filed 2026-09-20 from
  [docs/research/2026-09-20-native-call-sweep.md](https://github.com/webdavis/dotfiles/blob/main/docs/research/2026-09-20-native-call-sweep.md)'s
  proposed slice 6. Pin which predicate the control actually means first, then replace it with
  `proc_pidpath` or `KERN_PROCARGS2`.

  DONE 2026-09-20: slice 6 of the native-call sweep landed, the last pgrep spawn in posture. The
  watchdog's osqueryd liveness read ran `pgrep -fq '/opt/osquery/.*osqueryd'` on every tick; `-f` matches
  the whole command line, which on the live machine also matched an unrelated shell whose arguments
  merely quoted that path. The control means the vendor daemon itself, so the predicate is the executable
  path, and the in-process walk slice 1 landed already read it through proc_pidpath. ProcessLookup gained
  a directory filter, the watchdog reader gained a process table, and a match now means running while
  both no match and an unreadable table mean not running, the same answer the failed spawn gave. Pinned
  by tests over spawned fixtures: one selected by the directory it runs from, one in another directory
  rejected, and the reader asking for exactly that name and directory across all four answer shapes. The
  walk keeps posture's own deadline, and a grep for pgrep over posture's Rust sources now finds only
  control key names and doc comments. [PR #855](https://github.com/webdavis/dotfiles/pull/855), merged
  `ec5e09b83`.

- [x] 166. Evaluate `codesign` against Security.framework, filed 2026-09-20 from the native call sweep's
  proposed slice 7. A measurement and a record-shape decision before any code: the framework calls are
  verified to exist, but the enrichment stores `codesign`'s text today and a signing-information
  dictionary is a different record shape.

  DONE 2026-09-20: decided to build it as a slice, since the native call runs 9x to 24x faster and the
  record shape stays unchanged.
  [docs/research/2026-09-20-codesign-versus-security-framework.md](https://github.com/webdavis/dotfiles/blob/main/docs/research/2026-09-20-codesign-versus-security-framework.md)
  measured `codesign -dv --verbose=2` against SecStaticCodeCreateWithPath plus
  SecCodeCopySigningInformation on dresden over six paths (an Apple system binary, a Homebrew ad-hoc
  binary, a scratch ad-hoc binary, the same file with its signature removed, an Apple bundle and a
  Developer ID bundle), with a throwaway Rust program linking Security.framework through raw FFI because
  neither security-framework 3.7.0 nor security-framework-sys 2.17.0 binds the signing-information call.
  Parity holds on all three fields the classifier reads, since the absent kSecCodeInfoIdentifier key is
  the header-documented unsigned signal, the 0x2 flag is the ad-hoc signal and the first certificate's
  subject summary is the Authority line; only the rare "signed, no authority" branch could not be
  reproduced and is marked UNVERIFIED. The native call costs 0.83 to 4.50 ms at the median against 14.4
  to 40.8 ms for the spawn, and the p95 gap is wider than the median gap on every row. The record shape
  decision is to keep Enrichment unchanged and move the inspection seam to return three facts instead of
  codesign's text, so no reader and no page line changes, and the deadline wrapper is needed for file
  input and output alone, since the copy call validates nothing and network access is an opt-in flag on
  SecStaticCodeCheckValidity. Pinning five behaviours and carrying posture's own bounded-call wrapper is
  the plan. [PR #856](https://github.com/webdavis/dotfiles/pull/856), merged `c9ba0929c`.

- [x] 167. Find out whether the guest account and FileVault have public answers, filed 2026-09-20 from
  the native call sweep's proposed slice 8. A research slice, not a build: both are keeps today on the
  strength of a header search finding nothing, which is the weakest evidence in that document.

  DONE 2026-09-20: the guest account is a replace through the login window property list, and FileVault
  is a final keep because every interface that reports its state is private.
  [docs/research/2026-09-20-guest-account-and-filevault-public-answers.md](https://github.com/webdavis/dotfiles/blob/main/docs/research/2026-09-20-guest-account-and-filevault-public-answers.md)
  settles the two rows that were decided on a header search finding nothing.
  `/Library/Preferences/com.apple.loginwindow` carries a GuestEnabled boolean that agrees with
  sysadminctl on the live machine and sits in a property list posture already parses in process for the
  automatic-login control, at 0.12 ms against a 26.32 ms spawn. FileVault.framework under
  PrivateFrameworks and libcsfde exporting symbols with no header in the SDK are both private, and the
  two public encryption properties were measured disagreeing with fdesetup in both directions on one
  machine, for the reason Apple's own security documentation gives: the data volume is encrypted whether
  FileVault is on or off, and FileVault changes only how the key is protected.
  [PR #857](https://github.com/webdavis/dotfiles/pull/857), merged `c8474c757`.

- [x] 168. `uu/crates/uu-adapters/src/lanes/herdr.rs` (around line 151 on main) still carries a comment
  claiming that installing over an existing herdr plugin duplicates it, filed 2026-09-20. Task 155's
  [PR #839](https://github.com/webdavis/dotfiles/pull/839), merged `dc5d58d34`, disproved that when
  `run_after_53` started reinstalling a drifted pin in place. The comment must say what the code does
  now.

  DONE 2026-09-20: [PR #854](https://github.com/webdavis/dotfiles/pull/854), merged `bee58e61e`. The
  comment in uu's herdr lane now says what a failed uninstall does: the installed copy is left as it was
  and the report says so.

- [ ] 169. A brew upgrade of `gh` blocks every GitHub call until the operator answers LuLu, filed
  2026-09-20. uu's Sunday run upgraded gh to 2.101.0 at 12:31 (`/opt/homebrew/Cellar/gh/2.101.0/bin/gh`,
  INSTALL_RECEIPT time); from that minute `gh api` and every gh-axi call failed with
  `dial tcp 140.82.112.5:443: connect: bad file descriptor` or `i/o timeout` while curl, node and python
  reached api.github.com in under a second, and `git ls-remote` over SSH answered. LuLu keys its rule to
  the binary path, and the Cellar path carries the version, so every upgrade is a new binary waiting for
  an Allow. Two ship agents (slice 51, ledger batch three) stalled on it. Fix is in LuLu's own settings,
  not this repository: re-key the gh rule to its code-signing identity (LuLu offers that when the alert
  is answered), and check the other Rust-and-Go CLIs uu upgrades weekly (`herdr`, `td`, `atuin`) for the
  same trap. Evidence: this session's transcript, 2026-09-20 12:31 to 13:00. The operator did not change
  anything on 2026-09-20 and gh recovered on its own at about 15:30, so the cause is still open; watch
  the next Sunday run.

- [ ] 170. Give `pns recap` one line for the standing dead-letter count, filed 2026-09-20 out of task
  147's ruling. The watchdog pages on growth only, so the standing population needs a home the operator
  reads every day: the recap's `open` section (the one never shed from a delivered page) prints
  `N legs dead-lettered, run pns failures` when N is nonzero and nothing when it is zero. Its own PR
  after slice 53 lands, because slice 53 replaces the recap engine and is already the largest slice; the
  line reads `SqliteStore::failing_legs` filtered to `deadlettered`, the same query `pns failures` lists,
  so the two can never disagree. Add the line to the recap design's Sections table in the same PR.

- [ ] 171. Retire `~/.local/libexec/pns/` entirely, operator ruling 2026-09-20: pns ships no bash, and
  nothing of pns lives under libexec. Its last member, `hooks/codex/install-hooks.sh` (the jq merge of
  pns's four Codex hooks into `~/.codex/hooks.json`, run by
  `.chezmoiscripts/run_after_72-relay-codex-hooks.sh.tmpl` on every apply), becomes a pns verb with the
  same merge semantics and an atomic write, the apply script calls the binary, the `.chezmoiignore` line
  goes, and the directory is deleted from source. Chezmoi never deletes a retired target and this
  repository builds no removal mechanism, so the operator trashes `~/.local/libexec/pns` after the apply
  that lands it. Pull request in flight.

- [ ] 172. A failure notice reading `bad URL` names a fault pns never had, filed 2026-09-22 from the
  night the test sandbox leaked banners to the desk. `TransportOutcome::NoStatus` is labelled `bad URL`
  (`pns/crates/pns-domain/src/failure/meaning.rs:57`, repeated by `pns failures` at
  `pns/crates/pns/src/command_failures.rs:291`) and, for every destination but the phone, worded "the URL
  pns built for {route} is malformed, nothing was sent" (`meaning.rs:98`). A stored failure only carries
  that outcome from ledger outcome 3, `Delivery::Unlaunched`
  (`pns/crates/pns-adapters/src/persistence/sqlite/ledger/failing.rs:58`): an executable channel that
  could not be launched, a channel request that could not be encoded, an unregistered destination, or no
  durable delivery attempted. None of those is a URL, and a genuinely malformed hermes URL is stored as
  `Failed` and reads `no response` instead (`pns-adapters/src/destinations/hermes.rs:184`). The operator
  is sent to check a URL that was never built. Relabel from what the outcome records, and word the
  meaning line for an unlaunched leg.

- [x] 102. A rejected delivery config silences posture entirely and only a log file says so. DONE
  2026-09-17. Filed the same day 2026-09-17 from the firewall drill's incidental finding.
  `~/.local/log/osquery/firewall-gatekeeper-monitor.log` holds this line from 2026-09-16 19:06:
  `posture: the delivery config could not be used, so no page can be delivered: unknown field notify,`
  `expected delivery`. That specific mismatch is RESOLVED and is not the task: the source struct at
  `posture/crates/posture-adapters/src/notify/schema.rs:18` declares `pub(super) notify: Table` and
  `dot_config/posture/private_config.toml.tmpl:38` ships `[notify]`, so the two agree today, and the
  operator received real pages during the 2026-09-17 drill. THE DEFECT IS THE FAILURE MODE. That struct
  carries `#[serde(deny_unknown_fields)]`, so one unknown or renamed top-level key makes posture refuse
  the WHOLE delivery configuration and deliver NOTHING, and the only symptom is a line in a log nobody
  reads. Nothing catches it: the watchdog proves each job RAN rather than that it could deliver, and no
  check anywhere reads the delivery config for parseability, verified 2026-09-17. Paired with task 100's
  false all-clear, a machine can sit in total page silence while every surface reports healthy, which for
  a security monitor is the worst available state. THERE IS A LIVE INSTANCE OF THE RISK.
  [PR #721](https://github.com/webdavis/dotfiles/pull/721) added a `[jobs]` table to that same struct. It
  is `#[serde(default)]`, so a config without it is fine, but a config WITH it against an older binary is
  rejected outright. An apply writes the config target and rebuilds posture in the same run, so they
  normally move together; if the rebuild fails after the config has landed, posture stops delivering
  every page until the next successful apply. Two things are wanted. First, make the failure loud: a
  delivery config that will not parse should reach the local banner and `posture doctor` rather than only
  a log, because a page that cannot be delivered is precisely what the operator must hear about. Second,
  prefer degrading to refusing where it is safe: an unknown key inside a known table can warn and
  continue, while a malformed known key still refuses. Pin both with tests, including one that an unknown
  key never silently disables delivery. CONFIRMED LIVE 2026-09-17, during the apply that shipped task 97.
  The monitor log gained
  `posture: the notify config could not be used, so no page can be delivered: unknown field jobs,`
  `expected notify` inside the apply window (10:17:26Z to 10:18:49Z). The cause is the ordering this
  entry predicted: an apply writes the config target before the `run_onchange_after_5*` builder
  reinstalls the binary, so a scheduled poll tick that lands in between runs the OLD binary against the
  NEW config and delivers nothing. It closed itself when the rebuild finished: ticks after 10:18 produced
  no further error lines, `runs` reached 2572 at exit code 0, and the watchdog state is keyed by job
  name, which only the new binary writes. So the exposure was about one minute of total page silence,
  self-healing, and unreported anywhere but this log. That is the whole argument for the fix: the window
  is short here only because the rebuild succeeded. SHIPPED 2026-09-17 as
  [PR #733](https://github.com/webdavis/dotfiles/pull/733), merged `9767bda3`, BOTH HALVES. LOUD:
  `Notify::report` is the one place a delivery-config outcome is reported and `alert_sink` calls it, so
  all six jobs are covered by one guard rather than six. A refusal still writes its diagnostics line and
  now also raises the LOCAL BANNER through the `IndependentAlarm` the caller already hands in, which is
  the one destination a broken delivery config cannot take away. A new `posture doctor` answers the same
  question on demand: it names the config path, lists every ignored key, and either prints
  `FAILED: no page can be delivered: <reason>` and exits 1 or says the config is usable and exits 0.
  There was no doctor subcommand before. DEGRADE: `#[serde(deny_unknown_fields)]` is gone from all five
  structs in `notify/schema.rs`, replaced by a flattened unread map on each, so serde itself hands back
  the keys this build has no field for, at the top level and inside every known table, and each becomes
  one named warning line. A malformed KNOWN key still refuses the whole file, and so does a missing
  `[notify]` table, which is what keeps a mistyped top-level table name from paging nowhere. THE
  TOP-LEVEL DECISION, made deliberately: an unknown TOP-LEVEL key now warns and continues, because that
  is exactly what the live instance needed. A config written for a newer or older build of one tool is
  the ordinary case on a machine where an apply writes the config before the builder reinstalls the
  binary, and refusing it takes away the very pages that would report the trouble. A mistyped table
  BESIDE a valid `[notify]` costs that table's contents plus a named warning, which is the smaller loss.
  THE MERGE WITH TASK 99 WAS A REAL CONFLICT AND WAS RESOLVED RATHER THAN GUESSED: both tasks hardened
  the same four files. `parse` now returns the whole `Notify` instead of a tuple of mode, route and
  warnings, and `missing_hermes_keys` moved from `NotifyMode` to `Notify`, because `NotifyMode` has no
  route and cannot know the configured one. One test was added for the same reason, since nothing pinned
  that the signing-key check follows the CONFIGURED route rather than the shipped default. ALL FIVE
  BEHAVIOURS WERE PROVEN AGAINST THE REAL BINARY with fixture home directories: an unknown nested key
  warns and delivery continues; an unknown top-level key does the same, which is the live failure this
  task exists for; a malformed known key refuses and REDACTS the value in its message; a refusal reaches
  both doctor and the banner, pinned by a test and its negative; and an untiered page takes the
  configured route while a critical page still takes `priority`, proven by a fixture that keys only the
  other route and fails naming `priority`. 1200 posture tests pass. OPERATOR STEP: a full `chezmoi apply`
  rebuilds posture, after which `posture doctor` exists.

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

- [ ] PARTLY BUILT, checked against the machine on 2026-09-15, because this bullet read as not started
  and that is wrong. What exists: the Rust plugin is committed at
  `dot_local/share/herdr/plugins/herdr-process/`, built by
  `.chezmoiscripts/run_onchange_after_58-build-herdr-process-plugin.sh.tmpl`, deployed to
  `~/.local/share/herdr/plugins/herdr-process/` with a compiled `target/`, and its generated
  `herdr-plugin.toml` declares four actions for the `tuicr` profile (`split-right`, `split-below`,
  `toggle-float`, `kill`) plus an `attach` pane. Its own test suite passes: 42 adapter cases, 14
  application cases, 19 binary cases and 16 composition cases. WHAT IS LEFT: only one action is bound to
  a key, `herdr-process.tuicr:toggle-float` at `dot_config/herdr/config.toml:331`, so split-right,
  split-below and kill are reachable through the plugin but not from the keyboard; and `tuicr` is the
  only configured profile, so the reviewr, btop and scratch-shell examples in the requirement below do
  not exist yet. Runtime acceptance (attachment, redraw, resizing, focus, the configurable Ctrl+C
  behavior, explicit termination, process-exit cleanup) has not been run on the operator's own session.
  Original requirement, still the specification for what is missing: build a persistent process-toggle
  plugin for Herdr in Rust, following `$clean-code-rust` and its prerequisite `$clean-code`. Consult
  `$frontend-design:frontend-design` for terminal interface design and review. Behavior agreed
  2026-09-12; the 2026-09-13 goal authorizes implementation in the resume order. Support any number of
  named window configurations and running sessions, with no hardcoded cap. Each configuration supplies a
  program, arguments, working directory, floating dimensions, and independent shortcuts for Split right,
  Split below, and Toggle float. tuicr, reviewr, btop, and a scratch shell are example configurations;
  the plugin stays independent of the program. Split right opens side by side with a vertical divider;
  Split below stacks panes with a horizontal divider. Either split action starts, docks, or repositions
  the same session, or focuses it when already placed correctly. Toggle float starts a floating session,
  pops out an existing split, or hides/restores an existing float. Pop out/dock preserves the running
  program, terminal screen, position, comments, and unfinished input. Popping out releases the old
  split's space; hiding a float keeps it hidden until restored or explicitly docked. The plugin owns
  background session lifetime and forwards input and resize events through the attached view. Shortcuts
  work from Neovim and shell panes and while a float is focused. Show one floating window at a time:
  selecting another hides the previous float without stopping either session. Docked sessions remain
  visible. Stacking floating windows is outside the agreed scope. Center floats over the whole Herdr
  window, spanning underlying panes, with configurable percentage dimensions that resize and recenter
  when the terminal window changes size. A hidden session receives the current dimensions when restored.
  Keep toggling and docking fast. Per window, let users configure whether Ctrl+C in the popup terminates
  its process or only hides the popup and keeps the background session alive. The hide action must not
  forward Ctrl+C to the running program. Also expose a separately configurable Herdr kill binding for
  each named session, closing its view and terminating its owned processes whether floating, docked, or
  hidden. Hiding preserves the session; quitting or killing ends it. Process exit closes its view and
  clears its session, including exits while hidden. The next launch starts a fresh instance. Verify
  attachment, redraw, resizing, focus, configurable Ctrl+C behavior, explicit termination, and
  process-exit cleanup through supported Herdr interfaces before building the review launcher below. The
  2026-09-13 feasibility audit found a supported implementation path in Herdr 0.9.0: percentage popups
  center and resize over the shared pane surface, spanning its panes while excluding sidebar and tab-bar
  chrome. The owned Rust attachment must handle configured shortcuts while focused because popup input
  bypasses native Herdr binding dispatch. Read the same configured prefix and plugin actions, preserve
  unmatched input and paste, and reject ambiguous encodings. Keep the process in an owned pseudoterminal
  and replace its views. Hide by ending the owned attachment, never by blindly closing whichever popup is
  active. Prove view identity, redraw, transition rollback and process cleanup with fixtures before
  runtime acceptance. These are implementation requirements; no mandatory upstream change was found.
  Operator note 2026-09-14: herdr's documented `[[keys.command]]` popups are NOT this (a popup lives only
  until its command exits; no toggle, hide or float exists in the docs, the keybinding actions or the
  CLI, which offers zoom, split, move, swap and close). When this is built, dig into herdr's source for a
  true hide before settling for parking the pane in another tab, and check herdr's preview channel (its
  nightly, more or less) for a hide or float primitive that the stable release lacks.
- [x] DONE 2026-09-16 in [PR #706](https://github.com/webdavis/dotfiles/pull/706), merged `744489fa`:
  `~/.local/bin/worktree-review.sh` (bash, 352 lines, 19 bashunit tests, 21 mutants caught) plus a
  `review` profile and one chord handing it to the live herdr-process plugin for the three placement
  actions. `reviewr` is NOT installed on this machine, so the launcher takes the review command as an
  argument defaulting to `tuicr`, and nothing names reviewr. Measured against a synthetic 301-worktree
  repository: first load 4.9 s median cold (budget proposed 8 s), repeat load 0.05 s, review start under
  0.1 s. Recency is max(HEAD committer time, newest non-ignored mtime), `graphify-out/graph.json`
  excluded as generated. OPERATOR STEPS LEFT: confirm the three latency budgets the lane proposed, and
  run one real pick and resume in your own herdr session. Original entry: add a deterministic worktree
  picker and reviewr launcher. Consult `$frontend-design:frontend-design` for the picker's interface
  design and review. Implementation is authorized by the 2026-09-13 goal after the process-toggle
  feasibility checks pass. From the current repository, list existing worktrees with the most recently
  updated first, including commits and uncommitted file edits while excluding ignored files. Search and
  select a worktree without changing the agent's working directory or branch. Agents may remain on main
  while orchestrating multiple worktrees; selection needs no language-model call or agent-to-worktree
  registry. Keep worktree selection separate from the generic process-toggle plugin; the picker can be an
  ordinary command rather than another required plugin package. Use the shared session behavior above for
  Split right, Split below, and Toggle float. With no selected target, open the picker first; subsequent
  toggles resume that review without repeating discovery. Preserve the originating workspace context so
  reviewr can send comments to the intended agent, including its agent picker when several agents are
  present. Opening the picker and resuming a review must feel immediate with hundreds of worktrees.
  Measure first-load, repeat-load, and review-start latency separately and agree a budget before
  implementation. Evaluate cached activity ordering refreshed separately from display, keeping the
  selection stable during refresh; resolve deletion timestamps, stale paths, and cache freshness before
  claiming accurate recency. Reuse the process-toggle session handling after its feasibility checks pass;
  do not patch reviewr or duplicate that handling here. The requested upstream proposal already exists as
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
- [x] SP4, improve the interactive shell. Shipped 2026-09-21, after the xonsh no-go unblocked it. Alias
  consolidation done: 22 of the 27 aliases `~/.bashrc` declared inline moved into `~/.bash_aliases`, and
  the five that stayed each need a template conditional the plain file cannot carry (`claude`, `ping`,
  `bu` and `rm` behind the darwin gate, plus `j`, which stays paired with the carapace completion wrapper
  that exists only because it does). The alias set is byte-identical across the move, 41 lines either
  way. One table now drives both outputs: `chord render menu` emits one tab-separated record per binding
  row, and `__bash_bindings_list_bash_bindings` reads those records instead of regex-scraping the
  rendered `bind` calls, so all 176 rows are reachable with their group and description where the scraper
  matched 139 lines and could never match a readline command at all. The picker also runs the selection,
  which it previously could not: a `run` or `function` row executes, an `insert` row seeds an editable
  line, and a `command` or `macro` row is refused because it edits the line rather than running a
  command. `ctrl-x v` and the function name are unchanged. All five selection candidates were declined
  with evidence rather than adopted, the Charm verdict was reconciled (the G2 look was adopted, gum
  itself stays declined), and the per-binding declaration tests were dropped under the behavior-only
  policy rather than satisfied; the reasoning for each is in
  `docs/decisions/2026-09-21-sp4-interactive-shell-dispositions.md`. The Nushell no-go stands and xonsh
  remains an on-demand subshell.
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
- [x] Finish OpenSpec configuration, explicitly requested 2026-09-12. Its npm package is declared and
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
  and resuming. [PR #586](https://github.com/webdavis/dotfiles/pull/586) merged 2026-09-14 (`cdaa33fb`):
  `dot_config/openspec/config.json` deploys `~/.config/openspec/config.json` with `profile: core`,
  `delivery: both`, `telemetry.enabled: false` and `completionTipSeen: true`, and
  `docs/runbooks/agent-tooling.md` documents the tracked config, the per-project `openspec init` flow,
  and the separate upgrade-versus-refresh jobs. Deliberately out of scope, per the task's own "project
  specifications in their owning repositories" line: no `openspec/` root, no `openspec init` and no
  generated harness integrations in this checkout, since it gets no OpenSpec project of its own.
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
  L6's one real cross-project dependency is the optional vpt handoff, which stays in the item below and
  keeps canonical originals in the vault layout that already exists on disk
  (`agent-processing-pipeline/raw/`, `transcripts/` and `analysis/`). The boundary was written back as
  Todoist comments on the four homelab tasks and the dotfiles credential task, comment 6hW4Mc2w4Mf9PcrV
  on [A6](https://app.todoist.com/app/task/6hVpX4M8hmV4Gmr3), 6hW4MfWF2XGjmH63 on
  [F4](https://app.todoist.com/app/task/6hVpfWghCjQ66GG3), 6hW4MgVfXrQgpcXV on
  [F5](https://app.todoist.com/app/task/6hVpfWmPgm7qQHMV), 6hW4MhgQ5r44GVqV on
  [L6](https://app.todoist.com/app/task/6hVpfWrX8W4jrQcV) and 6hW4MmF39R6mpj5M on the laptop task. No
  homelab backlog item was imported, no task was completed because all four remain planning-stage behind
  F1 and F2, and no file in this repository was changed.
- [ ] Coordinate vpt's optional Open Notebook handoff with L6. Preserve one capture/transcription
  pipeline and canonical originals; decide the handoff format during integration design. vpt and Bob must
  not require Open Notebook merely to read or produce ordinary notes. Designed 2026-09-14 as
  `docs/superpowers/specs/2026-09-14-vpt-open-notebook-handoff-design.md`, the seventh document in the
  vpt chain, built on the boundaries design's four-homes table. The finding that decides it is in Open
  Notebook's own user guide, read today rather than remembered: "Audio/video is transcribed to text
  automatically", over MP3, WAV, M4A, OGG and FLAC, where M4A is what Apple Voice Memos writes. So
  handing it a recording starts a second transcription on a second engine with no flags, no alternatives
  and no `known-terms.txt`, which is exactly what L6's own bullet forbids and is one drag onto a web
  page. The recommendation makes the refusal structural instead of advisory: one pure command,
  `vpt handoff <id> [--stage transcript|analysis|brief|draft]`, prints one `vpt.handoff/1` document on
  standard output and does nothing else, and that document has no field that can hold audio or a path to
  it, so the supported path cannot produce the forbidden outcome. vpt performs no request, holds no
  credential and knows no address, which preserves the redacted-draft design's rule that vpt never
  transmits; the Open Notebook vocabulary lives in a mapping table in the document (`content`, `title`
  from vpt, `type: "text"`, `notebook_id` and `embed` from the pusher, everything else carried inside
  `content` because the source model has no metadata field) and in a four-line `jq` plus `curl` recipe
  owned by whoever runs it. That recipe was verified end to end against a throwaway local listener with
  an invented password: `curl -H @file` on the installed 8.22.0 delivered `Authorization: Bearer` with
  the password in no process argument list, the body arrived with `type: "text"`, and `shellcheck` passes
  clean; nothing left the machine, and no instance exists to send to since L6 is queued behind F1 and F2.
  Canonical originals hold because the emitted document is a pure function of the record and the note, so
  a lost notebook is regenerated by re-running one command, and because vpt has no importer and never
  reads anything back. The done-means is written as four absence checks (service absent, configuration
  absent, a grep over vpt's tree for `open.notebook`, `notebook_id`, `5055`, `8502` and `surreal` finding
  nothing, and `vpt.brief/1` unchanged for Bob), the third of which is the cheapest guard against a
  convenience push creeping in later. The header inside `content` carries the identity, the
  unresolved-flag count and a derived-copy sentence, keeps `[unverified]` markers in place because a
  model summarizing a source drops a footer and keeps the sentence, drops the note's frontmatter
  (`vppRecording` is a to-the-second capture timestamp wearing an identifier's clothes), and is
  deliberately declarative, since a source that instructs a model is indistinguishable from an injected
  one in a notebook that also holds web pages. Two upstream configuration findings are handed to L6
  rather than solved here, both in upstream's own words: one shared password sent in plain text with no
  rate limiting or audit log, so encrypted transport is mandatory, and an unrestricted `CORS_ORIGINS`
  means "any website the user visits can issue authenticated cross-origin requests to your API". Named
  ceiling: vpt never learns the remote source identifier, so a corrected note handed off twice makes two
  sources rather than replacing one; the upgrade path is written in the source instead of built. Waiting
  on the operator: whether a private note may cross or only a released redacted draft, whether the
  no-push rule survives costing three lines at every handoff, which artifacts may be handed off at all
  (the brief names the people who will be in a room), who holds the shared password when L6 exists and
  whether an agent gets write access through the `uvx open-notebook-mcp` server, who owns the return path
  for a note authored in Open Notebook, and where the transport recipe eventually lives. No code written.
  Full document: `docs/superpowers/specs/2026-09-14-vpt-open-notebook-handoff-design.md`. Operator steps:
  (1) Answer the private-versus-redacted question, because it decides what the feature is for: the
  proposal lets a private note cross with the document recording that it did, and the alternative is that
  only a released redacted draft may cross, which makes the notebook safe and much less useful. (2)
  Confirm the no-push rule: three lines of jq and curl at every handoff, forever, versus one
  `vpt handoff --push`. If three lines are too heavy for how this will actually be used, say so before
  the rule is written into tests rather than after a convenience flag is added around it. (3) Decide who
  holds Open Notebook's shared password when L6 exists: the laptop through KeePassXC, an agent through
  `uvx open-notebook-mcp` (uvx is already installed), or nobody, with the handoff done by hand in the web
  interface. This is the smallest concrete instance of the A6 credential question already in the ledger.
  (4) Hand L6 the two upstream configuration findings, which are not vpt's to fix: encrypted transport is
  mandatory because the shared password is sent in plain text with no rate limiting or audit log, and
  `CORS_ORIGINS` must be restricted or any site the operator's browser visits can reach the interface
  with the operator's session. (5) Decide who owns the return path for a note authored in Open Notebook
  that should become durable. L6's bullet says canonical locations including Obsidian; this design says
  vpt has no importer, which leaves the act unassigned until it is assigned deliberately. (6) This ledger
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
  (6) Does the handoff need to say it came from vpt? The proposed header names vpt and the record
  identity, which is good provenance and also tells anyone with notebook access that a recording exists.
  (7) What happens to a copy in the notebook when the note it came from is corrected? Today nothing: a
  second handoff makes a second source, because vpt never learns the remote identifier. The upgrade path
  is named; whether it is needed depends on how often a transcript is corrected after filing. (8) Where
  does the transport recipe eventually live? Nowhere today. A dotfiles `libexec` script, a `just` recipe,
  a homelab-side ingester and "the operator types it" are four answers with four different maintenance
  costs, and a scheduled one would need a separate argument against "no second automatic workflow". (9)
  Where does vpt's code live, and what is it called? Carried forward unresolved from the boundaries
  design, because the chain should not stay in disagreement with itself.

### vpt (Voice Processing Tool)

Planning addition, 2026-09-12. Build vpt in Rust to collect everyday Apple Voice Memos synced to the Mac,
preserve their original audio format, transcribe them, and produce agent notes and summaries. Track the
Mac workflow in [6hVpPJC2cjJW3V9M](https://app.todoist.com/app/task/6hVpPJC2cjJW3V9M). No ingestion or
transcription was started during this audit.

- [ ] Reconcile the existing sources before designing a second transcription system:
  `~/workspaces/Ivy/webdavis/homelab/docs/plans/PLAN-v12-experiments-backlog.md`, L-R5, records the vpt
  feature decisions and carries forward the earlier local transcription experiment. The vault's
  `CLAUDE.md` already defines `agent-processing-pipeline/raw/audio/`, `transcripts/` and `analysis/`,
  with audio excluded from Git. Homelab `PLAN-v11.md`, Phase 6, and
  [6gjGcHp69phXmXj3](https://app.todoist.com/app/task/6gjGcHp69phXmXj3) describe the broader ElevenLabs
  Scribe/whisply pipeline, Markdown transcripts, subtitle and word-timing exports, and local processing
  for sensitive audio or service outages. These are related plans; none explicitly specifies a watcher
  for Apple Voice Memos synced to macOS. Reconciled 2026-09-14 in
  `docs/research/2026-09-vpt-source-reconciliation.md`. The three named sources reconcile and the claim
  holds: L-R5 is the vpt specification itself (its `pns submit --json` contract reverified against
  `pns-protocol` source, `producer` and `signal.kind: needs_attention` both correct, though `submit` is
  missing from `pns --help`); the vault's `agent-processing-pipeline/` tree is a filing convention that
  has never run (all three directories empty since 2026-07-22); and `PLAN-v11` Phase 6 is a homelab
  hermes skill that transcribes URLs on `lash` through n8n, never a local-recording watcher, contributing
  only its engine decision (ElevenLabs Scribe v2 with whisply fallback) and its three output formats.
  None specifies a Voice Memos watcher. The reconciliation also found a FOURTH source the task does not
  name: the `minutes` 0.26.1 cask, declared and installed 2026-09-08, whose 60 subcommands already cover
  six of the seven vpt feature bullets (folder watcher plus launchd service, first-class `memo` type,
  `transcribe --json --diarize`, vault sync by symlink, voiceprints, retention policy, templates,
  insights, commitments, draft-only delivery). Only three vpt items are absent from all four sources:
  Voice Memos discovery, redundant transcription with disagreement comparison, and the pns
  needs-attention notification. **Verdict: defer the vpt ingestion design.** vpt's scope is blocked on
  the still-open `minutes` disposition (task `6hPV483GJgGHX95M`), which decides between an adapter around
  `minutes` and a full replacement; `PLAN-v12` L6 already forbids a second automatic Voice Memos workflow
  for Open Notebook and nobody has applied that rule here. **Decided 2026-09-15: the blocking question is
  answered.** vpt does not use or depend on `minutes`, in any form: no calling `minutes transcribe`, no
  `minutes watch` front end, no runtime spawn. The operator's own words: `minutes` is poorly designed,
  though it has good features worth learning from. Two of its own dependent questions are therefore MOOT
  rather than answered, because the premise each depended on no longer holds: whether vpt calls `minutes`
  or runs beside it, and whether the `PLAN-v12` L6 rule binds `minutes` against vpt. The vault `minutes`
  symlink repair and the false vault `CLAUDE.md` claim remain their own small item, unchanged, since they
  are about `minutes` rather than about vpt. Full record:
  `docs/decisions/2026-09-15-vpt-architecture-decisions.md`, decision 1. Measured inputs for the next
  task, not decisions: recordings are ALAC 48 kHz stereo in `.m4a` (not AAC), about 1 GB for 28 files
  with single files at 188 MB; titles, durations and stable identifiers live only in
  `CloudRecordings.db`, a Core Data store whose write-ahead log must be copied with it or a reader sees a
  nine-day-stale snapshot; `ZEVICTIONDATE` does not mean the audio is gone (both evicted rows still have
  local files); and `minutes storage` already classes 30-day-old originals as delete-candidates, which
  collides with vpt's preserve-originals rule. Separate live drift: the vault `CLAUDE.md` claims the
  `agent-processing-pipeline/minutes` symlink is managed by `minutes vault setup --subdir`, but no
  `minutes` config file exists and `minutes vault status` reports `Vault: not configured`, so the
  committed link is an orphan. Full document: `docs/research/2026-09-vpt-source-reconciliation.md`.
  Operator steps: (1) Read docs/research/2026-09-vpt-source-reconciliation.md. It is already
  mdformat-clean against the repo .mdformat.toml, so no reformat is needed. (2) Rule on the `minutes`
  disposition: keep it, or replace it. This is the blocking decision. Everything about vpt's scope
  follows from it, and it is still open in Todoist task 6hPV483GJgGHX95M. Do not let the next vpt ledger
  task (the ingestion design) start before this ruling lands. (3) Rule on whether PLAN-v12 L6's existing
  constraint ("must not create a second automatic Voice Memos capture/transcription workflow", written
  for Open Notebook) binds `minutes` against vpt. If it is a general rule rather than a per-service one,
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
  whatever version is then installed. (7) Optional pns follow-up, independent of vpt: `pns submit` works
  (pns/crates/pns/src/invocation.rs:119) but is absent from `pns --help`. Whoever implements a producer
  against it will not find it from the CLI. Open questions: (1) Is `minutes` kept or replaced? This is
  the blocking question; vpt's whole scope follows from it, and the ledger has carried it as an open
  evaluation since before vpt existed. **Decided 2026-09-15: replaced.** vpt is a standalone tool with no
  dependency on `minutes` at all; see decision 1 in
  `docs/decisions/2026-09-15-vpt-architecture-decisions.md`. (2) If `minutes` is kept, does vpt call it
  or run beside it? Calling `minutes transcribe --json` makes it one of vpt's two engines and reuses its
  summarization, vault sync and speaker work. Running beside it means two tools writing notes about
  recordings, which is what PLAN-v12 L6 forbids for Open Notebook. **Moot, 2026-09-15:** `minutes` is not
  kept, so neither branch applies. (3) Does the PLAN-v12 L6 rule ("must not create a second automatic
  Voice Memos capture/transcription workflow") bind `minutes` against vpt, or was it only ever about Open
  Notebook? **Moot, 2026-09-15:** vpt does not touch `minutes` for L6 to bind against. (4) Which two
  engines are the redundant pair? Phase 6 already chose ElevenLabs Scribe v2 with whisply on
  faster-whisper as the fallback, and Scribe, whisply, openai-whisper and the minutes on-device pipeline
  are all present here. Cloud-plus-local and two-local differ in cost, in what audio leaves the machine,
  and in whether a disagreement means anything. Still open: the pairing itself stays a configuration
  value rather than one fixed pair (decision 3), with Apple Speech and whisply as the two starting
  first-class adapters (decision 10). (5) Do Voice Memos originals leave the machine at all? L-R5
  inherits Phase 6's "local processing for sensitive audio", but everyday personal voice memos may all
  qualify, which would remove the metered cost line and one candidate engine together. **Decided
  2026-09-15:** this is the user's choice, not a product rule; local is the default, `vpt setup`
  preselects it, and the operator's own configuration stays local. See decision 2. (6) Repair the vault
  `minutes` symlink now or fold it into the vpt work? Folding it in leaves the vault CLAUDE.md claim
  false until vpt ships. **Decided 2026-09-15:** neither folded nor changed by this decision; the symlink
  repair and the false CLAUDE.md claim remain their own small item, unchanged, since they are about
  `minutes` rather than about vpt. (7) Should the four unwired transcription installs stay declared?
  whisply, openai-whisper, @elevenlabs/cli and the minutes cask are all in
  .chezmoidata/system_packages_autoinstall.yaml and no script, recipe or LaunchAgent reaches any of them.
  They are either vpt's future inputs or removable weight, and which depends on the engine decision.
  **Decided 2026-09-15:** whisply, openai-whisper and @elevenlabs/cli stay declared as candidates for
  vpt's engine adapters; the minutes cask has no remaining use under decision 1. The data file is
  unchanged by this pull request; dropping a declaration is an edit the operator makes deliberately, per
  this repository's no-removal-mechanisms rule. See decision 12. (8) Does `minutes` get upgraded to
  0.26.2 before the disposition decision, given that its feature set is the input to that decision? (9)
  Deferred to the next ledger task, not answered here: whether a launchd-run service can read
  ~/Library/Group Containers/group.com.apple.VoiceMemos.shared at all under its own macOS
  privacy-permission identity (every read in this record ran from a terminal that already holds broad
  disk access), and whether reading Apple's private CloudRecordings.db Core Data schema is acceptable at
  all. A no to either changes what vpt is, from a watcher to an export-path integration.
- [ ] Design automatic discovery of fully synced recordings, preserving original audio and capture
  metadata without modifying Apple's source recordings. Verify the supported macOS access/export path and
  actual audio format before choosing an ingestion mechanism. Handle interrupted sync, retries and
  repeated discovery without duplicate notes or lost audio. Keep original recordings, transcripts and
  agent analysis separately linked using the existing vault layout. Designed 2026-09-14 in
  `docs/superpowers/specs/2026-09-14-vpt-recording-discovery-design.md`, written against the second
  bullet only and scoped so the still-open `minutes` disposition changes the consumer rather than this
  producer. The access-path verification is the finding: Voice Memos ships no scripting dictionary, its
  App Intents expose only title, creation date and duration with no action that outputs a file, and
  Spotlight holds no content metadata, so no supported programmatic path yields the audio and the real
  choice is a read-only read of the undocumented group container or a human export through the share
  sheet. The format is Apple Lossless Audio Codec at 48000 Hz in an `.m4a` container, 988 MB across 28
  recordings. The capture timestamp was measured inside the audio file, matching the database to the
  second, so the private schema is needed only for the human title and its loss degrades to untitled
  rather than broken. Recommended: an idempotent `vpt ingest` sweep on a `StartCalendarInterval` rather
  than a watcher or a daemon; a wholeness gate of MPEG-4 top-level box lengths summing to file size with
  `moov` present, plus an mtime quiet period; content-derived identity; and `clonefile(2)` into
  `agent-processing-pipeline/raw/audio/`, whose `EEXIST` is the duplicate guard and whose copy-on-write
  clone preserved a 187.9 MB recording in 0.00 s for 16 KB. Not approved and not built: it carries twelve
  assumptions and eight open questions, and one measurement is unresolved, whether a LaunchAgent that
  launchd starts at login can read the group container, since every read in the session inherited
  Ghostty's Full Disk Access grant. Full document:
  `docs/superpowers/specs/2026-09-14-vpt-recording-discovery-design.md`. Operator steps: (1) Read the
  design and answer open question 1 first: is reading Apple's undocumented Voice Memos group container
  acceptable at all? The measurements closed every supported programmatic path, so a "no" turns vpt from
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
  the audio, so a "no" makes vpt a manual filing tool driven by the share sheet. (2) Can a LaunchAgent
  that launchd starts at login read the group container? Unresolved. Every read in this session inherited
  Ghostty's Full Disk Access grant, a launchctl-submitted job may inherit the same attribution, and
  launchctl procinfo needs root. (3) Does the minutes ruling change this boundary? The design assumes
  discovery is a producer and transcription a consumer. If vpt is meant to be a thin front end on
  `minutes watch`, the clone destination changes and most of the design is replaced by configuring a
  third-party tool. (4) Clones, or references in place? The design chose clonefile(2) on measured cost,
  which puts a second copy of every personal recording inside the vault directory, gitignored but
  present. (5) Fifteen minutes as the sweep interval, or something else? And is near-immediate discovery
  worth taking WatchPaths despite launchd.plist(5) discouraging it in its own words? (6) What happens to
  a recording deleted in Voice Memos after vpt has cloned it? The clone survives, which is the point of
  cloning. Should vpt notice the disappearance and mark the sidecar, or keep the clone silently? (7) Do
  the eight .waveform sidecars matter? They belong to 2022-era recordings only and are Apple's rendering
  cache; the design ignores them. (8) Should `vpt ingest` emit its per-recording record on stdout as JSON
  for a caller to pipe, or only write sidecars? The design does both on the assumption the next stage
  wants a stream; if nothing will consume it, that is unneeded surface.
- [ ] Use redundant transcription and compare disagreements; flag uncertain text and unsupported notes
  for review, notifying through pns's producer application programming interface (API). Preserve the
  alternatives and source references. Multiple engines agreeing does not prove correctness. The proposed
  feature for playing audio from a summary sentence was rejected; original audio preservation remains.
  Designed 2026-09-14 as `docs/superpowers/specs/2026-09-14-vpt-redundant-transcription-design.md`. Three
  engine runs against a synthesized clip with known ground truth settled the shape. whisply on Apple's
  MLX framework and `openai-whisper` with `turbo` produced normalized transcripts that were identical,
  because they are the same `large-v3-turbo` weights on two runtimes, so that pairing is not redundancy
  at all; the design makes a same-model-family pair a startup refusal. Word confidence proved a weak
  signal even on the strong model, ranking correct common words below actual errors, and the one error
  every engine shared ("Muthakrishnan" for Muthukrishnan) was invisible to both disagreement and
  confidence. So the recommendation is three signals rather than two: disagreement between different
  model families, low confidence where an engine reports any, and a risk-class flag on agreed names,
  numbers and dates, aggregated by surface form and suppressed by a `known-terms.txt` the operator grows.
  Note checking is a separate `vpt verify-note` over a timecode source-reference convention, with four
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
  `docs/superpowers/specs/2026-09-14-vpt-redundant-transcription-design.md`. Operator steps: (1) Answer
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
  Whisper, so it is the same model family as whisply and cannot be that engine's auditor. (3) Add a `vpt`
  route to the hermes gateway, or accept that vpt posts on the existing `pns` route. The gateway declares
  exactly `priority`, `pns` and `unattended-upgrades`; posture names a `posture` route that does not
  exist and eight of its Discord legs are dead-lettered with HTTP 404 right now. Settle this before vpt
  sends its first notification rather than after. **Decided 2026-09-15,** full record in
  `docs/decisions/2026-09-15-vpt-architecture-decisions.md`: the architecture is broader than this
  design's own single-pair recommendation. Two engine slots, primary and optional fallback, both named in
  config, neither hardcoded; whether the fallback runs on failure only or on every recording is also
  configurable (decision 3). Reconciliation picks a winner and flags uncertainty exactly two ways, inline
  markers plus a `vpt review <id>` queue that feeds `vpt confirm --term`; the summary-block and
  separate-document shapes were considered and rejected, not merely left unbuilt (decision 4). Three more
  engine-pair behaviors are approved to build (transcript-wins rule, both-engines-fail defaulting to
  keep-and-flag, engine per language), and a disagreement threshold and a cloud spend ceiling are
  explicitly not built, both for lack of a defensible default (decision 5). Engines are named in config
  with two starting first-class adapters, Apple Speech and whisply, plus a generic command escape hatch
  with no uncertainty flagging (decision 10). Notification gains a `[notify]` table with three modes,
  `desktop`, `command`, `off`, superseding this design's pns-only assumption; command mode substitutes
  placeholders into argv and writes vpt's own JSON on stdin simultaneously, vpt needs no copy of pns's
  wire contract, and a translator's exit code is load bearing (decisions 6, 7, 9). `minutes` is out
  entirely (decision 1). Open questions: (1) Which engine pairing, from the priced table? This is Open
  Question 8 and everything else in the design is a configuration value once it is answered. **Narrowed,
  not answered with one pair:** stays a configuration value; Apple Speech and whisply are the two
  starting adapters (decision 10). (2) Is the Apple SpeechAnalyzer route worth a Swift helper inside a
  Rust project? It is the only free different-family option on this Mac and it is confirmed available
  here (macOS 26.2, SpeechTranscriber asset supported, en_US installed), but its confidence reporting is
  unknown and it would put a second language in the build. **Decided 2026-09-15: yes**, it is one of the
  two starting first-class adapters. (3) May a transcript be committed to the vault, and therefore synced
  to a phone? The audio is gitignored and the transcript would not be. The design assumes yes because
  that is what the vault's `transcripts/` directory is for, but it is the decision that puts searchable
  text of every voice memo into a git history. (4) Should a confirmed correction rewrite future
  transcripts? Recording that "Muthakrishnan" should be "Muthukrishnan" is cheap; applying it
  automatically changes the transcript of record with no human reading the result. The design records and
  does not apply. **Confirmed 2026-09-15:** record only, feeding `vpt confirm --term` forward rather than
  rewriting the transcript of record. (5) How loud should the `agreed-unverified` class be? It is the
  class that catches the error every engine shared and also the largest class. The design ranks it last
  and aggregates it by surface form. Should it appear in the pns notification at all, or only in the
  file? (6) One notification per recording with aggregation past three, or one summary per run always? A
  weekly reviewer might prefer the latter. (7) Does `verify-note` belong in vpt or in whatever writes the
  note? It is a separate command precisely because the writer is undecided. If the writer turns out to be
  `minutes`, the check still works but would be checking a third-party tool's output against a timecode
  convention that tool does not follow, which needs the convention enforced somewhere else. **Decided
  2026-09-15: in vpt**, since `minutes` is out and vpt is the only thing that writes the note. (8) Where
  does vpt's code live? The sibling boundaries design recommends its own repository; the sibling
  discovery design assumed a fifth cargo workspace in dotfiles. Nothing in this document depends on the
  answer, but the two designs should not stay in disagreement.
- [ ] Support agent-suggested tags and relationships between recordings, with a defined metadata schema
  and configurable output paths. Use explicit links and deterministic filing rules for automatic routing.
  Markdown output can live in an Obsidian vault and use its existing mobile sync, but Obsidian is
  optional. Designed 2026-09-14 as
  `docs/superpowers/specs/2026-09-14-vpt-metadata-schema-and-filing-design.md`. Three schema homes were
  compared (adopt the vault's own eight-key note schema, adopt `minutes`' published frontmatter JSON
  Schema, or keep a versioned record in vpt's state and render the note from it) and the third is
  recommended, with `minutes`' vocabulary borrowed where it already named a concept. Measured in the
  vault while writing: 753 Markdown files, 738 with frontmatter, 722 note names and 246 aliases,
  scannable in 0.245 s, so the link and vocabulary index is rebuilt every run rather than cached; 122
  distinct tags over 1,109 uses, serialized three different ways and named three different ways (23
  camelCase, 10 kebab, the rest single words), which is why a new tag has no derivable shape and is held
  rather than written; `status`, `hub` and `description` appear in none of the 87 entries of
  `.obsidian/types.json`, so vpt's own keys need no registry edit. Obsidian's documentation settles the
  tag character set, the two link formats and the URL encoding rule, and says a nested property is
  source-mode only, which is why every vpt key is flat and prefixed and why `minutes`' nested `entities`
  was not copied. `yq` confirms an unquoted `[[Note Name]]` in YAML parses as a nested sequence, so wiki
  links in frontmatter are always quoted. Filing is defined as five testable properties (total, pure,
  stable, explainable, collision-free), keyed only on confirmed metadata, pinned at first write, and
  exposed as `vpt path <id> --stage <stage>` so the later note generator files correctly without
  embedding the rules; names are `{date}-{slug}-{hash8}` with the slug sanitized as a trust boundary.
  Links live in a marked managed block that vpt rewrites and never touches prose outside, and a note the
  operator renames in Obsidian is found again by its `vppRecording` key. Retention is a report and not a
  reaper: the whole back catalogue's transcripts are about 187 KB and both engines' raw outputs about 13
  MB, while the audio is nearly free until Apple deletes the original, so the real question is whether
  vpt's copy is the backup. Also corrected while measuring: `relationship_map` is a `minutes` capability
  flag for feature detection, not a subcommand, and the command-line surface is `minutes people`. Waiting
  on the operator: the output layout, the retention answer, whether new tags are held or accepted, and
  the shape of a new tag. No code written. Full document:
  `docs/superpowers/specs/2026-09-14-vpt-metadata-schema-and-filing-design.md`. Operator steps: (1)
  Answer the output-layout half of Open Question 8: audio cloned into the vault's raw/audio/ (the
  discovery design), an archive directory outside the vault with an optional symlink (the boundaries
  design, matching what minutes already does with ~/meetings), or a folder per recording (not
  recommended, it costs a folder note per recording under the vault's folder-note rule). (2) Answer the
  retention half, which is smaller than it looks: transcripts are about 187 KB for the whole back
  catalogue and about 1.5 MB a year, both engines' raw outputs together about 13 MB, and the audio is
  983.6 MiB logically but nearly free physically until Apple's original is deleted. The real decision is
  whether vpt's copy is the backup, and no machine backup exists yet. (3) If the vault layout is adopted,
  write the three missing folder notes for agent-processing-pipeline/, transcripts/ and analysis/
  carrying the reference DataviewJS query from the vault's CLAUDE.md. None exists today, so nothing vpt
  writes would appear in any listing. (4) Decide the shape of a tag that does not exist yet: kebab-case
  (the document's default) or camelCase. The vault corpus splits 10 to 23 the other way, so there is no
  majority to derive it from. One configuration value either way. Open questions: (1) Which output
  layout, and is vpt's audio copy the backup? Everything else in the design is a configuration value once
  that is answered. (2) New tags: held as suggestions (the design's default, because the vault's 122-tag
  vocabulary is small and deliberate and git makes a mistake permanent) or accepted automatically (which
  removes a confirmation step per recording and grows the vocabulary faster than any human would)? (3)
  kebab-case or camelCase for a newly invented tag? The corpus has 10 kebab and 23 camelCase tags, so no
  majority exists. (4) May vpt write into notes it did not create? The mentions relation links out to
  existing contact and project notes; the design writes that link only on the transcript's side and adds
  nothing to the target, and the alternative is vpt editing the operator's own writing. (5) Which mobile
  sync actually carries the vault? Obsidian's core Sync plugin is enabled and obsidian-git is configured
  to push every 15 minutes; they send the same transcripts to different third parties, which the
  transcription design's open question about committing transcripts cannot really be answered without.
  (6) Should `vpt path` stay the contract for the later note generator, or should vpt write the analysis
  note itself? (7) Does minutes stay? If it does, its notes are input that vpt files, and the two schemas
  sit side by side in one vault with no key in common; adopting its schema was rejected for reasons that
  would need revisiting if minutes becomes the note generator rather than a candidate. **Decided
  2026-09-15: no**, minutes is out entirely; there is only vpt's own schema. See
  `docs/decisions/2026-09-15-vpt-architecture-decisions.md`, decision 1. (8) Where does vpt's code live,
  and what is it called? Carried forward unresolved from the boundaries design so the chain does not stay
  in disagreement with itself.
- [ ] Plan meeting briefs using relevant notes, with optional read-only calendar and Todoist inputs.
  Record Bob, the future Hermes executive assistant, as a consumer of vpt's notes and briefs. The exact
  trigger, scheduling owner, access scopes and provider choices remain under discussion. Keep source
  references and unresolved transcription issues visible to Bob and in the brief. Designed 2026-09-14 as
  `docs/superpowers/specs/2026-09-14-vpt-meeting-briefs-design.md`, the fifth document in the vpt chain,
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
  can remind but cannot produce one). vpt assembles and cites rather than writing prose, and prose
  arrives as a proposal checked by the existing `vpt verify-note`. Calendar and Todoist context arrives
  through one `vpt.context/1` document that either vpt's own disabled-by-default collectors or Bob can
  produce, so the question of whether vpt reads the two services or takes them from Bob sets a
  configuration value instead of gating implementation. Bob is recorded as a consumer through
  `vpt.brief/1`: every item carries `certainty` (two values, never a score) and `sources` (recording
  identity, offset, note) with no default, and uncertainty is marked in place inside the line as well as
  counted in a section, because a consumer that quotes one bullet drops a footer and keeps the sentence.
  Selection is four exact selectors over confirmed terms and confirmed tags, capped at 12 and
  explainable; the obvious participant join is nearly empty here, 78 of the 103 notes carrying an
  `email:` key hold `email: []`, so an unmatched participant is a visible line with its confirm command
  rather than a silent omission. A brief is a fourth stage in the existing filing rule table, routed by
  `vpt path`, so there is no second copy of the rules. Waiting on the operator: who holds the two
  credentials, requested against scheduled, which calendars and projects, the Todoist single-slot problem
  (`td` stores one credential per account and it is read-write today, so a read-only login would probably
  replace it), whether a brief may be auto-committed into the vault when it names the people who will be
  in the room, and whether "brief" collides with Forzare's own morning brief. No code written. Full
  document: `docs/superpowers/specs/2026-09-14-vpt-meeting-briefs-design.md`. Operator steps: (1) Decide
  who holds the calendar and Todoist credentials: vpt, or Bob. Either answer works without changing vpt,
  and the shipped default (brief.context.source = "none") is the answer to "neither, yet". If it is Bob,
  this bullet's context half waits for Forzare and vpt still ships useful, which is what L-R5 requires.
  (2) Decide requested against scheduled briefs, and say whether meetings are going to start appearing on
  a calendar. The measurement (not one non-recurring forward item on this machine is timed; 8
  participant-bearing items in the last 90 days) says a scheduler would find nothing today, but that is a
  measurement of the past, not of the intent. (3) Name the calendars and the Todoist projects a brief may
  read. No provider can enforce this, so the configured list is the only scope that exists; an empty list
  refuses by design, so this answer is required before either collector can be turned on at all. (4) If
  vpt is to read the calendar, authorize a read-only Google credential with a command of the form
  `gog auth add <email> --services=calendar --readonly`, and confirm it does not disturb the existing
  one. `gog auth services` shows the calendar service's default scope is the full
  `https://www.googleapis.com/auth/calendar`, so this is a distinct authorization; whether `--client`
  lets a second read-only token bucket sit beside the existing credential was NOT verified, because
  `gog auth list` did not return inside a 20 second timeout. (5) If vpt is to read Todoist, settle the
  single-slot problem. `td accounts list` shows one stored account keyed by numeric id and
  `td auth status` reports it read-write, so `td auth login --read-only` would re-authorize that same
  account and most likely replace the operator's daily credential. Three ways out, in increasing cost: a
  second Todoist account sharing the relevant projects; a dedicated read-only token in KeePassXC used
  directly, making a sixteenth secret-bearing target; or Bob supplying the task context. (6) When the
  vault layout is adopted, write a folder note for `briefs/` carrying the reference DataviewJS query,
  making four in total alongside the three the metadata design already named. vpt writes no folder notes.
  Open questions: (1) Does vpt read the calendar and Todoist, or does Bob supply them? The design makes
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
  keep-or-replace ruling and does not need answering before it. **Moot, decided 2026-09-15:** minutes is
  out entirely; there is no `research` or `person` output to draw from. See
  `docs/decisions/2026-09-15-vpt-architecture-decisions.md`, decision 1. (8) Is the local Apple Calendar
  store representative of the operator's calendars? The measurement came from that store (16 calendars),
  and recommending the Google interface assumes the meetings that matter are in Google. A calendar
  existing only in Calendar.app would be invisible to this design. (9) Where does vpt's code live, and
  what is it called? Carried forward unresolved from the boundaries design, because the chain should not
  stay in disagreement with itself.
- [ ] Support a separate redacted draft for sharing, reviewed before release, preserving private
  originals. Choose the summary format, retention, transcription engines and local/cloud processing
  before implementation. Speaker labels and dated digests remain unapproved candidates. 2026-09-14:
  design written at `docs/superpowers/specs/2026-09-14-vpt-redacted-sharing-design.md` (sixth in the vpt
  chain, building on the meeting-briefs design). Recommendation: four verbs
  (`vpt share draft|review|approve|release`), with the draft assembled by allowlist into
  `~/.local/state/vpt/share/<draft-id>/` at mode 0700, outside the vault and outside any git working
  tree, because the vault's Obsidian Git settings were measured at a 10-minute commit and a 15-minute
  push, so a draft built in the vault is published before anyone reviews it. Release refuses without an
  approval record whose SHA-256 digest matches the current draft bytes and policy, re-runs the residue
  scan rather than trusting the draft-time pass, and refuses a destination inside a git working tree, a
  cloud-sync root, the output root, the audio destination or vpt's state tree; approval refuses when
  standard input is not an interactive terminal and requires the operator to type the draft's short
  digest, measured against an agent shell on this machine that has no terminal and sets `CLAUDECODE=1`.
  Redaction is deterministic over confirmed `known-terms.txt` entries plus six closed pattern classes,
  with flagged spans omitted by default and a capitalized-token candidate report for what no pattern can
  find: measured on one real 639-word vault note, 33 unique capitalized tokens of which 13 were personal
  names, which is why the heuristic is a review prompt and never an automatic mask. vpt transmits
  nothing; release writes a file. Speaker labels, dated digests, sending, watermarking, model-based
  detection and audio redaction are out of scope. Not approved and not built; 14 assumptions and 9 open
  questions are recorded, and the ledger's own four choices (summary format, retention, engines, local or
  cloud processing) gate implementation. Full document:
  `docs/superpowers/specs/2026-09-14-vpt-redacted-sharing-design.md`. Operator steps: (1) Answer the four
  choices this ledger bullet names, as four separate decisions rather than one: summary format (extract
  or generated prose, noting that for prose the human read is the only real protection), retention of
  drafts and released copies (a draft directory holds the placeholder-to-real-value map, so it is more
  sensitive than the transcript), transcription engines, and local or cloud processing. The last one
  matters here for a reason the transcription design did not raise: a recording transcribed by a cloud
  engine already left the machine once, before any redaction existed, and this gate only protects the
  second egress. (2) Choose the release directory and confirm it is not synced anywhere. The proposed
  default is ~/Documents/vpt-shared; measured, ~/Documents on this machine is local and not redirected
  into iCloud, but an iCloud Drive container is active (brctl reports 98 containers, CloudDocs last
  synced 2026-09-08). (3) Say which source kinds may be drafted from: transcript, analysis note, brief,
  or all three. The brief is the one worth a moment's thought, because it names the people who will be in
  a room. (4) Confirm the approval ritual: an interactive terminal plus typing eight digest characters,
  with no bypass flag. If that is too heavy for how you actually share things, change the ritual now
  rather than adding a bypass later. (5) Nothing to install or seed beyond the existing review flow:
  every `vpt confirm --term` during transcript review improves every future draft, so expect the first
  few candidate reports to be long. Open questions: (1) What is a shared draft made of: an extract, or
  written prose? For an extract the residue scan is a real mechanical check; for prose a paraphrase can
  reintroduce a redacted fact in words the pass never saw, and the human read is the only protection. (2)
  What is the retention of drafts and of released copies? A draft directory holds the map from each
  placeholder to the real value it replaced, which makes the draft tree the most sensitive directory in
  the chain, and vpt deletes nothing. (3) Should pseudonyms be stable across drafts? Stable numbering is
  friendlier for a recipient reading several and lets two drafts be correlated by anyone holding both;
  per-draft numbering is the proposed default. (4) May a brief be drafted from at all? It concentrates
  participants and context, which is what makes it useful and what makes it the most exposing source in
  the chain. (5) Should an approval expire? An approval taken today and released in three weeks reviewed
  the same bytes but not the same situation. (6) Does the released file say it came from vpt? The
  proposed source line says only that the file is a redacted extract and not a verbatim record; naming
  the tool is more honest about provenance and tells a recipient a recording exists. (7) Is the PDF path
  in scope later? The vault already has a PDF export recipe, and a PDF carries producer, timestamp and
  sometimes path metadata that a Markdown file does not. (8) If `minutes` stays, should its `vocabulary`
  feed terms alongside known-terms.txt? Two lists that disagree would mask a name in one pipeline and not
  the other. Downstream of the minutes keep-or-replace ruling. **Moot, decided 2026-09-15:** minutes is
  out entirely; there is only known-terms.txt. See
  `docs/decisions/2026-09-15-vpt-architecture-decisions.md`, decision 1. (9) Where does vpt's code live,
  and what is it called? Carried forward unresolved from the boundaries design.
- [ ] 138. `vpt note`, quick voice notes filed where the operator says. A subcommand that records one
  voice note, transcribes it, and when the recording ends prompts for a destination: one of the pre-saved
  projects from vpt's config, the current working directory, or a directory chosen at the prompt. The
  same subcommand takes a flag that skips the prompt: `--project <name>` for a pre-saved project or
  `--dir <path>` for an absolute or relative directory. The note lands as a dated markdown file with the
  transcript and a link to the audio; audio stays out of Git. First use is the operator's end-of-shift
  pattern journal for Broccoli, filed to `~/workspaces/Ivy/career-campaign/broccoli/`, with a weekly
  rollup of recurring issue types as a later piece. Requested 2026-09-17.
- [ ] Keep vpt application code in its own project, Mac installation and service configuration in
  dotfiles, output content in the configured directory (Ivy for this operator), and homelab deployments
  in homelab. Reuse existing transcription tasks. vpt must work without Bob, Forzare or the full homelab;
  Forzare integration follows the existing post-modernization ordering. Designed 2026-09-14 as
  `docs/superpowers/specs/2026-09-14-vpt-project-boundaries-design.md`. It compares three code homes (a
  fifth cargo workspace in dotfiles, its own repository built from a local clone, its own repository
  installed by `cargo install --git`) and recommends the second: `webdavis/vpt` from day one, built in
  the scalebar shape, which is already the proven precedent on this machine (`.chezmoidata/scalebar.yaml`
  plus a deferral-guarded builder plus one LaunchAgent). dotfiles' share is named file by file and stops
  at installation and service configuration; the configured output directory holds notes and links under
  the existing `agent-processing-pipeline/` layout; homelab keeps Open Notebook (L6) and any remote
  engine, all optional. Apple's container stays the canonical original and is read-only to vpt, including
  a `mode=ro&immutable=1` SQLite open, with one archive copy outside git because the vault ignores audio
  and no machine backup exists yet. Measured while writing: a `com.webdavis.vpt.plist` and a
  `~/.cargo/bin/vpt` fall outside the osquery known-good manifests, so vpt adds no CRIT coupling; pns
  needs no producer registration and `needs_attention` is real
  (`pns/crates/pns-protocol/src/request.rs:56`); `minutes` is already installed, already documents voice
  memos, and already owns the vault's `agent-processing-pipeline/minutes` symlink. The independence
  done-means is written as four absence checks (dotfiles absent, pns absent, network absent, vault and
  Obsidian absent) rather than mocks of Bob and Forzare, which do not exist yet. Waiting on the operator:
  own repository versus fifth workspace, and the shipping name, since `VPP` is FD.io's Vector Packet
  Processing and the repository's rules discourage new acronyms. No code written. Full document:
  `docs/superpowers/specs/2026-09-14-vpt-project-boundaries-design.md`. Operator steps: (1) Read
  docs/superpowers/specs/2026-09-14-vpt-project-boundaries-design.md and answer the seven open questions
  at its end. (2) Decide the code home first: own repository (recommended) or a fifth cargo workspace in
  dotfiles. Everything else in the document hangs off that answer. (3) Decide the shipping name before
  any repository is created; `VPP` is taken by FD.io's Vector Packet Processing (verified on fd.io) and
  renaming after install instructions circulate is a breaking change. (4) If the own-repository answer
  holds, create `webdavis/vpt` and clone it to ~/workspaces/Ivy/webdavis/vpt; the dotfiles builder is
  written to defer cleanly until that clone exists, so no apply is blocked in the meantime. (5) Rule on
  `minutes`: it is already installed, declared as a cask, documents "meetings and voice memos", and owns
  the vault's agent-processing-pipeline/minutes symlink. Answer before vpt's ingestion design is
  approved, since adopting it removes a layer. (6) Decide whether the audio archive copy may live
  unbacked on one machine until the restic work exists, or whether a backup is a prerequisite. (7) No
  apply, no build and no code are needed for this item; it is a document waiting on decisions. Open
  questions: (1) Own repository (`webdavis/vpt`) or a fifth cargo workspace inside dotfiles?
  Recommendation: own repository, matching the 2026-09-05 plugin ruling and the 2026-09-08
  shippable-product ruling. (2) Does the tool keep the name `vpp`? The acronym belongs to FD.io's Vector
  Packet Processing and the repository's own rules discourage introducing uncommon acronyms; the naming
  memories ask for self-documenting, user-agnostic names. **Decided 2026-09-15: renamed to `vpt`, Voice
  Processing Tool.** `vpp` is also taken on crates.io by an abandoned 0.0.1 placeholder, so
  `cargo install vpp` would fetch the wrong crate and `cargo publish` under it is refused permanently.
  Every two-letter option was taken, `vtp`, `vrp` and `vnp` clash, and `vpe` was rejected because the
  tool's own config names a primary and a fallback engine. `vpt` measured free (HTTP 404). Full record:
  `docs/decisions/2026-09-15-vpt-architecture-decisions.md`, decision 14. (3) Which copy of the audio is
  canonical, and is one unbacked copy acceptable until a real backup exists? Recommendation: Apple's
  container stays canonical, the archive copy lives outside git, and backups stay a separate ledger item.
  (4) Do vpt's notes share the vault's existing `transcripts/` and `analysis/` directories, or get their
  own `agent-processing-pipeline/vpt/` subtree beside the `minutes` symlink? (5) Is `minutes` in or out?
  It already lists and searches voice memos and already owns a directory inside the vault; if it is in,
  vpt's scope shrinks, and if it is out, its open tool evaluation should record that vpt supersedes it.
  **Decided 2026-09-15: out.** vpt does not use or depend on `minutes`, in any form; the operator's own
  words are that `minutes` is poorly designed, though it has good features worth learning from. vpt
  supersedes it. See `docs/decisions/2026-09-15-vpt-architecture-decisions.md`, decision 1. (6) Do the
  vault's folder-note and frontmatter conventions apply to machine-written notes, and who maintains the
  folder note for a directory a tool writes into? (7) Should vpt's binary, configuration and LaunchAgent
  join posture's user-configured watch list? They sit outside the osquery known-good manifests by default
  (verified), so this is an opt-in rather than a consequence.

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

- [ ] Let Bob consume vpt's transcripts, metadata and briefs for meeting preparation. Keep provenance and
  unresolved transcription warnings visible; do not turn uncertain notes into confirmed commitments.
  Decide whether Bob supplies optional calendar/Todoist context or vpt reads it directly during design.

- [ ] After the canonical Forzare documents merge, replace the research folder's Phase 2 copy with a
  pointer to them. Reconcile the residue in
  `~/Documents/ADHD_Task_System_Research_20260521/REVIEW-LEDGER-2026-07-11.md`. Its post-v1 items remain
  deferred: the mutation wrapper, per-channel delivery lease, Langfuse, email/communications triage and
  extra delivery lanes. Old graphify/pre-commit complaints need current verification before reopening.

- [ ] 127. Recurring-charge watch, part of Forzare, approved 2026-09-17. A monthly job runs YNAB's
  recurring-charge detection and posts only charges that are NEW since the last run (a payee and amount
  the previous snapshot did not carry) to the operator's Discord, one line each with the payee, amount
  and first-seen date, so a forgotten trial or a price rise is caught the month it starts. Read-only
  against YNAB, snapshot kept under the tool's state directory, nothing posted when nothing is new.

- [ ] 128. Gmail triage into Todoist, part of Forzare, approved 2026-09-17. A daily read-only pass over
  the inbox that turns actionable mail (a reply owed, a bill, a deadline, a form) into Todoist tasks
  carrying a link back to the thread and a due date when the mail names one, skips mail it already filed,
  and posts a three-line card (filed, skipped, needs a look) to the phone. It never sends, archives,
  labels or deletes mail, and the operator sees every filed task in Todoist's inbox before anything else
  happens.

- [ ] 134. Money mission control, managed by Forzare and posted to a dedicated Discord channel of its
  own, approved 2026-09-17. A cash-flow forecast from YNAB's scheduled transactions and income, bill-due
  escalation through pns, "can I afford X" answered from the phone, and a monthly one-page. Task 127 (the
  recurring-charge watch) is its first piece and posts to the same channel. Read-only against YNAB; the
  channel id is a KeePassXC entry like every other channel.

## After the ledger: approved ideas awaiting design

Three ideas the operator approved in principle on 2026-09-17 and ruled NOT to start until the rest of
this ledger is done. They are recorded so they are not lost and so no overnight run picks them up. Each
needs a design conversation with the operator first.

- [ ] 136. The second brain that answers, a Forzare feature. One local index over the vault, Readwise,
  transcripts, Todoist and the calendar, asked from any harness or from the phone through hermes, and fed
  to agents so they stop re-asking rulings. Operator ruling 2026-09-17: the data sources are chosen
  carefully and deliberately, one at a time, and this waits until the ledger is finished.
- [ ] Broccoli role tools live in the Todoist project `broccoli` (id `6hWq5H2cg7M3PfX7`), not here:
  `/investigate` (`6hWq5Hj6H9hwhjP7`) and `/escalate` (`6hWq5HwjRqVrFc8f`), both approved 2026-09-17 and
  both blocked until the operator has learned the real support workflow of the role. No overnight run
  starts them.
- [ ] 137. Campaign, the job-search crew, a future idea. The operator started at Broccoli AI on 2026-09
  (`~/workspaces/Ivy/career-campaign/broccoli/ai-tech-support/`) and is focused on that role; nothing
  here is built now.

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
1. vpt: decide permitted local/cloud processing and cost before selecting redundant engines. Then settle
   whether vpt reads optional calendar/Todoist context directly or accepts it from Bob, which calendars
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
