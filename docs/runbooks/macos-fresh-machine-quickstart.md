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
   usage-description keys and no entitlements, so macOS never lists it under Microphone or Camera and no
   such grant exists to give.
1. Confirm the monitor is running: `pgrep -x -U "$(id -u)" OverSight` prints a PID. User-scoped (`-U`)
   exactly like the security-posture poller's probe, so another user's OverSight cannot mask a stopped
   one here; the poller verifies this continuously (the `oversight` record in
   `.chezmoidata/macos_posture_controls.yaml`) and pages if the process stops.

### LuLu system extension approval

LuLu (the Objective-See outbound firewall, installed as a cask) filters through a network system
extension that macOS only activates after an interactive security consent. No supported command line
writes that approval:

1. Launch LuLu once (`open -a LuLu`) and follow its prompts.
1. Approve the extension: System Settings → General → Login Items & Extensions → Network Extensions →
   enable LuLu. macOS may also raise a "System Extension Blocked" dialog with an Allow button; allow it.
1. When prompted "LuLu would like to Filter Network Content", click Allow. This is the network-content
   filter consent, separate from the extension approval.
1. Confirm it took: `systemextensionsctl list` shows `com.objective-see.lulu.extension` with
   `[activated enabled]`, and `pgrep -x -U 0 com.objective-see.lulu.extension` prints a PID. The
   security-posture poller verifies the process continuously (the `lulu_extension` record in
   `.chezmoidata/macos_posture_controls.yaml`) and pages if it stops.

### LuLu rule creation

Rules cannot be pre-seeded: `rules.plist` is an NSKeyedArchiver archive of LuLu's private `Rule` class,
not hand-authorable by any supported tool, so every rule is created interactively, by answering LuLu's
prompt when a binary first makes an outbound connection or ahead of time via the app's Rules window (LuLu
menu bar icon → Rules → the plus button, which takes a binary path).

The required rules, from the talker table in `.chezmoidata/macos_posture_controls.yaml`
(`macos.lulu_talkers`):

1. **tailscaled** (`/usr/local/bin/tailscaled`): allow. Slice 8's remote recovery path 2 rides the
   tailnet; blocking this removes it.
1. **The Hermes gateway interpreter**: allow. This is the alerting channel's real egress hop. The gateway
   runs under the venv launcher `~/.hermes/hermes-agent/venv/bin/python`, a symlink into a uv-managed
   CPython; LuLu keys the rule on the RESOLVED binary
   (`readlink -f ~/.hermes/hermes-agent/venv/bin/python`), so create the rule for that resolved path.
   After a python upgrade moves the interpreter, the `lulu_rule_hermes_gateway` control pages and this
   step is repeated for the new path.

Both rules are verified continuously by the security-posture poller as existence-only checks: the archive
is readable enough to prove a rule mentioning the binary exists, but the rule action (allow vs block) is
not recoverable by supported tooling, so the poller does not claim it. When either control pages,
recreate the rule here and the poller re-arms on its own.

Lean narrow, and accept the prompt cost. Do NOT create blanket allow rules for shared interpreters and
clients (`/usr/bin/curl`, `/bin/bash`, `node`, `python3`, `/usr/bin/ssh`): LuLu keys rules on the
executing binary and cannot see which script invoked a shared client, so allowing `curl` allows it for
every process on the machine. Where a talker only reaches the network through a shared client, either
leave it prompting or give that path its own dedicated client. Version-pinned paths (Homebrew Cellar,
`/nix/store`) go stale on every upgrade; expect a one-time prompt after upgrades and answer it narrowly.

The alerter's own `curl` needs NO rule: it POSTs to `http://127.0.0.1:8644` and loopback is kept
unfiltered by the `allowLocalHost` preference (declared enforce-tier in
`.chezmoidata/macos_defaults.yaml`). A curl rule would not protect that hop and would allow every process
on the machine.

### LuLu preference changes

The six LuLu policy records in `.chezmoidata/macos_defaults.yaml` are `tier: verify`: `just D` compares
them against the live base file, and nothing in this repo ever writes that file. That is a finding, not
a gap, grounded in LuLu's source (`LuLu/Extension/Preferences.m`, v4.3.2): the extension loads
`preferences.plist` ONCE at start into an in-memory dictionary, never watches or re-reads it, and writes
that whole dictionary back to disk on every preference change it processes. An external `defaults write`
is therefore invisible to the running extension AND clobbered by its next save. Supporting read-only
evidence from this machine: LuLu rewrote both of its files spontaneously on 2026-07-27 at 13:25:06,
fifteen seconds after a display wake; and on a COPY, `defaults write` converted the XML file to a binary
plist, reset mode 0644 to 0600 (which blinds the unprivileged drift reader), added a quarantine xattr,
and replaced the inode.

To change one of the six declared values:

1. Change it in the LuLu app (menu bar icon → Preferences), which updates the running extension over its
   own channel and persists the file itself.
1. Update the record's `value` in `.chezmoidata/macos_defaults.yaml` to the new intent.
1. Run `just D` and confirm the declaration and the live file agree again.

Promotion gate: return these records to `tier: enforce` only when a LuLu version demonstrably re-reads
an external write to the file (a reload mechanism in its source or release notes, verified by observing
a CONSULTED preference take effect). Until then an enforce tier would claim an enforcement the machine
cannot deliver: the write would land, change nothing, and be silently reverted, the exact silent no-op
the tier model exists to surface.

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
