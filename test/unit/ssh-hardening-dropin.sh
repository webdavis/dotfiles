#!/usr/bin/env bash
# ssh-hardening-dropin.sh -- ssh-hardening.sh must emit an sshd drop-in that closes
# the PAM password hole. The audit found the old drop-in set only
# `PasswordAuthentication no`; with `UsePAM yes` and the default
# `KbdInteractiveAuthentication`, PAM keyboard-interactive password login stays
# OPEN. The accepted effective config (the ruling) is exactly:
#   passwordauthentication no
#   kbdinteractiveauthentication no
#   usepam yes            (macOS REQUIRES UsePAM yes for account/session mgmt;
#                          safe because BOTH password paths above are no)
#   pubkeyauthentication yes
#   permitrootlogin no
#
# The script's `--print-config` mode prints the drop-in to stdout with NO side
# effects (no sudo, no writes, no sshd) -- the pure inspection seam. This unit test
# greps that output for each key at its accepted value and rejects any conflicting
# line. sshd's own effective-config validation lives in the integration camp
# (needs the sshd binary).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_ssh-hardening.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -f $SCRIPT ]] || fail "missing script: $SCRIPT"

# --print-config must succeed with no sudo and no writes.
config="$(bash "$SCRIPT" --print-config)" || fail "ssh-hardening.sh --print-config exited non-zero"

# Each accepted key=value must be present as its own exact line.
declare -a want=(
  'PasswordAuthentication no'
  'KbdInteractiveAuthentication no'
  'UsePAM yes'
  'PubkeyAuthentication yes'
  'PermitRootLogin no'
)
for line in "${want[@]}"; do
  grep -qxF "$line" <<<"$config" ||
    fail "drop-in is missing the accepted line: '$line' (got:
$config)"
done

# No conflicting directive may reopen a closed path.
declare -a forbid=(
  'PasswordAuthentication yes'
  'KbdInteractiveAuthentication yes'
  'PermitRootLogin yes'
  'PubkeyAuthentication no'
)
for line in "${forbid[@]}"; do
  if grep -qxF "$line" <<<"$config"; then
    fail "drop-in contains a conflicting directive: '$line'"
  fi
done

# --print-path must name a drop-in that sorts FIRST in sshd's lexical Include order,
# AHEAD of Apple's 100-macos.conf. Under `Include sshd_config.d/*`, sshd is
# first-value-wins, so a drop-in that sorts after 100- is silently shadowed (R1-1).
dropin_path="$(bash "$SCRIPT" --print-path)" || fail "ssh-hardening.sh --print-path exited non-zero"
dropin_base="$(basename "$dropin_path")"
[[ $dropin_base == "000-ssh-hardening.conf" ]] ||
  fail "managed drop-in must be 000-ssh-hardening.conf so it sorts first (got: $dropin_base)"
first="$(printf '%s\n100-macos.conf\n' "$dropin_base" | LC_ALL=C sort | head -n1)"
[[ $first == "$dropin_base" ]] ||
  fail "managed drop-in must sort before 100-macos.conf (LC_ALL=C sort put '$first' first)"

printf 'ssh-hardening-dropin: OK (all five accepted keys present, no conflicting directive; 000- sorts first)\n'
