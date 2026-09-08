# Remaining work

The open task list for the pns, posture, uu, lights, Neovim and tailnet-pin program. Ordered, one task at
a time, with stopping points that leave the tree in a state worth applying.

Updated as tasks complete. Last updated 2026-09-08.

## Where things stand

`main` is at `7898b061`. Eighty-six pull requests merged between 2026-09-06 and 2026-09-08 and none of
them have been applied, so the next `chezmoi apply` deploys all of it at once.

Five branches exist only on this machine and are not pushed anywhere. Three of them are finished, tested
batches that were waiting on a push approval that went unanswered. Rescuing them is most of the work
before the first stopping point.

## Before the first stopping point

- [x] 1. Merge PR #458, the auto-commit spec fix and module rename
- [ ] 2. Commit and push the failure-reporting spec
- [ ] 3. Push `feat/nvim-pns-wiring` (Neovim task 26, pns.nvim), PR, merge
- [ ] 4. Push `feat/uu-tooling-e12-e17` (7 commits), PR, merge
- [x] 5. Merge main into PR #448 (herdr), merge
- [x] 6. Pre-apply verification: build pns and posture, headless Neovim start, zero stderr

### STOP POINT A

Everything above is additive. posture has not cut over, so the existing osquery pipeline keeps running
untouched. This is the recommended place to stop and apply.

At the apply: KeePassXC must be unlocked, Neovim needs `:Lazy restore`, and herdr needs a config reload
for the new keybindings.

## pns closure and the rescued lanes

- [ ] 7. Rework `fix/pns-retry-backoff` against the current crate layout, PR, merge
- [ ] 8. Review and push `feat/posture-producer-commands` (heartbeat), PR, merge
- [ ] 9. Review and push `feat/posture-converge-staging` (6 commits), PR, merge
- [ ] 10. pns 18.1a: the `cargo doc` gate with `RUSTDOCFLAGS="-D warnings"`
- [ ] 11. pns 8.4: the Codex and Claude hook-table verification record
- [ ] 12. pns: backfill decision record 0012 (SQLite two fail directions)
- [ ] 13. pns 18.1b: the completion report and line counts

### STOP POINT B

The pns refactor plan is closed and nothing is stranded.

## Delivery failure reporting

Designed in `docs/superpowers/specs/2026-09-08-pns-delivery-failure-reporting-design.md`.

- [ ] 14. Plan the build, the pull request breakdown from the spec
- [ ] 15. Permanent versus temporary classification in `pns-domain`
- [ ] 16. The ledger columns and the persisted failure record
- [ ] 17. The message, both render forms, the per-destination meaning tables
- [ ] 18. `pns failures` and the `pns doctor` routing to it
- [ ] 19. The `pns doctor` route check
- [ ] 20. The banner click: `pns click`, and its three configured types
- [ ] 21. The local page for moshi's browser preview

### STOP POINT C

A delivery failure is now loud, specific, and quick to act on, so posture can be trusted to page.

## posture foundation

- [ ] 22. posture 2.4: page, domain digest, protocol codec
- [ ] 23. posture 2.9: `drift.rs` and `converge_policy.rs`
- [ ] 24. posture 2.10: `cursor.rs` and `triage.rs`
- [ ] 25. posture 3.1 remainder: four adapters
- [ ] 26. posture 3.2 remainder: tailscale, process, gateway, `LaunchdState`
- [ ] 27. posture 3.3: the converge read half, staging, privileged

### STOP POINT D

The foundation is complete and nothing has cut over, so there is no runtime risk yet.

## posture cutovers

Every task in this section needs the pns-keyed gateway route to exist first. Adding it is an operator
step, and it gates the whole section.

- [ ] 28. posture 6.1: heartbeat cutover
- [ ] 29. posture 6.2: digest cutover
- [ ] 30. posture 6.3: alert cutover
- [ ] 31. posture 6.4: watchdog cutover
- [ ] 32. posture 6.5: poll cutover
- [ ] 33. posture 6.6: funnel cutover
- [ ] 34. posture 6.7: drainer retirement
- [ ] 35. posture 7.1: converge cutover

### STOP POINT E

Every posture producer is Rust and the old pipeline is off.

## uu

- [ ] 36. uu B1: `rust-toolchain.toml`, needs the stable toolchain certified
- [ ] 37. uu D1, D2, D3: the cargo lane and `RustupLane`
- [ ] 38. uu E12, E13, E14: skills hermes and forks
- [ ] 39. uu E15a, E15: the skills orchestrator
- [ ] 40. uu E16: retire `update-skills.sh` and its LaunchAgent
- [ ] 41. uu E17: retire `log-entries.sh`
- [ ] 42. uu E19: the log rotation lane, needs the hourly log writer stopped

### STOP POINT F

uu is complete.

## posture cleanup

- [ ] 43. posture 8.1, 8.2, 8.3: the ssh-hardening port
- [ ] 44. posture 9.1: osquery leaves the tracked set
- [ ] 45. posture 9.2: the completion report and 187-name mapping table

### STOP POINT G

posture is done and osquery is retired.

## The tail

- [ ] 46. lights: the argument-surface differential, owed since PR 1
- [ ] 47. lights PR 12: the aerospace keys F4 to F10 off the bash script
- [ ] 48. lights: the manifest decision for `~/.local/libexec/lights`
- [ ] 49. lights PR 11a, conditional on the `bulk_read_latency` drill
- [ ] 50. Neovim task 63: the acceptance record, needs the clean-home apply
- [ ] 51. tailnet-pin: the Rust crate replacing `reconcile-hosts-pin.sh`

### DONE

## Waiting on the operator

Each of these gates work that cannot start without it.

- [ ] Add the pns-keyed gateway route, gates every posture cutover, tasks 28 to 35
- [ ] Run `chezmoi apply`, nothing has been applied through 86 merges
- [ ] Certify the stable Rust toolchain, gates task 36 and 37
- [ ] Stop the hourly log writer, gates task 42
- [ ] The clean-home apply from PR #385, gates task 50
- [ ] The lamp drills, gates task 49
- [ ] Archive `webdavis/neovim-config` and remove `~/.config/nvim/.git`

## Open questions

- posture 0.2, the pns priority-route, could not be confirmed as done because the hermes config is
  age-encrypted and unreadable. It is one of four pns prerequisites the posture plan names as gating the
  cutovers, so it should be settled before task 28.
- `webdavis/pns.nvim` is its own repository and was not audited. Task 3 finishes its integration here,
  but unfinished work inside that repository would not have shown up.
