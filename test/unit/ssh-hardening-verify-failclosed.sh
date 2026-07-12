#!/usr/bin/env bash
# ssh-hardening-verify-failclosed.sh -- a verifier that CANNOT run must never report
# success in a production path (R2-3). The audit found: with the sshd binary off PATH,
# `--verify` printed "cannot verify" and returned 0; install_dropin calls --verify
# after writing the drop-in, so an environment without sshd claimed success having
# checked NOTHING. A security check that cannot run must FAIL CLOSED.
#
# The fix uses the absolute `/usr/sbin/sshd` (macOS default) and, when the verifier
# cannot run, returns NONZERO and loud in production. The tool-absence SKIP is
# permitted ONLY via an explicit test env seam (SSH_HARDENING_SKIP_IF_NO_SSHD), never
# in the default production path.
#
# Pure/fast: drives `--verify` with SSHD_BIN pointed at a nonexistent path (no real
# sshd, no config parse -- verify_effective returns at the binary-absence check). Unit
# camp. No SKIP needed.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_ssh-hardening.sh"

# Exercise the script through its PRODUCTION shebang interpreter (/bin/bash). macOS
# ships /bin/bash 3.2, which lacks associative arrays and the compgen builtin, and the
# operator invokes the script by its `#!/bin/bash` shebang -- so a 3.2-only regression
# must fail HERE, not in production. Falls back to the ambient bash where /bin/bash is
# absent (non-macOS).
BASH_BIN=/bin/bash
[[ -x $BASH_BIN ]] || BASH_BIN="bash"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}
[[ -f $SCRIPT ]] || fail "missing script: $SCRIPT"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
missing="$work/no-such-sshd"

failures=0
report() {
  if [[ $1 == ok ]]; then printf '  ok   %s\n' "$2"; else
    printf '  FAIL %s\n' "$2"
    failures=$((failures + 1))
  fi
}

printf 'ssh-hardening --verify fail-closed cases:\n'

# 1. PRODUCTION path (no skip seam), sshd binary absent -> FAIL CLOSED (nonzero, loud).
rc=0
err="$(SSH_HARDENING_SUDO="" SSHD_BIN="$missing" SSHD_MAIN_CONFIG=/dev/null \
  "$BASH_BIN" "$SCRIPT" --verify 2>&1 >/dev/null)" || rc=$?
if [[ $rc -ne 0 ]]; then report ok "production: --verify FAILS closed (rc=$rc)"; else
  report bad "production: --verify must FAIL when the verifier cannot run (got rc=0; out/err: $err)"
fi
if grep -qi 'closed' <<<"$err"; then report ok "production: warns it is failing closed"; else
  report bad "production: must warn it is failing closed (err: $err)"
fi

# 2. TEST seam set, sshd absent -> clean SKIP (rc 0, a skip note, never a bogus pass).
rc=0
out="$(SSH_HARDENING_SUDO="" SSHD_BIN="$missing" SSHD_MAIN_CONFIG=/dev/null \
  SSH_HARDENING_SKIP_IF_NO_SSHD=1 "$BASH_BIN" "$SCRIPT" --verify 2>"$work/e2")" || rc=$?
err2="$(cat "$work/e2")"
if [[ $rc -eq 0 ]]; then report ok "test-seam: --verify skips cleanly (rc=0)"; else
  report bad "test-seam: --verify must skip (rc=0) when the seam is set (rc=$rc; err: $err2)"
fi
if grep -qi 'skip' <<<"$err2$out"; then report ok "test-seam: notes the skip"; else
  report bad "test-seam: must note the skip (out: $out; err: $err2)"
fi
# The skip must NOT masquerade as a verification pass.
if grep -qi 'verified' <<<"$out"; then
  report bad "test-seam: skip must not print a 'verified' claim (out: $out)"
else report ok "test-seam: no false 'verified' claim"; fi

# 3. The absolute default matters: with no SSHD_BIN override, the default must be the
#    absolute /usr/sbin/sshd (macOS default), not a bare PATH name a stripped PATH
#    could silently fail to resolve.
if grep -qF 'SSHD_BIN:-/usr/sbin/sshd' "$SCRIPT"; then
  report ok "default: SSHD_BIN defaults to absolute /usr/sbin/sshd"
else
  report bad "default: SSHD_BIN must default to the absolute /usr/sbin/sshd"
fi

if [[ $failures -gt 0 ]]; then
  printf 'ssh-hardening-verify-failclosed: %d assertion(s) FAILED\n' "$failures" >&2
  exit 1
fi
printf 'ssh-hardening-verify-failclosed: OK (production fails closed; test seam skips cleanly; absolute default)\n'
