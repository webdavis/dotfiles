#!/usr/bin/env bash
#
# ssh-reload-lib.bash - stub harness for the disruptive modes (--reload,
# --rollback) of dot_local/bin/executable_ssh-hardening.sh, layered on
# ssh-hardening-lib.bash (which supplies the sandbox tree and the failing
# PATH `sudo` tripwire).
#
# Every tool the disruptive modes drive is a FULLY CONTROLLED stub reached
# through the SSH_HARDENING_SUDO / SSHD_BIN / LAUNCHCTL_BIN / KEYSCAN_BIN /
# SLEEP_BIN seams, and every seam is MIRRORED on PATH by a tripwire that
# always fails loudly (exit 96) and records the call: a regressed script that
# drops a seam and calls a bare tool name hits the tripwire, never the real
# tool. No test sourcing this library requires a real sshd, launchctl,
# ssh-keyscan, or sudo, so none can skip, and none can reach the live daemon
# through those routes.
#
# What the barriers do NOT guarantee, stated so nobody leans on more than is
# there: the tripwires shadow only PATH-RESOLVED bare names, so a regression
# that hard-codes /usr/bin/sudo or /bin/launchctl walks straight past them,
# and if the developer's sudo timestamp happens to be cached (or sudo is
# passwordless) such a call could really escalate. The backstop is narrower
# than "unprivileged execution makes violations impossible": it is that the
# specific dangerous actions here are root-gated by the OS itself -- launchd
# refuses an unprivileged kickstart of system/com.openssh.sshd and /etc/ssh
# is root-owned 0755 -- so THOSE fail loudly even if every layer above
# regressed at once. Keeping absolute-path escalation out of the script is a
# review property, not something this harness can enforce.

# shellcheck source=./ssh-hardening-lib.bash
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/ssh-hardening-lib.bash"

# reload_sandbox_setup: ssh_sandbox_setup plus the reload seams.
#
# Exports the seams (all absolute paths into the sandbox):
#   SSHD_BIN / LAUNCHCTL_BIN / KEYSCAN_BIN   controlled stubs
#   SLEEP_BIN                                a sleep spy that logs the delay
#                                            and returns immediately, so
#                                            nonzero-interval cases observe
#                                            their pacing without slowing the
#                                            suite
#   SSH_HARDENING_SUDO                       passthrough sudo stub (logs the
#                                            call, then executes it); point it
#                                            at SSH_SUDO_DENY_STUB for the
#                                            privilege-failure case
# and the spy logs:
#   SSHD_SPY_LOG, LAUNCHCTL_SPY_LOG, KEYSCAN_SPY_LOG, SLEEP_SPY_LOG,
#   SUDO_OK_SPY_LOG, SUDO_DENY_SPY_LOG, SSH_BARE_TOOL_SPY_LOG (the PATH
#   tripwires for bare sshd / launchctl / ssh-keyscan; bare sudo keeps slice
#   7's SSH_SUDO_SPY_LOG), and SSH_SEAM_CALL_LOG, one SHARED ordered log
#   every controlled stub appends `<tool> <argv>` to, so tests can assert
#   the exact sequence of seam calls across tools (a spy that records THAT a
#   call happened but not WHAT it asked lets a dropped flag, a typo'd label,
#   a wrong -f target, or a reordered step survive).
#
# Stub behavior knobs, all exported and overridable per invocation:
#   SSHD_STUB_SYNTAX_STATUS          exit status of `sshd -t` (default 0)
#   SSHD_STUB_RESOLVE_STATUSES       space-separated exit statuses for
#                                    successive `sshd -G` calls within one
#                                    run; the last entry repeats (default 0;
#                                    a uniform nonzero models a verifier
#                                    binary that runs but ERRORS, a trailing
#                                    nonzero fails only a later call such as
#                                    the reload's port resolution)
#   SSHD_STUB_PORT                   space-separated list of `port` lines
#                                    `sshd -G` prints (default 2222; empty
#                                    prints NO port line)
#   SSHD_STUB_FORCE_HARDENED         nonempty: -G prints hardened values even
#                                    with the drop-in absent (models a second
#                                    file still enforcing the policy)
#   SSHD_STUB_PARTIAL_HARDENED       nonempty: -G keeps BOTH interactive
#                                    password channels closed but leaves the
#                                    rest at defaults (models a sibling that
#                                    still blocks passwords after the drop-in
#                                    is gone, without verifying fully
#                                    hardened)
#   SSHD_STUB_BLOCKED_ADDRESSES      space-separated addr values: a `-G -T -C`
#                                    resolution whose spec carries one of
#                                    these addresses keeps BOTH interactive
#                                    password channels closed even when the
#                                    tree is otherwise open (models a Match
#                                    block scoped to that address). This is
#                                    what makes the stub's output VARY by
#                                    connection spec, so a recovery gate that
#                                    drops one of its two samples is caught
#                                    by a test instead of staying green
#   LAUNCHCTL_STUB_PRINT_STATUSES    space-separated exit statuses for
#                                    successive `launchctl print` calls within
#                                    one run; the last entry repeats
#   LAUNCHCTL_STUB_KICKSTART_STATUS  exit status of `launchctl kickstart` (0)
#   KEYSCAN_STUB_MODE                banner | refuse | silent-zero |
#                                    garbage-zero (exit 0 with output that is
#                                    NOT a host-key record) | banner-nonzero
#                                    (a real record but a nonzero status)
#   KEYSCAN_STUB_ANSWER_PORT         banner mode only: refuse every requested
#                                    port except this one (empty: answer any)
#
# The sshd stub keys its -G output off the PRESENCE of the sandbox drop-in, so
# --verify (which the reload and rollback paths re-run in a child) tracks what
# install or rollback actually did to the tree, the way the real sshd would.
reload_sandbox_setup() {
  ssh_sandbox_setup
  SSH_STUB_DIR="$SSH_SANDBOX/stubs"
  SSH_STUB_STATE="$SSH_SANDBOX/stub-state"
  mkdir -p "$SSH_STUB_DIR" "$SSH_STUB_STATE"
  SSHD_SPY_LOG="$SSH_SANDBOX/sshd-spy.log"
  LAUNCHCTL_SPY_LOG="$SSH_SANDBOX/launchctl-spy.log"
  KEYSCAN_SPY_LOG="$SSH_SANDBOX/keyscan-spy.log"
  SLEEP_SPY_LOG="$SSH_SANDBOX/sleep-spy.log"
  SUDO_OK_SPY_LOG="$SSH_SANDBOX/sudo-ok-spy.log"
  SUDO_DENY_SPY_LOG="$SSH_SANDBOX/sudo-deny-spy.log"
  SSH_BARE_TOOL_SPY_LOG="$SSH_SANDBOX/bare-tool-spy.log"
  SSH_SEAM_CALL_LOG="$SSH_SANDBOX/seam-call.log"
  : >"$SSHD_SPY_LOG"
  : >"$LAUNCHCTL_SPY_LOG"
  : >"$KEYSCAN_SPY_LOG"
  : >"$SLEEP_SPY_LOG"
  : >"$SUDO_OK_SPY_LOG"
  : >"$SUDO_DENY_SPY_LOG"
  : >"$SSH_BARE_TOOL_SPY_LOG"
  : >"$SSH_SEAM_CALL_LOG"

  # PATH tripwires: a bare-name call to any tool the reload modes touch must
  # fail loudly, never reach the real tool. $SSH_SANDBOX/bin is already first
  # on PATH (ssh_sandbox_setup put the failing `sudo` there).
  local tool
  for tool in sshd launchctl ssh-keyscan; do
    cat >"$SSH_SANDBOX/bin/$tool" <<STUB
#!/bin/bash
printf '%s %s\n' '$tool' "\$*" >>"\${SSH_BARE_TOOL_SPY_LOG:?}"
echo '$tool: bare-name call blocked by test tripwire (the script must use its seam)' >&2
exit 96
STUB
    chmod +x "$SSH_SANDBOX/bin/$tool"
  done

  # Controlled sshd: -t exits SSHD_STUB_SYNTAX_STATUS; -G (with or without
  # -T -C) prints a `port` line plus the seven protected directives, hardened
  # exactly when the sandbox drop-in exists (or SSHD_STUB_FORCE_HARDENED is
  # set), unhardened defaults otherwise. A `-C` spec whose addr is listed in
  # SSHD_STUB_BLOCKED_ADDRESSES resolves the two password channels closed
  # even on an otherwise-open tree, so the output varies by connection spec
  # the way the real resolver does under an address-scoped Match block.
  cat >"$SSH_STUB_DIR/sshd" <<'STUB'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$*" >>"${SSHD_SPY_LOG:?}"
printf 'sshd %s\n' "$*" >>"${SSH_SEAM_CALL_LOG:?}"
case " $* " in
  *' -t '*)
    if [[ ${SSHD_STUB_SYNTAX_STATUS:-0} -ne 0 ]]; then
      echo 'sshd stub: bad configuration option (syntax failure injected)' >&2
      exit "${SSHD_STUB_SYNTAX_STATUS}"
    fi
    exit 0
    ;;
esac
count_file="${SSH_STUB_STATE:?}/sshd-resolve-count"
count="$(cat "$count_file" 2>/dev/null || printf '0')"
count=$((count + 1))
printf '%s' "$count" >"$count_file"
# shellcheck disable=SC2206  # deliberate word split of the status list
statuses=(${SSHD_STUB_RESOLVE_STATUSES:-0})
index=$((count - 1))
if [[ $index -ge ${#statuses[@]} ]]; then
  index=$((${#statuses[@]} - 1))
fi
if [[ ${statuses[$index]} -ne 0 ]]; then
  echo 'sshd stub: -G resolution failure injected' >&2
  exit "${statuses[$index]}"
fi
# The connection spec, when this call is a per-connection resolution
# (`-G -T -C user=...,host=...,addr=...`). The addr field is what
# SSHD_STUB_BLOCKED_ADDRESSES keys on.
specification=''
previous=''
for argument in "$@"; do
  if [[ $previous == '-C' ]]; then
    specification="$argument"
  fi
  previous="$argument"
done
specification_address=''
case $specification in
  *addr=*)
    specification_address="${specification##*addr=}"
    specification_address="${specification_address%%,*}"
    ;;
esac
blocked_for_specification=0
if [[ -n ${SSHD_STUB_BLOCKED_ADDRESSES:-} && -n $specification_address ]]; then
  for blocked_address in ${SSHD_STUB_BLOCKED_ADDRESSES}; do
    if [[ $specification_address == "$blocked_address" ]]; then
      blocked_for_specification=1
      break
    fi
  done
fi
for stub_port in ${SSHD_STUB_PORT-2222}; do
  printf 'port %s\n' "$stub_port"
done
if [[ -f "${SSHD_CONFIG_D:?}/${SSH_DROPIN_NAME:?}" || -n ${SSHD_STUB_FORCE_HARDENED:-} ]]; then
  printf '%s\n' 'passwordauthentication no' 'kbdinteractiveauthentication no' \
    'usepam yes' 'pubkeyauthentication yes' 'permitrootlogin no' \
    'gssapiauthentication no' 'hostbasedauthentication no'
elif [[ -n ${SSHD_STUB_PARTIAL_HARDENED:-} || $blocked_for_specification -eq 1 ]]; then
  printf '%s\n' 'passwordauthentication no' 'kbdinteractiveauthentication no' \
    'usepam yes' 'pubkeyauthentication yes' 'permitrootlogin prohibit-password' \
    'gssapiauthentication no' 'hostbasedauthentication no'
else
  printf '%s\n' 'passwordauthentication yes' 'kbdinteractiveauthentication yes' \
    'usepam yes' 'pubkeyauthentication yes' 'permitrootlogin prohibit-password' \
    'gssapiauthentication no' 'hostbasedauthentication no'
fi
STUB
  chmod +x "$SSH_STUB_DIR/sshd"

  # Controlled launchctl: `print` walks LAUNCHCTL_STUB_PRINT_STATUSES one call
  # at a time (a per-run counter file keeps the position, so the pre- and
  # post-kickstart probes can answer differently); `kickstart` exits
  # LAUNCHCTL_STUB_KICKSTART_STATUS. Everything is logged first, so a test can
  # assert whether a kickstart was ATTEMPTED independently of how it exited.
  cat >"$SSH_STUB_DIR/launchctl" <<'STUB'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$*" >>"${LAUNCHCTL_SPY_LOG:?}"
printf 'launchctl %s\n' "$*" >>"${SSH_SEAM_CALL_LOG:?}"
case "${1:-}" in
  print)
    count_file="${SSH_STUB_STATE:?}/launchctl-print-count"
    count="$(cat "$count_file" 2>/dev/null || printf '0')"
    count=$((count + 1))
    printf '%s' "$count" >"$count_file"
    # shellcheck disable=SC2206  # deliberate word split of the status list
    statuses=(${LAUNCHCTL_STUB_PRINT_STATUSES:-0})
    index=$((count - 1))
    if [[ $index -ge ${#statuses[@]} ]]; then
      index=$((${#statuses[@]} - 1))
    fi
    status="${statuses[$index]}"
    if [[ $status -eq 0 ]]; then
      printf 'system/com.openssh.sshd = { state (stub) }\n'
    else
      printf 'launchctl stub: print exiting %s\n' "$status" >&2
    fi
    exit "$status"
    ;;
  kickstart)
    # Snapshot the script's captured output AT the moment of the disruptive
    # step: the kickstart is what can kill the session carrying the output,
    # so anything that must reach the operator has to be in these files
    # BEFORE this call. The runners redirect the script's stdout/stderr to
    # $SSH_SANDBOX/run.out and run.err, so copying them here gives tests a
    # pre-kickstart view to assert against.
    cat "${SSH_SANDBOX:?}/run.out" >"${SSH_STUB_STATE:?}/stdout-at-kickstart" 2>/dev/null || :
    cat "${SSH_SANDBOX:?}/run.err" >"${SSH_STUB_STATE:?}/stderr-at-kickstart" 2>/dev/null || :
    status="${LAUNCHCTL_STUB_KICKSTART_STATUS:-0}"
    if [[ $status -ne 0 ]]; then
      printf 'launchctl stub: kickstart refused (exit %s)\n' "$status" >&2
    fi
    exit "$status"
    ;;
  *)
    printf 'launchctl stub: unexpected subcommand: %s\n' "$*" >&2
    exit 64
    ;;
esac
STUB
  chmod +x "$SSH_STUB_DIR/launchctl"

  # Controlled ssh-keyscan: `banner` completes the exchange (a key line on
  # stdout naming the REQUESTED port, exit 0), unless KEYSCAN_STUB_ANSWER_PORT
  # names a different port, in which case the request is refused (multi-port
  # cases prove the probe walks every resolved port); `refuse` models
  # connection refused (exit 1, nothing on stdout); `silent-zero` exits 0
  # with NO output, the shape of a probe that ran but proved nothing, so a
  # reload that trusts the exit status alone and not the banner itself is
  # convicted by it.
  cat >"$SSH_STUB_DIR/ssh-keyscan" <<'STUB'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$*" >>"${KEYSCAN_SPY_LOG:?}"
printf 'ssh-keyscan %s\n' "$*" >>"${SSH_SEAM_CALL_LOG:?}"
requested_port=''
previous=''
for argument in "$@"; do
  if [[ $previous == '-p' ]]; then
    requested_port="$argument"
  fi
  previous="$argument"
done
case "${KEYSCAN_STUB_MODE:-banner}" in
  banner)
    if [[ -n ${KEYSCAN_STUB_ANSWER_PORT:-} && $requested_port != "${KEYSCAN_STUB_ANSWER_PORT}" ]]; then
      printf 'ssh-keyscan stub: connect to 127.0.0.1 port %s: Connection refused\n' "$requested_port" >&2
      exit 1
    fi
    printf '[127.0.0.1]:%s ssh-ed25519 AAAA-stub-host-key\n' "$requested_port"
    exit 0
    ;;
  refuse)
    printf 'ssh-keyscan stub: connect to 127.0.0.1: Connection refused\n' >&2
    exit 1
    ;;
  silent-zero)
    exit 0
    ;;
  garbage-zero)
    printf 'stub-noise-not-a-key-record\n'
    exit 0
    ;;
  banner-nonzero)
    printf '[127.0.0.1]:%s ssh-ed25519 AAAA-stub-host-key\n' "$requested_port"
    exit 1
    ;;
  *)
    printf 'ssh-keyscan stub: unknown KEYSCAN_STUB_MODE %s\n' "${KEYSCAN_STUB_MODE}" >&2
    exit 70
    ;;
esac
STUB
  chmod +x "$SSH_STUB_DIR/ssh-keyscan"

  # Controlled sleep: logs the requested delay and returns immediately. The
  # script's SLEEP_BIN default is the real /bin/sleep; tests point the seam
  # here so pacing is observable and a regression back to a bare `sleep`
  # (ignoring the seam) shows up as an empty spy log.
  cat >"$SSH_STUB_DIR/sleep" <<'STUB'
#!/bin/bash
printf '%s\n' "$*" >>"${SLEEP_SPY_LOG:?}"
printf 'sleep %s\n' "$*" >>"${SSH_SEAM_CALL_LOG:?}"
exit 0
STUB
  chmod +x "$SSH_STUB_DIR/sleep"

  # Passthrough sudo: logs, handles the -v priming call, then executes the
  # wrapped command directly (the sandbox is user-owned, so no privilege is
  # needed or taken).
  cat >"$SSH_STUB_DIR/sudo-ok" <<'STUB'
#!/bin/bash
printf '%s\n' "$*" >>"${SUDO_OK_SPY_LOG:?}"
if [[ ${1:-} == '-v' ]]; then
  exit 0
fi
exec "$@"
STUB
  chmod +x "$SSH_STUB_DIR/sudo-ok"

  # A wrapper that primes fine but fails ONLY for launchctl, with the exact
  # status the reload reads as "service absent", without ever running it:
  # the shape of a wrapper failure masquerading as a confirmed-absent
  # service. Everything else it is asked to run passes through.
  cat >"$SSH_STUB_DIR/sudo-launchctl-113" <<'STUB'
#!/bin/bash
if [[ ${1:-} == '-v' ]]; then
  exit 0
fi
case "${1:-}" in
  *launchctl*)
    printf '%s\n' "$*" >>"${SUDO_DENY_SPY_LOG:?}"
    printf 'sudo stub: refusing to run launchctl (exit 113 without executing it)\n' >&2
    exit 113
    ;;
esac
exec "$@"
STUB
  chmod +x "$SSH_STUB_DIR/sudo-launchctl-113"
  SSH_SUDO_LAUNCHCTL_113_STUB="$SSH_STUB_DIR/sudo-launchctl-113"

  # A wrapper that SWALLOWS rm: reports success without executing it, the
  # shape of a removal command that lies. Everything else passes through.
  cat >"$SSH_STUB_DIR/sudo-swallow-rm" <<'STUB'
#!/bin/bash
if [[ ${1:-} == '-v' ]]; then
  exit 0
fi
if [[ ${1:-} == 'rm' ]]; then
  printf '%s\n' "$*" >>"${SUDO_DENY_SPY_LOG:?}"
  exit 0
fi
exec "$@"
STUB
  chmod +x "$SSH_STUB_DIR/sudo-swallow-rm"
  SSH_SUDO_SWALLOW_RM_STUB="$SSH_STUB_DIR/sudo-swallow-rm"

  # Denying sudo: logs and fails the way a passwordless-sudo revocation or a
  # missing terminal does. Nothing it is asked to run ever runs.
  cat >"$SSH_STUB_DIR/sudo-deny" <<'STUB'
#!/bin/bash
printf '%s\n' "$*" >>"${SUDO_DENY_SPY_LOG:?}"
printf 'sudo: a password is required and no terminal is available\n' >&2
exit 1
STUB
  chmod +x "$SSH_STUB_DIR/sudo-deny"
  SSH_SUDO_DENY_STUB="$SSH_STUB_DIR/sudo-deny"

  SSHD_BIN="$SSH_STUB_DIR/sshd"
  LAUNCHCTL_BIN="$SSH_STUB_DIR/launchctl"
  KEYSCAN_BIN="$SSH_STUB_DIR/ssh-keyscan"
  SLEEP_BIN="$SSH_STUB_DIR/sleep"
  SSH_HARDENING_SUDO="$SSH_STUB_DIR/sudo-ok"
  SSHD_STUB_SYNTAX_STATUS=0
  SSHD_STUB_RESOLVE_STATUSES=0
  SSHD_STUB_PORT=2222
  SSHD_STUB_FORCE_HARDENED=""
  SSHD_STUB_PARTIAL_HARDENED=""
  SSHD_STUB_BLOCKED_ADDRESSES=""
  LAUNCHCTL_STUB_PRINT_STATUSES='0'
  LAUNCHCTL_STUB_KICKSTART_STATUS=0
  KEYSCAN_STUB_MODE=banner
  KEYSCAN_STUB_ANSWER_PORT=""
  export SSH_STUB_DIR SSH_STUB_STATE SSH_SUDO_DENY_STUB \
    SSH_SUDO_LAUNCHCTL_113_STUB SSH_SUDO_SWALLOW_RM_STUB \
    SSHD_SPY_LOG LAUNCHCTL_SPY_LOG KEYSCAN_SPY_LOG SLEEP_SPY_LOG \
    SUDO_OK_SPY_LOG SUDO_DENY_SPY_LOG SSH_BARE_TOOL_SPY_LOG \
    SSH_SEAM_CALL_LOG \
    SSHD_BIN LAUNCHCTL_BIN KEYSCAN_BIN SLEEP_BIN SSH_HARDENING_SUDO \
    SSHD_STUB_SYNTAX_STATUS SSHD_STUB_RESOLVE_STATUSES SSHD_STUB_PORT \
    SSHD_STUB_FORCE_HARDENED SSHD_STUB_PARTIAL_HARDENED \
    SSHD_STUB_BLOCKED_ADDRESSES \
    LAUNCHCTL_STUB_PRINT_STATUSES LAUNCHCTL_STUB_KICKSTART_STATUS \
    KEYSCAN_STUB_MODE KEYSCAN_STUB_ANSWER_PORT
}

# run_ssh_reload <args...>: run_ssh_hardening_bounded with fresh per-run spy
# state, then assert no bare-name tripwire fired. Every reload/rollback
# invocation goes through here so no run can reach a real tool unobserved,
# and every run is WALL-CLOCK BOUNDED (SSH_RELOAD_TIME_LIMIT seconds,
# default 30): a regression that makes the readiness loop infinite fails the
# gate instead of hanging it.
run_ssh_reload() {
  : >"$SSHD_SPY_LOG"
  : >"$LAUNCHCTL_SPY_LOG"
  : >"$KEYSCAN_SPY_LOG"
  : >"$SLEEP_SPY_LOG"
  : >"$SUDO_OK_SPY_LOG"
  : >"$SUDO_DENY_SPY_LOG"
  : >"$SSH_SEAM_CALL_LOG"
  rm -f "$SSH_STUB_STATE/launchctl-print-count" "$SSH_STUB_STATE/sshd-resolve-count" \
    "$SSH_STUB_STATE/stdout-at-kickstart" "$SSH_STUB_STATE/stderr-at-kickstart"
  run_ssh_hardening_bounded "${SSH_RELOAD_TIME_LIMIT:-30}" "$@"
  if [[ ${SSH_RUN_TIMED_OUT:-0} -eq 1 ]]; then
    printf 'FAIL: run_ssh_reload %s exceeded its %ss wall clock; a reload that can hang is itself a regression\n' \
      "$*" "${SSH_RELOAD_TIME_LIMIT:-30}" >&2
    exit 1
  fi
  if [[ -s $SSH_BARE_TOOL_SPY_LOG ]]; then
    printf 'FAIL: the script called a tool by bare name instead of its seam during %s; tripwire log:\n%s\n' \
      "run_ssh_reload $*" "$(cat "$SSH_BARE_TOOL_SPY_LOG")" >&2
    exit 1
  fi
}

# assert_kickstart_attempted <label> / assert_no_kickstart <label>: whether
# `launchctl kickstart` was invoked during the LAST run_ssh_reload, judged by
# the stub's own spy log, never by the script's output. The attempted form
# asserts the EXACT argv: without -k a running instance is never terminated
# (a reload "succeeds" while sshd serves the old configuration), and a
# typo'd label makes real launchctl exit 113, which the reload reads as
# Remote Login being off.
assert_kickstart_attempted() {
  if ! grep -qxF 'kickstart -k system/com.openssh.sshd' "$LAUNCHCTL_SPY_LOG"; then
    printf 'FAIL: %s: expected exactly "kickstart -k system/com.openssh.sshd"; launchctl spy log:\n%s\n' \
      "$1" "$(cat "$LAUNCHCTL_SPY_LOG")" >&2
    exit 1
  fi
}

# assert_seam_calls <label> <expected-line>...: the ORDERED argv of every
# seam call in the last run, diffed whole against the shared seam log. One
# assertion covers the argument of every call AND the order between them
# (syntax check before verify before port resolution before the kickstart
# before the probes), so none of those can regress individually.
assert_seam_calls() {
  local label="$1" expected_file="$SSH_SANDBOX/expected-seam-calls"
  shift
  printf '%s\n' "$@" >"$expected_file"
  if ! diff -u "$expected_file" "$SSH_SEAM_CALL_LOG" >&2; then
    printf 'FAIL: %s: seam calls differ from expected (diff above: - expected, + actual)\n' \
      "$label" >&2
    exit 1
  fi
}

assert_no_kickstart() {
  if grep -q '^kickstart ' "$LAUNCHCTL_SPY_LOG"; then
    printf 'FAIL: %s: a kickstart was attempted; launchctl spy log:\n%s\n' \
      "$1" "$(cat "$LAUNCHCTL_SPY_LOG")" >&2
    exit 1
  fi
}

# config_tree_fingerprint: every REGULAR FILE under the sandbox drop-in
# directory with its checksum, so "this mode wrote nothing" is judged by
# content rather than by file count. Stated coverage: paths and contents of
# regular files only -- not modes, ownership, symlinks, directories, the
# main config, or Include targets outside the drop-in directory -- and the
# suites compare it on the happy path, not after every case.
config_tree_fingerprint() {
  local file
  find "$SSHD_CONFIG_D" -type f -print0 | LC_ALL=C sort -z |
    while IFS= read -r -d '' file; do
      printf '%s ' "$file"
      cksum <"$file"
    done
}
