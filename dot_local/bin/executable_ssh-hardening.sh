#!/bin/bash
# ssh-hardening.sh -- lock sshd to public-key authentication via a drop-in,
# closing the PAM password channel and denying root login.
#
# The drop-in file IS the lock: leave it in place permanently. Without it, sshd
# reverts to its default of allowing password auth.
#
# The drop-in is named 000-ssh-hardening.conf so it sorts FIRST under sshd's
# `Include /etc/ssh/sshd_config.d/*`. That Include is lexical and FIRST-VALUE-WINS,
# so a file that sorts before ours (e.g. Apple's 100-macos.conf, or any future
# sibling) would SHADOW the hardening. Sorting first (000- before any 0*/1* file)
# guarantees our values win. The old name (50-no-password-auth.conf) sorted AFTER
# 100-macos.conf and was defeated; install migrates it away.
#
# Modes:
#   (default)         write the drop-in if missing or stale (idempotent; needs
#                     sudo), migrate the old 50- name, then VERIFY the full
#                     effective config is hardened. Does NOT reload sshd (--reload).
#   --print-config    print the drop-in content to stdout and exit. No sudo, no
#                     writes, no sshd: the pure inspection and test seam.
#   --print-path      print the managed drop-in's absolute path and exit.
#   --verify          parse the FULL effective sshd config (main config + every
#                     drop-in) with `sshd -G` and assert all five hardening values
#                     are in force. Read-only: no reload, no writes. Nonzero if the
#                     hardening is shadowed by a sibling drop-in.
#   --reload          reload sshd so a running daemon picks up the drop-in. This
#                     is the disruptive, operator-controlled step: it validates the
#                     complete config first and fails closed, but a reload can drop
#                     the current SSH session, so run it deliberately from a local
#                     console (or with a second session open) and prove key auth
#                     works in a NEW session before closing the old one.
#
# On a fresh Mac the drop-in applies the moment Remote Login is first enabled
# (sshd starts and reads it), so no reload is needed there.
set -euo pipefail

# Overridable for tests, which target a sandbox tree and drop the sudo prefix.
SSHD_CONFIG_D="${SSHD_CONFIG_D:-/etc/ssh/sshd_config.d}"
DROPIN="$SSHD_CONFIG_D/000-ssh-hardening.conf"
# The superseded name (sorted AFTER 100-macos.conf, so it was shadowed). Migrated
# away on install.
LEGACY_DROPIN="$SSHD_CONFIG_D/50-no-password-auth.conf"

SSHD_BIN="${SSHD_BIN:-sshd}"
SSHD_MAIN_CONFIG="${SSHD_MAIN_CONFIG:-/etc/ssh/sshd_config}"
# Privilege prefix for live-system reads/writes/reloads. Default sudo; tests set
# SSH_HARDENING_SUDO="" to operate unprivileged against a sandbox tree.
SUDO="${SSH_HARDENING_SUDO-sudo}"

# The accepted effective values (the hardening ruling), exactly as `sshd -G` /
# `sshd -T` print them (lowercased key, single space).
ACCEPTED_EFFECTIVE=(
  'passwordauthentication no'
  'kbdinteractiveauthentication no'
  'usepam yes'
  'pubkeyauthentication yes'
  'permitrootlogin no'
)

# priv <cmd...> -- run a command with the configured privilege prefix (sudo by
# default; nothing when SSH_HARDENING_SUDO is empty, e.g. under test).
priv() {
  if [[ -n $SUDO ]]; then
    "$SUDO" "$@"
  else
    "$@"
  fi
}

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

# assert_effective_config <effective-config-text> -- return 0 iff ALL five accepted
# values are present. On any mismatch, print a loud per-key WARNING to stderr and
# return 1. Pure: no sshd, no sudo, no writes -- the unit seam.
assert_effective_config() {
  local effective="$1" rc=0 pair key actual
  for pair in "${ACCEPTED_EFFECTIVE[@]}"; do
    if ! grep -qxiF "$pair" <<<"$effective"; then
      key="${pair%% *}"
      actual="$(grep -iE "^${key} " <<<"$effective" | head -n1 || true)"
      printf '[ssh-hardening] WARNING: effective sshd config has "%s" but hardening requires "%s"\n' \
        "${actual:-(no ${key} line)}" "$pair" >&2
      rc=1
    fi
  done
  return "$rc"
}

# verify_effective -- assert the FULL effective sshd config (main config + every
# drop-in, in sshd's own Include precedence) resolves to the five accepted values.
# Uses `sshd -G` (read-only, host-key-free: never reloads, never binds). Root is
# needed to read the mode-0600 managed drop-in, so it runs under the privilege
# prefix. When sshd is absent it cannot verify: it says so and returns 0 (macOS
# always ships sshd, so that path is only hit off-target, e.g. Linux CI).
verify_effective() {
  if ! command -v "$SSHD_BIN" >/dev/null 2>&1; then
    printf '[ssh-hardening] NOTE: %s not found; cannot verify the effective config here. On macOS verify with: sudo sshd -T | grep -iE "passwordauthentication|kbdinteractiveauthentication|usepam|pubkeyauthentication|permitrootlogin"\n' \
      "$SSHD_BIN" >&2
    return 0
  fi
  local effective
  if ! effective="$(priv "$SSHD_BIN" -G -f "$SSHD_MAIN_CONFIG" 2>/dev/null)"; then
    printf '[ssh-hardening] WARNING: could not parse the effective sshd config (%s -G -f %s failed); NOT claiming this host is hardened.\n' \
      "$SSHD_BIN" "$SSHD_MAIN_CONFIG" >&2
    return 1
  fi
  if assert_effective_config "$effective"; then
    printf '[ssh-hardening] verified: all five hardening values are in force in the effective config\n'
    return 0
  fi
  return 1
}

# install_dropin -- write the drop-in when missing or stale (idempotent), migrate
# the superseded 50- name, then verify the full effective config is hardened.
install_dropin() {
  local desired current
  desired="$(render_dropin)"
  current=""
  [[ -f $DROPIN ]] && current="$(priv cat "$DROPIN" 2>/dev/null || true)"
  if [[ $current != "$desired" ]]; then
    render_dropin | priv tee "$DROPIN" >/dev/null
    printf '[ssh-hardening] wrote %s\n' "$DROPIN"
  else
    printf '[ssh-hardening] %s already current\n' "$DROPIN"
  fi

  # Migrate the superseded 50- drop-in: it sorted AFTER 100-macos.conf and so was
  # shadowed. Remove it in the same privileged step so no orphan/duplicate lingers.
  if [[ -e $LEGACY_DROPIN ]]; then
    priv rm -f "$LEGACY_DROPIN"
    printf '[ssh-hardening] removed superseded drop-in %s\n' "$LEGACY_DROPIN"
  fi

  # Defense in depth: the drop-in must WIN over every sibling (e.g. a future hostile
  # 100-macos.conf). Refuse to claim success if any of the five is not accepted.
  if ! verify_effective; then
    printf '[ssh-hardening] ERROR: the drop-in is in place but the effective sshd config is NOT fully hardened -- a sibling drop-in is overriding it. Resolve before relying on this host.\n' >&2
    return 1
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
  --print-path)
    printf '%s\n' "$DROPIN"
    ;;
  --verify)
    verify_effective
    ;;
  --reload)
    do_reload
    ;;
  "")
    install_dropin
    ;;
  *)
    printf 'usage: ssh-hardening.sh [--print-config | --print-path | --verify | --reload]\n' >&2
    exit 2
    ;;
esac
