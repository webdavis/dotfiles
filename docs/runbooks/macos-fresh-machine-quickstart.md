# macOS Fresh-Machine Quickstart

A checklist for everything that `chezmoi apply` can't (or shouldn't) automate. Read top-to-bottom on a
brand-new Mac before running `chezmoi apply` for the first time.

## Before first `chezmoi apply`

1. **Install Xcode Command Line Tools**: `xcode-select --install`. Required for git and brew.
1. **Sign into Apple ID**: System Settings → Apple ID. Required for iCloud Drive (KeePassXC db sync) and
   `mas` App Store installs.
1. **Retrieve the KeePassXC database**: from offline backup or iCloud Drive. Place at the path referenced
   in `.chezmoi.toml.tmpl`.
1. **Install chezmoi**: `brew install chezmoi` (or pre-install via homebrew bootstrap).
1. **Initialize chezmoi**: `chezmoi init <repo-url>`. This will require the KeePassXC db to be reachable
   for any KeePassXC-templated files.

## During `chezmoi apply`

The Tier 2 runner (`run_onchange_after_41-macos-system-setup.sh.tmpl`) will prompt once for sudo if the
system_setup YAML is non-empty. Enter your password.

## After first `chezmoi apply`

These steps require GUI interaction or interactive auth. There's no `defaults` equivalent.

### Aerospace compatibility

- **System Settings → Desktop & Dock → Mission Control → Displays have separate Spaces**: set per
  machine: ON for tri-monitor, OFF for single-monitor.
- **System Settings → Desktop & Dock → Click wallpaper to reveal desktop**: set to "Only in Stage
  Manager" (the `defaults` key changes name across Sequoia point releases, so manual is more durable).

### TCC privacy grants

System Settings → Privacy & Security → grant the following:

- **Full Disk Access**: Ghostty, Karabiner-Elements, Hammerspoon.
- **Screen Recording**: any tool you use that needs it (Loom, Zoom, OBS).
- **Accessibility**: Karabiner-Elements, Rectangle, any keyboard-remap tools.
- **Input Monitoring**: Karabiner-Elements.

Each grant requires opening the Privacy sheet and dragging the app into the listed sheet. There's no CLI
surface.

### OverSight notification delivery

OverSight (the Objective-See microphone and camera activation monitor, installed as a cask) alerts
through Notification Center, and that alert is its ONLY output: with delivery denied the monitor still
observes correctly and tells nobody, so a dead alert channel reads as healthy. The authorization is
interactive-only; no supported command line writes it:

1. Launch OverSight once (`open -a OverSight`). First launch registers it with Notification Center and
   prompts to allow notifications; click Allow.
1. If the prompt was dismissed or denied: System Settings → Notifications → OverSight → turn on Allow
   notifications, and pick the Alerts style so an activation that fires while you are away stays on
   screen (banners dismiss themselves).
1. Grant nothing under Privacy & Security. OverSight watches device activation events (CoreMediaIO
   property listeners), never microphone or camera content; the installed 2.4.0 bundle declares no
   usage-description keys and no entitlements, so macOS never lists it under Microphone or Camera and
   no such grant exists to give.
1. Confirm the monitor is running: `pgrep -x OverSight` prints a PID. The security-posture poller
   verifies this continuously (the `oversight` record in `.chezmoidata/macos_posture_controls.yaml`) and
   pages if the process stops.

### Firewall log diagnostics

Nothing to enable here, this section is view-only. Firewall logging is on by default on macOS 26.2 and
its activity flows to the unified log automatically. `socketfilterfw` has no logging flag (older releases
had `--setloggingmode`; 26.2 lists none in `-h` or its man page), the Firewall pane in System Settings
exposes no logging toggle, and `/var/log/appfirewall.log` no longer exists. To view the activity:

```bash
# Watch firewall activity live:
log stream --predicate 'process == "socketfilterfw"' --info

# Review recent history:
log show --last 1h --predicate 'process == "socketfilterfw"' --info
```

Do not resurrect the legacy `defaults write /Library/Preferences/com.apple.alf loggingenabled` toggle:
nothing on 26.2 documents that the daemon still reads it, and a preference written under a subsystem that
ignores it reads back as configured while doing nothing.

### SSH hardening: reload and the way back in

`ssh-hardening.sh` writes `/etc/ssh/sshd_config.d/000-ssh-hardening.conf` (public-key-only sshd policy)
and verifies it, but never restarts sshd. The running daemon picks the drop-in up only via the separate,
deliberately disruptive `ssh-hardening.sh --reload`, which validates the complete configuration first,
restarts the launchd service, and refuses to report a restart as successful until an SSH banner exchange
completes on the resolved port. One documented exception: when Remote Login is off (the launchd service
is confirmed absent), `--reload` exits 0 as a clean no-op, with no restart and no banner; the drop-in
applies when Remote Login is next enabled.

Before running `--reload` on a machine you are not sitting at:

1. Above all, keep any SSH session you still have OPEN until a new login succeeds.
1. Confirm Screen Sharing over the tailnet works BEFORE the reload, not during the incident.

If `--reload` fails, warns about a possible lockout, or a new session cannot connect: from the physical
console or Screen Sharing over the tailnet, run `ssh-hardening.sh --rollback` (or
`sudo rm /etc/ssh/sshd_config.d/000-ssh-hardening.conf`), then turn Remote Login off and back on in
System Settings > General > Sharing. `--rollback` removes the drop-in and confirms the hardening is out
of the effective configuration; the next sshd start accepts password authentication again.

### Hardware pairing

- **Bluetooth**: pair AirPods, mice, keyboards via System Settings → Bluetooth.
- **Wi-Fi profiles / 802.1X**: connect to your network; the password / cert flow is interactive.
- **Touch ID**: enroll fingerprints via System Settings → Touch ID & Password.

### App authentication

- **Browser sign-ins**: 1Password browser extension, GitHub, work accounts.
- **App Store apps requiring purchase confirmation**: after `mas install <id>`, confirm purchase in the
  modal that appears.

### Login Items

System Settings → General → Login Items → add anything not covered by an installed-app's preferences
(launchd is generally the better path; this is a fallback).

### Out-of-scope items (by design)

The following are intentionally NOT tracked in the YAML:

- **Karabiner-Elements rules**: managed by Karabiner's own JSON in `dot_config/private_karabiner/`.
- **SIP-protected toggles** (`nvram`, `csrutil`): recovery-mode only.
- **Hot Corners / Mission Control assignments**: `defaults` keys vary by macOS major version; punt to v2.
- **Per-app keyboard shortcuts** (`NSGlobalDomain NSUserKeyEquivalents`): arrays-of-dicts not supported
  by v1 schema; punt to v2.

## Sanity checks after setup is complete

```bash
# Aerospace required default
defaults read com.apple.dock mru-spaces  # expect 0

# All tracked defaults match YAML
just D  # expect exit 0, no output

# Aerospace itself running
pgrep -x AeroSpace  # expect a PID
```
