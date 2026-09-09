# Remaining work

The open task list for the pns, posture, uu, lights, Neovim and tailnet-pin program. Ordered, one task at
a time, with stopping points that leave the tree in a state worth applying.

Updated as tasks complete. Last updated 2026-09-08.

## Where things stand

`main` carries eighty-six merged pull requests from 2026-09-06 onward and the first full `chezmoi apply`
since then has now run and passed.

`main` is green as of run 34301231057. Both red cases turned out to be wrong assertions rather than
broken code, and both repairs are mutation-verified.

Every pull request tracked by the 2026-09-06 Codex handoff has merged. Two pull requests remain open,
`#24` and `#51`, and both predate this program and are unrelated to it. This file replaces that handoff.

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

- [ ] 11c. Bootout `com.webdavis.update-skills`, then trash its plist and the two scripts tasks 55 and 56
  retire: `~/Library/LaunchAgents/com.webdavis.update-skills.plist`,
  `~/.local/libexec/unattended-upgrades/agent-skills/update-skills.sh` and
  `~/.local/libexec/unattended-upgrades/helpers/log-entries.sh`. Deleting the chezmoi source does not
  delete the deployed copy, and the LaunchAgent stays loaded until it is booted out.

- [ ] 11d. Clear stale `~/.claude/ide/*.lock` files. A lock whose Neovim is gone makes claudecode.nvim
  open a plain HTTP connection to a dead port and warn `Missing or invalid Upgrade header` on every file
  open. Three were found on 2026-09-08, two of them nearly three days old.

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

## The Rust extraction program

The operator's standing constraint: nothing moves until a plan exists. Seven tools leave this repository
for their own public repositories, and most of the chezmoi machinery that builds and deploys them gets
deleted rather than rewritten.

- [ ] 20. Write the extraction plan: the seven repositories, what each one takes with it, what this
  repository deletes, and the order.
- [ ] 21. Get the plan approved before touching anything.

## pns closure and the rescued lanes

- [ ] 22. Rework `fix/pns-retry-backoff` against the current crate layout, PR, merge
- [ ] 23. Review and push `feat/posture-producer-commands` (heartbeat), PR, merge
- [ ] 24. Review and push `feat/posture-converge-staging` (6 commits), PR, merge
- [ ] 25. pns 18.1a: the `cargo doc` gate with `RUSTDOCFLAGS="-D warnings"`
- [ ] 26. pns 8.4: the Codex and Claude hook-table verification record
- [ ] 27. pns: backfill decision record 0012 (SQLite two fail directions)
- [ ] 28. pns 18.1b: the completion report and line counts

### STOP POINT B

The pns refactor plan is closed and nothing is stranded.

## Delivery failure reporting

Designed in `docs/superpowers/specs/2026-09-08-pns-delivery-failure-reporting-design.md`.

- [ ] 29. Plan the build, the pull request breakdown from the spec
- [ ] 30. Permanent versus temporary classification in `pns-domain`
- [ ] 31. The ledger columns and the persisted failure record
- [ ] 32. The message, both render forms, the per-destination meaning tables
- [ ] 33. `pns failures` and the `pns doctor` routing to it
- [ ] 34. The `pns doctor` route check
- [ ] 35. The banner click: `pns click`, and its three configured types
- [ ] 36. The local page for moshi's browser preview

### STOP POINT C

A delivery failure is now loud, specific, and quick to act on, so posture can be trusted to page.

## posture foundation

- [ ] 37. posture 2.4: page, domain digest, protocol codec
- [ ] 38. posture 2.9: `drift.rs` and `converge_policy.rs`
- [ ] 39. posture 2.10: `cursor.rs` and `triage.rs`
- [ ] 40. posture 3.1 remainder: four adapters
- [ ] 41. posture 3.2 remainder: tailscale, process, gateway, `LaunchdState`
- [ ] 42. posture 3.3: the converge read half, staging, privileged

### STOP POINT D

The foundation is complete and nothing has cut over, so there is no runtime risk yet.

## posture cutovers

Every task in this section needs the pns-keyed gateway route to exist first. Adding it is an operator
step, and it gates the whole section.

- [ ] 43. posture 6.1: heartbeat cutover
- [ ] 44. posture 6.2: digest cutover
- [ ] 45. posture 6.3: alert cutover
- [ ] 46. posture 6.4: watchdog cutover
- [ ] 47. posture 6.5: poll cutover
- [ ] 48. posture 6.6: funnel cutover
- [ ] 49. posture 6.7: drainer retirement
- [ ] 50. posture 7.1: converge cutover

### STOP POINT E

Every posture producer is Rust and the old pipeline is off.

## uu

- [ ] 51. uu B1: `rust-toolchain.toml`, needs the stable toolchain certified
- [ ] 52. uu D1, D2, D3: the cargo lane and `RustupLane`
- [x] 53. uu E12, E13, E14: skills hermes and forks
- [x] 54. uu E15a, E15: the skills orchestrator
- [x] 55. uu E16: retire `update-skills.sh` and its LaunchAgent
- [x] 56. uu E17: retire `log-entries.sh`
- [ ] 57. uu E19: the log rotation lane, needs the hourly log writer stopped

### STOP POINT F

uu is complete.

## posture cleanup

- [ ] 58. posture 8.1, 8.2, 8.3: the ssh-hardening port, then trash the deployed
  `~/.local/bin/ssh-hardening.sh` it replaces
- [ ] 59. posture 9.1: osquery leaves the tracked set
- [ ] 60. posture 9.2: the completion report and 187-name mapping table

### STOP POINT G

posture is done and osquery is retired.

## The tail

- [ ] 61. lights: the argument-surface differential, owed since PR 1
- [ ] 62. lights PR 12: the aerospace keys F4 to F10 off the bash script
- [ ] 63. lights: the manifest decision for `~/.local/libexec/lights`
- [ ] 64. lights PR 11a, conditional on the `bulk_read_latency` drill
- [ ] 65. Neovim task 63: the acceptance record, needs the clean-home apply
- [ ] 66. tailnet-pin: the Rust crate replacing `reconcile-hosts-pin.sh`
- [ ] 66a. herdr: the clean-code pass on `dot_local/share/herdr/plugins/herdr-smart-nav`, approved and
  scheduled after posture

## Repository hygiene

- [ ] 67. Delete the dead local branches. About a hundred of them: every `wf_*`, `worktree-agent-*` and
  `agent-*` name, plus the old `backup/*` and throwaway experiment branches. Each group needs the
  operator's approval before it goes.
- [ ] 68. Remove the worktrees those branches left under `~/.herdr/worktrees/`. There are 195 of them.
  Their cargo `target/` directories are already cleaned, so this is about clutter rather than disk.
  Remove with `git worktree remove`, never `rm`, and keep any worktree whose branch still holds commits
  that are not on origin.

## Waiting on the operator

Each of these gates work that cannot start without it.

- [ ] Add the pns-keyed gateway route, gates every posture cutover, tasks 43 to 50
- [ ] Certify the stable Rust toolchain, gates tasks 51 and 52
- [ ] Stop the hourly log writer, gates task 57
- [ ] The clean-home apply from PR #385, gates task 65
- [ ] The lamp drills, gates task 64
- [ ] Archive `webdavis/neovim-config` and remove `~/.config/nvim/.git`
- [ ] Approve the branch and worktree deletions, gates tasks 67 and 68
- [ ] Restart Claude Code so the old `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE` leaves the process environment

## Deferred, not scheduled

Carried over from the Codex handoff so it is not lost when that file goes. None of these start without
the operator saying so.

- SP4 and SP5, the shell migration to xonsh. Waits until the Neovim overhaul is finished and the operator
  says go.
- SP3 part 2, the A to H feature proposal. Not approved; it needs an operator decision and a credential.
- SP-nix, the nix-darwin go or no-go. Deferred, research first.
- SP7, the sweep and backlog pass including Todoist hygiene. After everything above.
- The babysitter execute-bit issue, draft in `~/.claude/pipeline/babysitter-issues/`. Deferred by the
  operator on 2026-09-06.

## Open questions

- posture 0.2, the pns priority-route, could not be confirmed as done because the hermes config is
  age-encrypted and unreadable. It is one of four pns prerequisites the posture plan names as gating the
  cutovers, so it should be settled before task 43.
- `webdavis/pns.nvim` is its own repository and was not audited. Task 14 finishes its integration here,
  but unfinished work inside that repository would not have shown up.
