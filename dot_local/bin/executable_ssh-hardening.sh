#!/bin/bash
# Disable SSH password authentication via drop-in config and reload sshd.
# Manually invoked (no launchd schedule). Run after enabling Remote Login
# on a fresh Mac to lock the SSH server to public-key auth only.
#
# The drop-in file IS the lock — leave it in place permanently. Without it,
# sshd reverts to its default of allowing password auth.
set -euo pipefail

DROPIN="/etc/ssh/sshd_config.d/50-no-password-auth.conf"

if [[ ! -f $DROPIN ]] || ! sudo grep -q "PasswordAuthentication no" "$DROPIN" 2>/dev/null; then
  echo "PasswordAuthentication no" | sudo tee "$DROPIN" >/dev/null
  echo "[ssh-hardening] Wrote $DROPIN"
fi

# Reload sshd via the modern kickstart -k idiom (kill + restart in one call,
# replaces the deprecated launchctl unload/load pair). Skip silently if sshd
# isn't currently loaded — the drop-in stays in place and will apply
# whenever Remote Login is next enabled.
if sudo launchctl print system/com.openssh.sshd &>/dev/null; then
  sudo launchctl kickstart -k system/com.openssh.sshd
  echo "[ssh-hardening] sshd reloaded"
else
  echo "[ssh-hardening] sshd not currently running — config will apply when Remote Login is enabled"
fi
