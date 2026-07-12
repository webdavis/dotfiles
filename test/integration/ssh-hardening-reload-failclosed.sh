#!/usr/bin/env bash
# ssh-hardening-reload-failclosed.sh -- the operator-facing `--reload` must FAIL
# CLOSED (R1-2). The audit found: with sudo stubbed rc!=0, `--reload` returned 0 and
# mis-reported "sshd not running" (every sudo/launchctl probe error fell into the
# benign not-running branch); and it ran `launchctl kickstart -k` (which TERMINATES
# the listener) with NO syntax/effective-value validation first, so a broken sibling
# drop-in could drop the daemon. `--reload` is the command the operator runs during
# the physically-present hardening step, so it must:
#   (a) prime sudo and fail closed if it is unavailable;
#   (b) validate the COMPLETE config BEFORE the disruptive kickstart -- `sshd -t`
#       for syntax AND all five effective values -- and abort loudly if either
#       fails (never restart onto a config that would drop or unharden the daemon);
#   (c) distinguish "service CONFIRMED absent" (launchctl print rc 113) from a probe
#       ERROR (any other nonzero): a sudo/launchctl error is NOT proof the daemon is
#       down, so propagate it, never proceed as if stopped;
#   (d) return NONZERO on any failure;
#   (e) verify the launchd job reloaded after the kickstart (first signal only); and
#   (f) prove sshd is actually READY -- accepting an SSH connection -- not merely a
#       loaded launchd job (R2-2). `launchctl print` returns 0 for a loaded-but-
#       crashed service, so a readiness probe (ssh-keyscan on the listener) must gate
#       the green result; a loaded-but-not-accepting sshd fails loud (remote lockout).
#
# Drives `--reload` through fully-controlled sudo/sshd/launchctl/ssh-keyscan stubs
# (the SSH_HARDENING_SUDO / SSHD_BIN / LAUNCHCTL_BIN / KEYSCAN_BIN seams, mirrored on
# PATH so the UNFIXED script's bare-name calls hit the same stubs). NEVER touches the
# live daemon or /etc/ssh. SKIPs never needed (no real tool required).
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

work="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$work"' EXIT

stub="$work/bin"
mkdir -p "$stub"
: >"$work/main"

# sudo stub: total failure when STUB_SUDO_FAIL=1 (models "operator cannot
# escalate"); otherwise `-v` primes and anything else passes through.
cat >"$stub/sudo" <<'EOF'
#!/bin/bash
[[ ${STUB_SUDO_FAIL:-0} == 1 ]] && {
  printf 'stub sudo: a password is required\n' >&2
  exit 77
}
[[ $1 == -v ]] && exit 0
exec "$@"
EOF

# sshd stub: -t is syntax (fails when STUB_SSHD_T_FAIL=1); -G/-T dump a fixture
# effective config.
cat >"$stub/sshd" <<'EOF'
#!/bin/bash
case "$1" in
  -t)
    [[ ${STUB_SSHD_T_FAIL:-0} == 1 ]] && {
      printf 'stub sshd: bad configuration\n' >&2
      exit 1
    }
    exit 0
    ;;
  -G | -T) cat "${STUB_SSHD_EFFECTIVE:?}" ;;
  *)
    printf 'stub sshd: unexpected %s\n' "$*" >&2
    exit 2
    ;;
esac
EOF

# launchctl stub: `print` returns the n-th rc from a colon list (so pre- and
# post-kickstart probes can differ); `kickstart` logs the call and returns a
# controllable rc.
cat >"$stub/launchctl" <<'EOF'
#!/bin/bash
case "$1" in
  print)
    n=1
    [[ -f ${STUB_LAUNCHCTL_PRINT_COUNT:-} ]] && n=$(($(cat "$STUB_LAUNCHCTL_PRINT_COUNT") + 1))
    printf '%s' "$n" >"${STUB_LAUNCHCTL_PRINT_COUNT:?}"
    rc="$(printf '%s' "${STUB_LAUNCHCTL_PRINT_RCS:-0}" | cut -d: -f"$n")"
    [[ -z $rc ]] && rc=0
    exit "$rc"
    ;;
  kickstart)
    printf 'kickstart %s\n' "$*" >>"${STUB_LAUNCHCTL_KICKLOG:?}"
    exit "${STUB_LAUNCHCTL_KICKSTART_RC:-0}"
    ;;
  *)
    printf 'stub launchctl: unexpected %s\n' "$*" >&2
    exit 2
    ;;
esac
EOF

# ssh-keyscan stub: models the readiness probe. STUB_KEYSCAN_READY=1 -> emit a fake
# host-key line (sshd answered the SSH banner exchange); otherwise emit nothing (the
# listener is not accepting -- a loaded-but-crashed sshd).
cat >"$stub/ssh-keyscan" <<'EOF'
#!/bin/bash
[[ ${STUB_KEYSCAN_READY:-0} == 1 ]] && printf '[127.0.0.1]:22 ssh-ed25519 AAAAFAKEKEYMATERIAL\n'
exit 0
EOF
chmod +x "$stub/sudo" "$stub/sshd" "$stub/launchctl" "$stub/ssh-keyscan"

# The effective config the sshd stub reports. Includes a `port` line so effective_port
# aims the readiness probe (and exercises the extraction).
cat >"$work/eff-hardened" <<'EOF'
port 22
passwordauthentication no
kbdinteractiveauthentication no
usepam yes
pubkeyauthentication yes
permitrootlogin no
EOF
# One value lost (passwordauthentication back on) -> verify must abort the reload.
cat >"$work/eff-lost" <<'EOF'
port 22
passwordauthentication yes
kbdinteractiveauthentication no
usepam yes
pubkeyauthentication yes
permitrootlogin no
EOF

# reload_case <name> <env=val...> -> populates RC, OUT, ERR, KICKED.
reload_case() {
  local name="$1"
  shift
  local kicklog="$work/$name.kick"
  : >"$kicklog"
  local pcount="$work/$name.pcount"
  rm -f "$pcount"
  RC=0
  OUT="$(env \
    SSH_HARDENING_SUDO="$stub/sudo" SSHD_BIN="$stub/sshd" LAUNCHCTL_BIN="$stub/launchctl" \
    KEYSCAN_BIN="$stub/ssh-keyscan" SSH_READY_ATTEMPTS=2 SSH_READY_SLEEP_SECONDS=0 \
    SSHD_MAIN_CONFIG="$work/main" STUB_SSHD_EFFECTIVE="$work/eff-hardened" \
    STUB_LAUNCHCTL_KICKLOG="$kicklog" STUB_LAUNCHCTL_PRINT_COUNT="$pcount" \
    PATH="$stub:$PATH" "$@" \
    "$BASH_BIN" "$SCRIPT" --reload 2>"$work/$name.err")" || RC=$?
  ERR="$(cat "$work/$name.err")"
  KICKED="$(cat "$kicklog")"
}

failures=0
report() {
  if [[ $1 == ok ]]; then printf '  ok   %s\n' "$2"; else
    printf '  FAIL %s\n' "$2"
    failures=$((failures + 1))
  fi
}
rc_nonzero() { if [[ $RC -ne 0 ]]; then report ok "$1: exits nonzero"; else report bad "$1: must exit nonzero (got rc=0; out: $OUT; err: $ERR)"; fi; }
rc_zero() { if [[ $RC -eq 0 ]]; then report ok "$1: exits 0"; else report bad "$1: must exit 0 (got rc=$RC; err: $ERR)"; fi; }
no_kick() { if [[ -z $KICKED ]]; then report ok "$1: did NOT kickstart"; else report bad "$1: must NOT kickstart before validation ($KICKED)"; fi; }
did_kick() { if [[ -n $KICKED ]]; then report ok "$1: kickstarted"; else report bad "$1: expected a kickstart, none logged"; fi; }
err_has() { if grep -qi -- "$2" <<<"$ERR"; then report ok "$1: stderr has '$2'"; else report bad "$1: stderr must mention '$2' (err: $ERR)"; fi; }
err_no() { if grep -qi -- "$2" <<<"$ERR"; then report bad "$1: stderr must NOT mention '$2' (err: $ERR)"; else report ok "$1: no '$2' in stderr"; fi; }
out_has() { if grep -qi -- "$2" <<<"$OUT"; then report ok "$1: stdout has '$2'"; else report bad "$1: stdout must mention '$2' (out: $OUT)"; fi; }

printf 'ssh-hardening --reload fail-closed cases:\n'

# 1. sudo failure: propagate nonzero, do NOT misreport "not running", do NOT kick.
reload_case sudo-fail STUB_SUDO_FAIL=1
rc_nonzero sudo-fail
no_kick sudo-fail
err_no sudo-fail "not loaded"
err_no sudo-fail "not currently running"
err_has sudo-fail "sudo"

# 2. malformed config: sshd -t fails -> abort BEFORE the kickstart.
reload_case malformed STUB_SSHD_T_FAIL=1
rc_nonzero malformed
no_kick malformed
err_has malformed "syntax"

# 3. lost hardening: effective config missing a value -> abort BEFORE the kickstart.
reload_case lost STUB_SSHD_EFFECTIVE="$work/eff-lost"
rc_nonzero lost
no_kick lost
err_has lost "not fully hardened"

# 4. service CONFIRMED absent (launchctl print rc 113): benign, exit 0, no kick.
reload_case absent STUB_LAUNCHCTL_PRINT_RCS=113
rc_zero absent
no_kick absent
out_has absent "not loaded"

# 5. probe ERROR (nonzero but NOT 113): fail closed, do NOT treat as stopped, no kick.
reload_case probe-error STUB_LAUNCHCTL_PRINT_RCS=1
rc_nonzero probe-error
no_kick probe-error
err_has probe-error "could not determine"
err_no probe-error "not loaded"

# 6. kickstart failure: nonzero + loud, kickstart WAS attempted.
reload_case kick-fail STUB_LAUNCHCTL_PRINT_RCS=0:0 STUB_LAUNCHCTL_KICKSTART_RC=1
rc_nonzero kick-fail
did_kick kick-fail
err_has kick-fail "kickstart"

# 7. launchd job did not reload after kickstart: nonzero + loud.
reload_case no-return STUB_LAUNCHCTL_PRINT_RCS=0:3
rc_nonzero no-return
did_kick no-return
err_has no-return "did not reload"

# 8. loaded but NOT READY (R2-2): launchctl reports the job loaded (rc 0 both probes)
#    and the kickstart succeeds, but the readiness probe never sees sshd accept a
#    connection -> fail closed, loud, NOT green.
reload_case not-ready STUB_LAUNCHCTL_PRINT_RCS=0:0 STUB_KEYSCAN_READY=0
rc_nonzero not-ready
did_kick not-ready
err_has not-ready "did NOT become ready"
err_has not-ready "lockout"

# 9. happy path: validated, kicked, launchd job reloaded AND the readiness probe saw
#    sshd accept a connection.
reload_case happy STUB_LAUNCHCTL_PRINT_RCS=0:0 STUB_KEYSCAN_READY=1
rc_zero happy
did_kick happy
out_has happy "accepting SSH connections"

if [[ $failures -gt 0 ]]; then
  printf 'ssh-hardening-reload-failclosed: %d assertion(s) FAILED\n' "$failures" >&2
  exit 1
fi
printf 'ssh-hardening-reload-failclosed: OK (validates before kickstart; distinguishes absent from errored; fails closed)\n'
