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
