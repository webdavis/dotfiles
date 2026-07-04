# Homebrew weekly-upgrade design

Date: 2026-06-21

## Context

Homebrew upgrades on this machine run **daily and unattended** via the `domt4/autoupdate` tap,
configured in `.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl` as
`autoupdate start 86400 --upgrade --cleanup --immediate --sudo`. Because it runs every 24 hours at an
arbitrary time, it upgrades apps — and surfaces macOS "Open?" / admin-password prompts and app restarts —
**while the operator may be away and unable to act**. On a remotely-accessed always-on host that is a real
problem: an upgrade that restarts a critical app (or demands a click) during an absence is disruptive and,
in the worst case, contributes to lockout.

The goal is to move *all* Homebrew upgrades into a single predictable window — **Monday 12:00**, when the
operator is reliably at the machine — so any restart or prompt happens with someone present, and to do so
without weakening macOS security. This change is **connection-safe**: it touches nothing in the
network / SSH / Tailscale path.

## Goals

- All Homebrew upgrades (formulae, casks, and Mac App Store apps) run on a fixed weekly schedule,
  Monday 12:00 local, when the operator is present.
- Only outdated packages are upgraded (no churn on up-to-date software).
- Upgrades are scoped to the operator's declared/installed package set.
- A readable per-run log of what changed.
- No reduction in macOS security posture (no Gatekeeper/quarantine bypass).

## Non-goals

- Pinning packages to specific versions (Homebrew is rolling-release; out of scope).
- Auto-approving System/Network Extension updates or stripping Gatekeeper quarantine (explicitly
  rejected — keep all macOS security layers; present-time "Open?" clicks Monday noon are acceptable).
- Changing the apply-time `brew bundle` install/cleanup behavior (it already installs **and upgrades** the
  declared set when the package list changes and `chezmoi apply` runs — that is present-time and fine).

## Design

### 1. Remove the daily auto-upgrader

In `.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl`, replace the
`autoupdate start … --upgrade …` block with an idempotent **teardown** (`brew autoupdate stop` +
`brew autoupdate delete`), and remove `domt4/autoupdate` from the `taps:` (and `trusted_taps:` if listed)
keys of `.chezmoidata/system_packages_autoinstall.yaml` so `brew bundle --cleanup` untaps it.

**Ordering gotcha (load-bearing):** the teardown must run **before** `brew bundle --cleanup` untaps the
tap — otherwise the `brew autoupdate` subcommand no longer exists when we call it and the script aborts
under `set -euo pipefail`. So move the teardown to the **top** of the darwin block (before the trust loop
and `brew bundle`), and **guard it on the tap being present** (`brew tap | grep -q '^domt4/autoupdate$'`)
so it is a clean no-op on machines where the tap is already gone or was never installed. The existing
`if ! … autoupdate status | grep -q running` guard is dropped (it would skip the teardown precisely when
autoupdate *is* running, which is the case we must handle).

### 2. Weekly LaunchAgent

New chezmoi-managed user LaunchAgent `Library/LaunchAgents/com.webdavis.homebrew-weekly-upgrade.plist.tmpl`
(rendered to `~/Library/LaunchAgents/`), mirroring `com.webdavis.osquery-uptime-watchdog.plist.tmpl`:

- `Label` = `com.webdavis.homebrew-weekly-upgrade`
- `ProgramArguments` = `[/opt/homebrew/bin/bash, {{ .chezmoi.homeDir }}/.local/bin/homebrew-weekly-upgrade.sh]`
- `StartCalendarInterval` = `{ Weekday = 1; Hour = 12; Minute = 0 }` — **Weekday 1 = Monday**, verified
  against `man launchd.plist` ("0 and 7 are Sunday"). launchd catches up on next wake if the machine was
  asleep at the fire time (a non-issue for an always-on host).
- `RunAtLoad` = `false` — **critical**: loading/bootstrapping the agent must never trigger an upgrade;
  only the calendar schedule does. This is what makes activating it (even while remote) safe.
- `EnvironmentVariables` = `PATH` (`/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin`) and
  `HOME` (`{{ .chezmoi.homeDir }}`) — brew needs both.
- `StandardOutPath` / `StandardErrorPath` = `{{ .chezmoi.homeDir }}/.local/log/homebrew/weekly-upgrade.log`.

### 3. Upgrade helper

New `dot_local/bin/executable_homebrew-weekly-upgrade.sh` (`#!/usr/bin/env bash`, header comment). It
prints to stdout (the LaunchAgent routes that to the log; manual runs show on the terminal). Flow:

1. ISO-8601 timestamp header.
1. `brew update` (refresh metadata).
1. Log `brew outdated` and `mas outdated` — **what is about to change** (this is the useful form of the
   "filter through `brew outdated`" idea: `brew upgrade`/`mas upgrade` are already outdated-only, so the
   list is for visibility, not filtering).
1. `brew upgrade` (formulae + casks; outdated-only; brew already skips self-updating casks).
1. `mas upgrade` (Mac App Store apps; outdated-only).
1. `brew cleanup`.
1. Done footer.

**Resilience:** each step runs independently; a failing step is logged but does **not** abort the
remaining steps, and `brew cleanup` always runs (do not use a bare `set -e` that aborts the whole run on
one package). **Sudo-type casks:** the rare pkg-based cask that needs an admin password cannot upgrade in
the unattended LaunchAgent context (no TTY/askpass); it will be logged for the operator to upgrade by
hand Monday — most casks are app bundles and need no sudo. **No quarantine stripping.**

A convenience `just brew-upgrade` recipe runs the same helper on demand (e.g. to trigger the first upgrade
by hand when the operator is settled, or any ad-hoc upgrade). Like `just test-brew-cache`, it uses the
host brew and runs outside the Nix shell.

### 4. Loader chezmoiscript

New `.chezmoiscripts/run_onchange_after_65-load-homebrew-weekly-upgrade-launchagent.sh.tmpl`, an exact
copy of the atuin loader pattern (`run_onchange_after_40-load-atuin-daemon-launchagent.sh.tmpl`):
`#!/bin/bash`, darwin-gated, a `# plist hash:` gate comment over the plist so it re-runs only when the
plist changes, `mkdir -p ~/.local/log/homebrew`, then `launchctl bootout` + a 3× retry `bootstrap`.

### 5. Lint + docs

- Add the loader `.sh.tmpl` to `find_shell_templates` in `scripts/lint.sh` (the helper `.sh` is
  auto-shellchecked by `find_shell_files`; the `.plist.tmpl` is XML, validated with `plutil -lint` during
  verification, not shellcheck).
- Document under "System Package Management" in `CLAUDE.md`: daily autoupdate removed; the
  `domt4/autoupdate` teardown-before-untap ordering gotcha; the weekly Monday-noon LaunchAgent +
  `mas upgrade`; that upgrades are outdated-only and scoped to the declared set; `Weekday 1 = Monday`.

## Key decisions

- **Monday 12:00, present.** Upgrades (and any restart/prompt) happen only when the operator is at the
  machine — the core fix for the away-time problem.
- **No quarantine stripping.** Keeps every macOS security layer (Gatekeeper, notarization assessment,
  XProtect, runtime code-signature enforcement). The residual "Open?" clicks are present-time
  confirmations, not lockout risks.
- **MAS included.** The weekly job runs `mas upgrade` so Mac App Store apps (7 declared, incl. Xcode and
  DaVinci Resolve) update in the same window. Accepts occasional large downloads at Monday noon.
- **Outdated-only, scoped to declared.** `brew upgrade`/`mas upgrade` only touch outdated software, and
  because `brew bundle --cleanup` keeps installed == declared, `brew upgrade` only ever upgrades the
  declared set (plus dependencies). The declared package *data* is reused indirectly (via what is
  installed), not by coupling the scheduled job to the apply-time `brew bundle` template.
- **Standalone scheduled job, not the system-packages template.** The install/cleanup mechanism is an
  apply-time chezmoi template (renders its Brewfile from `.chezmoidata` when `chezmoi apply` runs); a
  weekly `launchd` job has no chezmoi render, so routing through it would require rendering via chezmoi at
  runtime (coupling) or persisting a Brewfile (a new moving part). A plain helper is simpler and gets the
  same scoping for free.

## Rollout

**Activate now.** Apply so the old daily upgrader is torn down and the new LaunchAgent is loaded; with
`RunAtLoad=false`, no upgrade runs until the first Monday-noon schedule. Stopping the old upgrader
immediately removes the away-time risk during the current remote window.

## Verification (no real upgrade run remotely)

The actual `brew upgrade` is **not** run as a test (it would restart apps); the first real run is the
scheduled Monday-noon one. Verify the plumbing only:

- `plutil -lint` the plist; render + `shellcheck` the helper and the loader
  (`CI=1 chezmoi execute-template --no-tty < <file> | shellcheck -`).
- Confirm `mas upgrade` is actually functional on macOS 26.2 (see Risks) — `mas version` / `mas outdated`
  as a read-only probe.
- After apply: `launchctl print gui/$(id -u)/com.webdavis.homebrew-weekly-upgrade` shows it loaded with
  the Monday-12:00 schedule; `brew autoupdate status` (or `brew tap`) confirms autoupdate/​tap are gone.
- `just l` + `just test`; commit via the normal flow (pre-commit runs lint + tests).

## Risks / caveats

- **`mas upgrade` reliability on macOS 26.** `mas` has had breakage on recent macOS from Apple removing
  private App Store APIs. Verify `mas upgrade` works on 26.2; if it cannot, document that MAS apps fall
  back to the App Store's own auto-update (System Settings → App Store → Automatic Updates) and drop the
  `mas upgrade` step.
- **First-run timing.** With "activate now," the first scheduled run is the upcoming Monday noon, which is
  near the operator's return. Accepted per the rollout decision; `RunAtLoad=false` ensures nothing runs
  before then.
- **Sudo-type casks** are logged, not auto-upgraded (no unattended askpass) — handled by hand when present.
- **Autoupdate-removal ordering** (teardown before untap) — captured above; must not be reordered.

## Files touched

- `.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl` (edit: teardown + remove autoupdate config)
- `.chezmoidata/system_packages_autoinstall.yaml` (edit: remove `domt4/autoupdate` from `taps`/`trusted_taps`)
- `Library/LaunchAgents/com.webdavis.homebrew-weekly-upgrade.plist.tmpl` (new)
- `dot_local/bin/executable_homebrew-weekly-upgrade.sh` (new)
- `.chezmoiscripts/run_onchange_after_65-load-homebrew-weekly-upgrade-launchagent.sh.tmpl` (new)
- `scripts/lint.sh` (edit: add loader to `find_shell_templates`)
- `justfile` (edit: add `brew-upgrade` recipe)
- `CLAUDE.md` (edit: document the mechanism)
