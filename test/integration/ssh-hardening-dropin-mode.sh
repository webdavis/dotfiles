#!/usr/bin/env bash
# ssh-hardening-dropin-mode.sh -- install must pin the managed drop-in to a
# deterministic, non-secret 0644 (R2-5), not leave its mode to root's umask. The audit
# found a comment claiming a "mode-0600 managed drop-in" while install never chmod'd,
# so under root's umask 022 the file was actually 0644. The drop-in holds NO secret and
# sshd needs it readable (like Apple's 100-macos.conf), so the correct, honest state is
# an explicit 0644 -- independent of the ambient umask.
#
# Under a restrictive umask 0077, `tee` alone would create the file 0600; the explicit
# chmod makes it 0644 regardless. This runs install against a SANDBOX drop-in dir (the
# SSHD_CONFIG_D + SSH_HARDENING_SUDO="" seams -- NEVER touches live /etc/ssh, NEVER
# reloads sshd) with umask 0077 and asserts the resulting mode is exactly 644.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_ssh-hardening.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}
[[ -f $SCRIPT ]] || fail "missing script: $SCRIPT"

# GNU-first, BSD-fallback file mode (octal). GNU coreutils uses -c '%a'; BSD uses
# -f '%Lp'. GNU MUST come first: on Linux `stat -f` means "filesystem status" and
# would succeed with the wrong output.
mode_of() { stat -c '%a' "$1" 2>/dev/null || stat -f '%Lp' "$1"; }

work="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$work"' EXIT

confd="$work/sshd_config.d"
mkdir -p "$confd"
main="$work/sshd_config"
printf 'Include %s/*\n' "$confd" >"$main"

# Install under a restrictive umask so a missing chmod would yield 0600 (the RED).
# SSH_HARDENING_SKIP_IF_NO_SSHD=1 lets the tail verify step skip cleanly where sshd is
# absent (the file is written+chmod'd before verify either way); where sshd is present,
# verify runs against this sandbox tree (hardened) and passes.
rc=0
(
  umask 0077
  SSH_HARDENING_SUDO="" SSHD_CONFIG_D="$confd" SSHD_MAIN_CONFIG="$main" \
    SSH_HARDENING_SKIP_IF_NO_SSHD=1 bash "$SCRIPT" >"$work/out" 2>"$work/err"
) || rc=$?

dropin="$confd/000-ssh-hardening.conf"
[[ -f $dropin ]] || fail "install did not write the drop-in (rc=$rc; err: $(cat "$work/err"))"

mode="$(mode_of "$dropin")"
[[ $mode == 644 ]] ||
  fail "managed drop-in must be a deterministic non-secret 0644, got $mode (an explicit chmod must override root's umask; rc=$rc)"

printf 'ssh-hardening-dropin-mode: OK (drop-in is 0644 even under umask 0077)\n'
