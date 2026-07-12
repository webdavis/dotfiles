#!/bin/bash
# ssh-hardening.sh -- lock sshd to public-key authentication via a drop-in,
# closing the PAM password channel and denying root login.
#
# The drop-in file IS the lock: leave it in place permanently. Without it, sshd
# reverts to its default of allowing password auth.
#
# Modes:
#   (default)         write the drop-in if missing or stale (idempotent; needs
#                     sudo). Does NOT reload sshd (see --reload).
#   --print-config    print the drop-in content to stdout and exit. No sudo, no
#                     writes, no sshd: the pure inspection and test seam.
#   --reload          reload sshd so a running daemon picks up the drop-in. This
#                     is the disruptive, operator-controlled step: a reload can
#                     drop the current SSH session, so run it deliberately from a
#                     local console (or with a second session open) and prove
#                     key auth works in a NEW session before closing the old one.
#
# On a fresh Mac the drop-in applies the moment Remote Login is first enabled
# (sshd starts and reads it), so no reload is needed there.
set -euo pipefail

DROPIN="/etc/ssh/sshd_config.d/50-no-password-auth.conf"

# render_dropin -- print the desired drop-in content. Pure: no sudo, no writes.
# PasswordAuthentication no + KbdInteractiveAuthentication no together close BOTH
# interactive-password channels. UsePAM yes is required on macOS for account and
# session management, and is safe here precisely because neither password channel
# is open, so PAM has no password path to authenticate.
render_dropin() {
  cat <<'EOF'
# Managed by ssh-hardening.sh: lock sshd to public-key authentication only.
PasswordAuthentication no
KbdInteractiveAuthentication no
UsePAM yes
PubkeyAuthentication yes
PermitRootLogin no
EOF
}

# install_dropin -- write the drop-in when missing or stale (idempotent).
install_dropin() {
  local desired current
  desired="$(render_dropin)"
  current=""
  [[ -f $DROPIN ]] && current="$(sudo cat "$DROPIN" 2>/dev/null || true)"
  if [[ $current != "$desired" ]]; then
    render_dropin | sudo tee "$DROPIN" >/dev/null
    printf '[ssh-hardening] wrote %s\n' "$DROPIN"
  else
    printf '[ssh-hardening] %s already current\n' "$DROPIN"
  fi
  printf '[ssh-hardening] drop-in in place; run ssh-hardening.sh --reload (or re-enable Remote Login) to activate it on a running sshd\n'
}

# do_reload -- reload a running sshd via the modern kickstart -k idiom (kill +
# restart in one call). Skips silently when sshd is not loaded: the drop-in stays
# in place and applies whenever Remote Login is next enabled.
do_reload() {
  if sudo launchctl print system/com.openssh.sshd &>/dev/null; then
    sudo launchctl kickstart -k system/com.openssh.sshd
    printf '[ssh-hardening] sshd reloaded\n'
  else
    printf '[ssh-hardening] sshd not currently running; the drop-in applies when Remote Login is next enabled\n'
  fi
}

case "${1:-}" in
  --print-config)
    render_dropin
    ;;
  --reload)
    do_reload
    ;;
  "")
    install_dropin
    ;;
  *)
    printf 'usage: ssh-hardening.sh [--print-config | --reload]\n' >&2
    exit 2
    ;;
esac
