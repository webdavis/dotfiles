#!/usr/bin/env bash
# Install the self-healing nix-installer repair LaunchDaemon, mirroring the
# `systems.determinate.nix-installer.nix-hook` pattern from Determinate's
# installer but pointed at the NixOS-maintained fork's binary (same path:
# `/nix/nix-installer`).
#
# Invoked from .chezmoidata/macos_system_setup.yaml via the tier-2
# (sudo-required) system-setup runner. Idempotent: only writes/bootstraps when
# the plist differs from disk or the daemon isn't loaded.
#
# Why: on macOS the NixOS fork's installer drops a `nix-installer repair`
# subcommand that fixes shell-profile and remote-building integration after
# system upgrades, but does not auto-install a LaunchDaemon to run it. This
# script reproduces Determinate's hook so reboots self-heal the Nix install.

set -euo pipefail

PLIST_PATH=/Library/LaunchDaemons/systems.nixos.nix-installer.nix-hook.plist
LABEL=systems.nixos.nix-installer.nix-hook

EXPECTED=$(
  cat <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>systems.nixos.nix-installer.nix-hook</string>
	<key>ProgramArguments</key>
	<array>
		<string>/bin/sh</string>
		<string>-c</string>
		<string>/bin/wait4path /nix/nix-installer &amp;&amp; /nix/nix-installer repair</string>
	</array>
	<key>KeepAlive</key>
	<dict>
		<key>SuccessfulExit</key>
		<false/>
	</dict>
	<key>StandardErrorPath</key>
	<string>/nix/.nix-installer-hook.err.log</string>
	<key>StandardOutPath</key>
	<string>/nix/.nix-installer-hook.out.log</string>
</dict>
</plist>
EOF
)

# Skip silently when /nix/nix-installer doesn't exist yet (fresh machine pre-Nix install).
if [[ ! -x /nix/nix-installer ]]; then
  echo "  /nix/nix-installer not present yet — skipping nix-hook install."
  exit 0
fi

needs_write=1
if [[ -f $PLIST_PATH ]] && diff -q <(printf '%s\n' "$EXPECTED") "$PLIST_PATH" >/dev/null 2>&1; then
  needs_write=0
fi

if [[ $needs_write == 1 ]]; then
  printf '%s\n' "$EXPECTED" >"$PLIST_PATH"
  chown root:wheel "$PLIST_PATH"
  chmod 644 "$PLIST_PATH"
  launchctl bootout "system/$LABEL" 2>/dev/null || true
  launchctl bootstrap system "$PLIST_PATH"
  echo "  Installed $PLIST_PATH"
else
  # Plist matches; ensure it's loaded.
  if ! launchctl print "system/$LABEL" >/dev/null 2>&1; then
    launchctl bootstrap system "$PLIST_PATH"
    echo "  Bootstrapped existing $LABEL"
  fi
fi
