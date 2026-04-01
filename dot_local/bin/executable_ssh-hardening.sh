#!/bin/bash
# Disable SSH password authentication via drop-in config
# Run manually to (re)apply SSH hardening on this machine
set -euo pipefail

DROPIN="/etc/ssh/sshd_config.d/50-no-password-auth.conf"

if [[ ! -f $DROPIN ]] || ! sudo grep -q "PasswordAuthentication no" "$DROPIN" 2>/dev/null; then
  echo "PasswordAuthentication no" | sudo tee "$DROPIN" >/dev/null
  echo "[ssh-hardening] Wrote $DROPIN"
fi

# Reload sshd
sudo launchctl unload /System/Library/LaunchDaemons/ssh.plist 2>/dev/null || true
sudo launchctl load /System/Library/LaunchDaemons/ssh.plist 2>/dev/null || true
echo "[ssh-hardening] sshd reloaded"
