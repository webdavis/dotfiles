#!/usr/bin/env bash
# ssh-hardening-sshd-validate.sh -- validate ssh-hardening.sh's drop-in through the
# REAL sshd parser, from its effective-config output. The brief named `sshd -t` /
# `sshd -T`, but both require host keys and root and fail as an unprivileged CI/test
# user ("no hostkeys available"). `sshd -G` parses the config and dumps the
# effective settings WITHOUT host keys or root -- the correct host-key-free seam. It
# also rejects bad syntax (exit != 0), so it doubles as the syntax gate the brief
# asked for.
#
# This test NEVER touches the live /etc/ssh config and NEVER reloads sshd: it feeds
# a temp file (the --print-config output) to `sshd -G` and diffs the effective
# values against the accepted set. Whether the drop-in wins against the live main
# config on the real host is the operator's apply-time `sshd -T` check.
#
# SKIPs cleanly where sshd is absent (the de-homebrewed CI-faithful PATH has no
# /usr/sbin) or where `sshd -G` is unusable in the sandbox.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_ssh-hardening.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -f $SCRIPT ]] || fail "missing script: $SCRIPT"
command -v sshd >/dev/null 2>&1 || {
  printf 'SKIP: sshd not on PATH; cannot validate the drop-in with the real parser\n'
  exit 0
}
# Probe: is `sshd -G` usable unprivileged here? An empty config must parse+dump.
if ! sshd -G -f /dev/null >/dev/null 2>&1; then
  printf 'SKIP: sshd -G is not usable in this environment (older OpenSSH or sandbox)\n'
  exit 0
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

dropin="$work/50-ssh-hardening.conf"
bash "$SCRIPT" --print-config >"$dropin" || fail "--print-config exited non-zero"

# sshd -G rejects bad syntax; a non-zero exit here means the drop-in is malformed.
effective="$work/effective"
sshd -G -f "$dropin" >"$effective" 2>"$work/err" ||
  fail "sshd -G rejected the drop-in (syntax/value error): $(cat "$work/err")"

# sshd -G lowercases keys and prints "key value". Assert each accepted pair.
declare -a want=(
  'passwordauthentication no'
  'kbdinteractiveauthentication no'
  'usepam yes'
  'pubkeyauthentication yes'
  'permitrootlogin no'
)
for pair in "${want[@]}"; do
  grep -qxF "$pair" "$effective" ||
    fail "sshd effective config missing '$pair' (got: $(grep -iE '^(passwordauthentication|kbdinteractiveauthentication|usepam|pubkeyauthentication|permitrootlogin) ' "$effective" | tr '\n' ';'))"
done

printf 'ssh-hardening-sshd-validate: OK (sshd -G accepts the drop-in; all five effective values match the ruling)\n'
