# Dotfiles modernization audit handoff

- Date: 2026-07-10
- Repository: `webdavis/dotfiles`
- Audited worktree: `slice/tailscale`
- Current pull request (PR): #38

Primary documents:

- `docs/superpowers/plans/2026-07-03-sp2-combine-and-split.md`
- `docs/superpowers/specs/2026-07-02-repo-modernization-roadmap-design.md`

The audit itself was read-only. This handoff file is the only repository change made from it.

## Instructions for Fable

1. Verify each finding against the current branch before editing.
1. Correct the plan and roadmap before implementing affected work.
1. Keep logically separate fixes in separate commits and PRs.
1. Use test-driven development (TDD) for behavioral changes.
1. Run `just lint-check` and `just test` before each handoff.
1. Do not perform live cutover, Secure Shell (SSH) changes, service removal, or bare `chezmoi apply`
   without operator approval.
1. Do not close the integration/reference PRs until the final soak passes.

## Current state

- PR #35, S2 lint/test/continuous integration (CI) work, is merged.
- PR #36, S3 skills architecture, is merged.
- PR #37, S4 herdr migration, is merged.
- PR #38, S5 Tailscale, is open with passing lint.
- The `slice/tailscale` worktree was clean at the end of the audit.
- `git diff --check origin/main...HEAD` passed.
- `just test` passed.
- `just lint-check` passed.

Green checks do not cover the live-state, retry, service-ownership, or cutover defects below.

## Important correction: keep Tailscale's copied daemon

The earlier recommendation to restore `sudo brew services start tailscale` was wrong.

Homebrew can run the service. It was deliberately rejected during the original PR #31 implementation
because running `brew services` through `sudo` changed ownership of the Tailscale Cellar, `opt`,
linked-keg, `bin`, and `sbin` paths to root. The weekly Homebrew upgrade runs as the user, so that
ownership broke the unattended upgrade model.

Evidence:

- PR #31 commit `3b43d67` initially implemented `sudo brew services`.
- The service eventually started successfully. Its first failure was caused by the GUI cask uninstall
  also removing the formula.
- The live cutover then stopped the Homebrew service and restored user ownership with `chown`.
- Commit `01d15ad` switched to `tailscaled install-system-daemon`.
- Claude's project memory at
  `~/.claude/projects/-Users-stephen-workspaces-Ivy-webdavis-dotfiles/memory/tailscaled-migration.md`
  records this as one of the four cutover gotchas.
- The installed Homebrew service code deliberately performs the ownership change at
  `/opt/homebrew/Homebrew/Library/Homebrew/services/cli.rb:286-349`.
- The current daemon is the root-owned `/usr/local/bin/tailscaled`.
- The Homebrew formula remains user-owned.
- The copied daemon and formula binary were byte-identical during the audit.
- PR #35 contains no Tailscale decision or implementation.

Recommendation:

- Keep `install-system-daemon`.
- Keep S6 responsible for re-copying the daemon after a formula upgrade.
- Add a clear supersession note to the old June Tailscale spec and plan. Those files still say Homebrew
  service management was the final decision.
- Do not leave the explanation only in Claude's untracked project memory.

## Findings in merged PRs #35-#37

### PR #35: lint and test foundation

#### Medium: rendered shell-template coverage regressed

`treefmt.nix:218-227` explicitly covers five chezmoi scripts plus `dot_bashrc.tmpl`. There are 20
shell-script templates. The old linter covered about twelve.

A full manual sweep found four hidden failures:

- An unquoted `$HOME` in the system-setup render.
- Three osquery loaders whose shebang renders on line two.

Required correction:

- Discover all safely renderable shell templates programmatically.
- Fix the four current failures.
- Add a coverage test that fails when a new shell template is omitted.

#### Low: active documentation is stale

- `CLAUDE.md:285` says Dependabot does not auto-merge, although the workflow does.
- `private_dot_claude/CLAUDE.md:77-78` says the commit hook uses Haiku, but it uses Sonnet.
- `CLAUDE.md:123` says "Both hooks" before listing three.

Correct these behavior claims now. Leave the larger instruction-file rewrite to S12.

### PR #36: skills architecture

#### High: updates can defer forever

`dot_local/bin/executable_update-skills.sh:350-354` exits successfully whenever any Claude, Codex, or
Hermes process exists. The LaunchAgent runs only Monday at 04:00 and has no retry. The live machine had
five Claude processes, one Codex process, and two logged deferrals.

Required correction:

- Distinguish active sessions from persistent background processes.
- Schedule bounded retries after a deferral.
- Alert when the retry budget is exhausted.

#### High: fresh-machine installation is not started automatically

`--install-only` exists, but only the manual justfile recipe invokes it. The loader bootstraps a
`RunAtLoad=false` LaunchAgent, so a fresh machine may wait until Monday.

Add a deterministic first-install trigger keyed to the lock and updater. Test it from an empty home
directory, including idle-gate retry behavior.

#### High: skill fan-out is additive instead of convergent

The updater creates missing links but does not remove obsolete links or replace valid links pointing to
the wrong target. Live Hermes state had 29 store links versus 13 declared links, leaving 16 stale
default-profile links. A scratch run preserved both a stale link and a wrong-target link while exiting
successfully.

Compute the desired `(profile, skill, target)` set and converge only updater-owned symlinks. Preserve
catalog and registry-owned directories. Test removal, relocation, wrong targets, broken links, and
collision names.

#### Low: PR #38 carries an unrelated skills-test fix

The hermetic change to `test/update-skills-launchagent-path.sh` is reasonable but belongs in the skills
stabilization PR, not S5.

### PR #37: herdr migration

#### High: the live migration is not operationally atomic

The system-packages runner removes tmux and sesh during `before_10`, while the herdr installer runs later
during `before_15`. If installation fails, the machine can have neither multiplexer.

Install and verify herdr before cleanup. Retain tmux and sesh until the binary, configuration, plugins,
and a second session are proven. Include rollback instructions.

#### High: missing Cargo or failed plugin registration is treated as success

`.chezmoitemplates/herdr-plugin-build.sh.tmpl:28-36` exits zero when Cargo is missing and consumes the
`run_onchange` trigger. Plugin-link failures also exit zero without verifying registration. Required
keybindings already point to these plugins.

Separate build from registration, recognize herdr error envelopes, verify the exact plugin after linking,
and retain retry state until registration succeeds.

Normal Cargo compilation errors are not swallowed. `set -e` makes those fail. The false-success cases are
missing Cargo and failed plugin registration.

#### Medium: rebuild hashing omits `Cargo.toml`

The template hashes `main.rs`, `Cargo.lock`, and `herdr-plugin.toml`, but not `Cargo.toml`. Hash every
tracked build input and test that changing each input changes the rendered trigger.

#### Medium: removing the old Claude LaunchAgent source does not unload it

PR #37 deletes the plist and loader, but no managed script runs `launchctl bootout` or removes an
already-deployed plist. The service happens to be absent on this machine, but fresh convergence is
incomplete.

Add a one-time idempotent retirement script with stubbed tests.

## PR #38 findings

PR #38's daemon-ownership model is correct. It should remain open for the following fixes.

### 1. Status classification is incomplete

`.chezmoiscripts/run_onchange_after_66-tailscaled-status.sh.tmpl:15-26` recognizes only text matching:

```text
logged out
needslogin
not logged in
```

Every other non-success response is reported as "daemon is not running" and recommends reinstalling it.

Tailscale 1.98.8 has these relevant states:

- `Running`
- `Starting`
- `NeedsLogin`
- `NeedsMachineAuth`
- `Stopped`

`NeedsMachineAuth` prints "Machine is not yet approved by tailnet admin." `Stopped` prints "Tailscale is
stopped." Both are currently misclassified as a missing daemon. `Starting` is treated as fully healthy
because the command exits successfully.

Official behavior:
<https://github.com/tailscale/tailscale/blob/05a91829316e055517a1e84f7b00016846ef4107/cmd/tailscale/cli/status.go#L260-L278>

Required correction:

- Read `tailscale status --json`.
- Separate command or connection failure from `.BackendState`.
- Handle every known state explicitly.
- Treat unknown states as unknown, not "daemon missing."
- Add fake-binary tests for every state and connection failure.

### 2. MagicDNS acceptance currently fails

Live results on 2026-07-10:

- Tailscale version: 1.98.8.
- Backend state: `Running`.
- `AcceptDNS=true`.
- A peer's MagicDNS name resolves through a direct query to `100.100.100.100`.
- The same name fails through the macOS system resolver.
- `scutil --dns` contains no `100.100.100.100` resolver.

The Tailscale Domain Name System (DNS) service is healthy. The macOS resolver integration is the failing
layer.

This fails the explicit at-home acceptance criterion in
`docs/superpowers/plans/2026-06-23-tailscaled-headless-daemon.md:197-206`. Current `CLAUDE.md` describes
only possible foreign-network flakiness, but the failure happens at home now.

Before S5 is accepted, choose one:

1. Fix the supported macOS resolver integration.
1. Explicitly accept degraded headless behavior and document a fallback such as `/etc/hosts` or direct
   names and IP addresses.

Do not configure `100.100.100.100` as a permanent global resolver. That would break normal DNS when
off-tailnet.

### 3. Record the superseded service decision

The old Tailscale spec and plan still mandate `sudo brew services` and reject `install-system-daemon`.
Add a dated execution note explaining the root-ownership discovery and commit `01d15ad`.

### 4. Split unrelated scope

Move commit `35922d4`, the `test/update-skills-launchagent-path.sh` change, into the skills stabilization
PR.

### 5. Clean evergreen documentation

Fix these claims in `CLAUDE.md`:

- Remove "S6 will automate it weekly."
- Replace the "3-6 months" forecast with a role-based policy.
- Replace "never re-authenticates" with "node-key expiry will not force reauthentication."
- Use the full formula path for `install-system-daemon`.
- Keep the explanation of copied-daemon upgrades, but avoid documenting unimplemented S6 behavior.

## Roadmap and SP2 plan corrections

### Progress tracking is stale

Add one status table with phase or slice, state, PR, merge commit, and remaining follow-up. Mark Phase A
and S1-S4 complete. Mark S5 active.

### S3 describes an obsolete architecture

Replace the old 21-skill counts and file names with PR #36's 31-skill provenance model. Keep only
verified residual debt in Phase E.

### The roadmap still treats Nushell as active

Record the 2026-07-09 operator-ratified no-go. Update the sub-project table, issue #5, P8, the deferred
index, and SP3's shell seam. SP4 is now the Bash-improvement successor.

### SP3 is called fully designed while contract items remain

Change its status to "behavior contract approved; final implementation spec pending." Remove the
already-resolved UDR probe from the open list and add the R7 native-push decisions.

### SP5 is both standalone and part of S11

Keep Thaw as a standalone SP5 PR.

### SP-nix is missing from the sequence table

Add it as conditional research with an explicit start trigger.

### OpenClaw is simultaneously dropped and documented

Decide whether to keep or remove it. If removing it, handle the package, F1 binding, docs, and task
cleanup together.

### The Tailscale decision history is contradictory

Mark the June Homebrew-service decision as superseded during execution.

### S12 ordering is contradictory

Make S12 unambiguously pre-cutover after all implementation PRs.

## Cutover defects

### Replace the empty-diff gate

The plan permits improvements over the integration branch but also requires an empty final diff. Both
cannot be true.

Replace the empty-diff requirement with an expected-delta ledger. Classify every reference-branch hunk
as:

- Landed unchanged.
- Intentionally improved.
- Deliberately omitted, with a reason.
- Missing.

Only "missing" blocks cutover.

### Track the reconciliation tooling

The plan relies on `.superpowers/sdd/live-reconcile-skills.sh`, but `.superpowers/` is ignored and the
script is not tracked.

Move durable reconciliation into `scripts/` or a managed executable. It must support dry-run, be
idempotent, and have tests.

### Put Phase E into the dependency graph

Phase E says every item must finish before SP2 closes, but several items happen after apply and the
current D1 closes the reference PRs immediately.

Split D1 into:

1. Preflight.
1. Staged activation.
1. Live reconciliation.
1. Soak.
1. Final closure.

Attach every Phase E item to one of those gates.

### Add operational safety

Before switching the live source to `main`:

- Classify every dirty or untracked file in the primary worktree.
- Back up uncaptured Hermes profile state using the repository backup convention.
- Capture current LaunchAgent and service state.
- Keep the integration branch and previous deployed files available for rollback.
- Keep a second remote session open.
- Apply in stages.
- Verify remote reachability before ending the original session.
- Do not close PRs #25, #31, or #32 until the soak passes.

### Explicitly retire old services

The cutover needs a before-and-after LaunchAgent inventory and managed retirement of services whose
source files were deleted.

## Remaining slice gaps

### S6: weekly Homebrew upgrades

- Depend explicitly on S5's copied-daemon model.
- Re-copy Tailscale only when the formula binary changes.
- Add mutual exclusion so two upgrade runs cannot overlap.
- Continue later steps after an individual failure, but return an aggregate failure status.
- Test missing tools, partial failures, logging, loader rendering, and Tailscale refresh failure.
- Split package-runner refactoring if it makes S6 too large.

### S7: relay notifications

The plan knowingly ships bugs that can drop notifications:

- Missing human interface device idle data aborts all channels instead of failing open.
- A partially written final transcript line can discard the whole summary.
- A stale directory lock can suppress later notifications.
- A missing flag value can abort parsing.

Classify defects into delivery blockers and harmless baseline quirks. Fix delivery-loss defects before
merging S7. Add characterization tests for anything deliberately retained. SP3 can still replace the Bash
design later.

### S8: Hermes age encryption

- Do not remove the Darwin guard without a complete Linux credential-source and identity design.
- The current macOS paths and KeePassXC assumptions do not constitute Linux support.
- Replace the destructive `chezmoi forget` plus `add` rotation sequence with the installed
  `chezmoi re-add --re-encrypt` workflow.
- Enumerate managed encrypted targets explicitly.
- Rehearse rotation in a scratch source and destination.
- Re-scope S8 before work because encrypted per-profile Hermes configs and codegraph state materially
  expand it.
- Round-trip test each captured profile independently.

### S9: osquery

- Build an exact path and hunk matrix before implementation.
- Add S5 as a dependency because the Tailscale monitor moved into S9.
- Render and parse every plist.
- Test every loader label and path.
- Split dispatch, alerter, pollers, and pack changes if the real diff is not quickly reviewable.
- Keep query and pack changes behind the existing operator sign-off gate.

### S10: macOS defaults and SSH

- Perform SSH hardening only while physically present.
- Define the exact accepted `sshd -T` output before implementation.
- Validate syntax and effective configuration before reload.
- Keep an existing session open.
- Prove a new key-only session works.
- Test rollback before closing the original session.
- Do not leave "address UsePAM's interaction" as an undefined requirement.

### S11: long-tail chores

S11 is several unrelated PRs. Split it into:

1. Shell and brew-cache work.
1. Secret permission changes.
1. Git hygiene.
1. Desktop and hotkey cleanup.
1. Log rotation.
1. Fork maintenance.
1. Plugin installation.

Keep Thaw as standalone SP5. Decide OpenClaw separately.

### S12: global instructions

- Complete S12 before cutover.
- Build the shared Claude and Codex rules partial.
- Render both global targets in tests.
- Compare the shared block byte-for-byte.
- Re-check every command and path against the converged `main`.
- Move conditional operational detail into runbooks instead of the always-loaded instruction files.

## Deferred sub-projects

### SP3: Rust notifications

Write the final spec before Rust implementation. It still needs:

- Event input schemas.
- Per-harness event mapping.
- Native-push ownership.
- Per-channel retry and failure behavior.
- Presence thresholds.
- Lights quiet hours.
- Migration coexistence and rollback.
- Acceptance boundaries.

The design currently describes a stateless per-event executable, not a daemon. Stop calling it a service
unless a daemon is explicitly approved.

### SP4: Bash improvements

Update the roadmap to reflect the Nushell no-go. Run a separate brainstorm, spec, and plan after SP3.

### SP5: Thaw

Ship one small standalone install and manifest PR during SP2.

### SP6: Neovim

At audit time, `nvim-overhaul` was 69 commits behind and three ahead of `origin/main`.

Before modernization:

1. Re-check branch state.
1. Back up both repositories.
1. Inventory the live Neovim configuration.
1. Import the live configuration unchanged.
1. Modernize through later reviewable PRs.

### SP7: cleanup backlog

Deduplicate the ledger into tracked tasks with current status, severity, and dependencies. P8 is
unblocked by the Nushell decision. P12 is already on `main`. Close or update obsolete OpenClaw and
issue-tracking work.

### SP-nix

Do not start it merely because it appears in the roadmap. Start only after one of these triggers:

- A larger Mac fleet.
- A material maintenance failure in the current defaults system.
- A proven design that preserves the current single-apply and secrets model.

## Recommended implementation order

1. Repair PR #38 without changing its copied-daemon architecture.
1. Resolve or explicitly accept the MagicDNS failure.
1. Amend the roadmap and SP2 plan.
1. Land the skills stabilization PR.
1. Land the herdr stabilization PR.
1. Land rendered-template coverage and documentation fixes.
1. Implement S6 against the settled Tailscale model.
1. Resolve the S7 delivery-defect policy.
1. Resolve S8's Linux and encrypted-profile boundary.
1. Implement S7 and the re-scoped S8.
1. Re-scope and split S9.
1. Implement S10 during a physical-presence window.
1. Split S11 and ship SP5 separately.
1. Complete and mechanically verify S12.
1. Run cutover preflight and expected-delta reconciliation.
1. Activate `main` in stages.
1. Run tracked live reconciliation.
1. Soak before closing the reference PRs.
1. Continue with SP3, SP4, SP6, then SP7.
1. Start SP-nix only if its trigger occurs.
