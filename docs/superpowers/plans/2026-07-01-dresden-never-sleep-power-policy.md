# Plan: never-sleep power policy for dresden (`disablesleep 1` + `tcpkeepalive 1`)

Status: ready to implement, 2026-07-01. Host: **dresden** (MacBook, macOS 26.2).

## Goal

Make dresden **never sleep** (any power source, lid open or closed) as a *tracked, reproducible*
policy, so it is reachable over Tailscale (SSH from phone) 24/7. Also keep TCP connections alive across
sleep as dormant insurance.

## Why

- **Remote access is the driver.** A sleeping Mac suspends `tailscaled`; Tailscale has no cloud proxy to
  wake it, and Wake-on-LAN (L2) can't be sent over Tailscale (L3) without an always-on helper box on the
  home LAN. The only reliable way to SSH into dresden anytime is to **keep it awake**.
- **The one-switch constraint.** `disablesleep` (aka the system-wide `SleepDisabled` shown in `pmset -g`)
  is the *only* thing that keeps a MacBook awake with the lid closed and no external display. It is a
  single system-wide switch, no `-c`/`-b` (AC/battery) variant, so "never sleep everywhere" is exactly
  the case it expresses cleanly, with **no daemon, no lid/timer logic**. (`caffeinate`-style assertions,
  incl. the Claude/Codex "keep awake" toggles, are all defeated by closing the lid; only `disablesleep`
  survives a closed lid.)

## Current state (as of 2026-07-01, for context)

- `pmset -g` shows `SleepDisabled 1`, set **manually**, not tracked in chezmoi. A reprovision would not
  restore it.
- A **persistent `caffeinate -im`** ("asserting forever", observed pid 5658) is spawned by the **Happy
  daemon** (`happy daemon start-sync`, supervised by the chezmoi-managed `com.webdavis.happy-daemon`
  LaunchAgent). It is **NOT an orphan**, do not kill it (the daemon owns and respawns it). It becomes a
  redundant no-op under `disablesleep 1`, but is harmless.
- A transient `caffeinate -i -t 300` is Claude Code's own keep-awake (parent `claude --remote-control`);
  it auto-releases after 5 min. Leave it.
- `tcpkeepalive` is already `1` live, but untracked.
- So dresden already never sleeps, but only via manual/temporary mechanisms (a hand-set
  `SleepDisabled=1` plus several app/daemon keep-awake assertions). This plan makes it durable.

## Accepted trade-off

On battery, dresden will **never** idle-sleep either, unplug and walk away and it will drain flat. This
is the deliberate choice for dead-simple, always-reachable behavior.

## Changes

### 1. Track the pmset settings (Tier-2 sudo runner)

Append two records to the `macos.system_setup:` array in
`.chezmoidata/macos_system_setup.yaml`. Schema is `description` / `command` (bash, **no** `sudo` prefix)
/ `sudo: true`. Both commands are idempotent (setting a pmset value to its current value is a no-op),
satisfying the runner's documented idempotency requirement.

```yaml
    - description: "Never sleep, any power source, lid open or closed"
      command: "pmset -a disablesleep 1"
      sudo: true
    - description: "Keep TCP connections alive through sleep/standby"
      command: "pmset -a tcpkeepalive 1"
      sudo: true
```

- `disablesleep 1` is the load-bearing setting (never sleeps → remote SSH works, lid open or closed).
- `tcpkeepalive 1` is a deliberate no-op while `disablesleep 1` holds, kept only so connections would
  survive automatically if dresden is ever allowed to sleep again.
- **Do NOT** add these to the Tier-1 `defaults write` runner
  (`run_onchange_after_30-macos-defaults`): `pmset` writes root-only system power prefs, so it belongs in
  the sudo tier.

These are applied by `.chezmoiscripts/run_onchange_after_41-macos-system-setup.sh.tmpl`, which re-fires on
the YAML hash and wraps the batch in one upfront `sudo -v` (`sudo` is passwordless on dresden, so no
prompt). No new script or LaunchAgent is needed.

### 2. Keep-awake processes: nothing to purge

Under `disablesleep 1`, every other keep-awake mechanism becomes a harmless redundant no-op. Do **not**
kill any of them:

- The persistent `caffeinate -im` is owned by the **Happy daemon** (`com.webdavis.happy-daemon`), killing
  it would fight your own remote-control bridge and it would respawn. Leave it.
- The transient `caffeinate -i -t 300` is Claude Code's own; it auto-releases. Leave it.
- The Claude Desktop and Codex Desktop "keep awake" toggles are also redundant under `disablesleep 1`.
  They are per-app UI state (not chezmoi-managed) and may be turned off manually for tidiness, optional,
  and best done *after* this plan is applied and `SleepDisabled=1` is verified, so coverage never drops.

The manual `SleepDisabled=1` currently on the box gets reconciled by the tracked setting on the next
apply, no action needed.

> Verify before killing anything: `pmset -g assertions | grep -i caffeinate`, then trace each pid's
> parent (`ps -o ppid= -p <pid>`). A `caffeinate` under a launchd daemon is intentional, not cruft.

### 3. Unchanged: screen lock

No change. Verified on dresden: lock delay `immediate`, screensaver idle 20 min. Closing the lid turns
the display off → locks the screen on both AC and battery, independent of the sleep policy.

## Verification

1. **Lint/render:** `just l` (renders the Tier-2 runner template via `chezmoi execute-template` →
   `shellcheck -`; `shfmt -i 2 -ci -s`; `yq` for the YAML).
2. **Tests:** `just test` (also the pre-commit gate).
3. **Apply (interactive terminal, Tier-2 runner does `sudo -v`):** `chezmoi apply`. Do **not** run bare
   `chezmoi apply` from automation per repo policy; run it from an interactive terminal.
4. **Confirm settings:** `pmset -g | grep -i sleepdisabled` → `1`; `pmset -g | grep tcpkeepalive` → `1`.
5. **Behavior spot-check:** on battery, lid open, idle past the old timer → does **not** sleep
   (`pmset -g log | grep -e Sleep -e Wake | tail`).
6. **Remote access:** from another tailnet device, `ssh dresden true` keeps succeeding after the lid is
   closed on power.
7. **Durability/idempotency:** re-run `chezmoi apply` → no diff; the settings are now source-controlled.

Note: the Happy daemon's `caffeinate` will still appear in `pmset -g assertions` after applying, that is
expected and fine (redundant under `disablesleep 1`); it is not something to remove.

## Commit

Single logical change. Conventional Commits, e.g.:
`feat(macos): pin dresden to never-sleep (disablesleep + tcpkeepalive) for 24/7 Tailscale access`
(no `Co-Authored-By` / Claude trailer per repo policy).
