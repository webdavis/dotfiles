#!/usr/bin/env bash
#
# ssh-reload-lib.bash - stub harness for the disruptive modes (--reload,
# --rollback) of dot_local/bin/executable_ssh-hardening.sh, layered on
# ssh-hardening-lib.bash (which supplies the sandbox tree and the failing
# PATH `sudo` tripwire).
#
# Every tool the disruptive modes drive is a FULLY CONTROLLED stub reached
# through the SSH_HARDENING_SUDO / SSHD_BIN / LAUNCHCTL_BIN / KEYSCAN_BIN
# seams, and every seam is MIRRORED on PATH by a tripwire that always fails
# loudly (exit 96) and records the call: a regressed script that drops a seam
# and calls a bare tool name hits the tripwire, never the real tool. No test
# sourcing this library requires a real sshd, launchctl, ssh-keyscan, or sudo,
# so none can skip, and none can reach the live daemon. On top of the seams
# and the tripwires, the tests run UNPRIVILEGED: even if every layer above
# regressed at once, launchd refuses an unprivileged kickstart of the real
# system/com.openssh.sshd, and /etc/ssh is root-owned, so the violation would
# be loud and harmless rather than a live restart.

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
#   LAUNCHCTL_SPY_LOG, KEYSCAN_SPY_LOG, SLEEP_SPY_LOG, SUDO_OK_SPY_LOG,
#   SUDO_DENY_SPY_LOG, SSH_BARE_TOOL_SPY_LOG (the PATH tripwires for bare
#   sshd / launchctl / ssh-keyscan; bare sudo keeps slice 7's
#   SSH_SUDO_SPY_LOG).
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
  LAUNCHCTL_SPY_LOG="$SSH_SANDBOX/launchctl-spy.log"
  KEYSCAN_SPY_LOG="$SSH_SANDBOX/keyscan-spy.log"
  SLEEP_SPY_LOG="$SSH_SANDBOX/sleep-spy.log"
  SUDO_OK_SPY_LOG="$SSH_SANDBOX/sudo-ok-spy.log"
  SUDO_DENY_SPY_LOG="$SSH_SANDBOX/sudo-deny-spy.log"
  SSH_BARE_TOOL_SPY_LOG="$SSH_SANDBOX/bare-tool-spy.log"
  : >"$LAUNCHCTL_SPY_LOG"
  : >"$KEYSCAN_SPY_LOG"
  : >"$SLEEP_SPY_LOG"
  : >"$SUDO_OK_SPY_LOG"
  : >"$SUDO_DENY_SPY_LOG"
  : >"$SSH_BARE_TOOL_SPY_LOG"

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
  # set), unhardened defaults otherwise.
  cat >"$SSH_STUB_DIR/sshd" <<'STUB'
#!/bin/bash
set -euo pipefail
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
for stub_port in ${SSHD_STUB_PORT-2222}; do
  printf 'port %s\n' "$stub_port"
done
if [[ -f "${SSHD_CONFIG_D:?}/000-ssh-hardening.conf" || -n ${SSHD_STUB_FORCE_HARDENED:-} ]]; then
  printf '%s\n' 'passwordauthentication no' 'kbdinteractiveauthentication no' \
    'usepam yes' 'pubkeyauthentication yes' 'permitrootlogin no' \
    'gssapiauthentication no' 'hostbasedauthentication no'
elif [[ -n ${SSHD_STUB_PARTIAL_HARDENED:-} ]]; then
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
  LAUNCHCTL_STUB_PRINT_STATUSES='0'
  LAUNCHCTL_STUB_KICKSTART_STATUS=0
  KEYSCAN_STUB_MODE=banner
  KEYSCAN_STUB_ANSWER_PORT=""
  export SSH_STUB_DIR SSH_STUB_STATE SSH_SUDO_DENY_STUB \
    LAUNCHCTL_SPY_LOG KEYSCAN_SPY_LOG SLEEP_SPY_LOG SUDO_OK_SPY_LOG \
    SUDO_DENY_SPY_LOG SSH_BARE_TOOL_SPY_LOG \
    SSHD_BIN LAUNCHCTL_BIN KEYSCAN_BIN SLEEP_BIN SSH_HARDENING_SUDO \
    SSHD_STUB_SYNTAX_STATUS SSHD_STUB_RESOLVE_STATUSES SSHD_STUB_PORT \
    SSHD_STUB_FORCE_HARDENED SSHD_STUB_PARTIAL_HARDENED \
    LAUNCHCTL_STUB_PRINT_STATUSES LAUNCHCTL_STUB_KICKSTART_STATUS \
    KEYSCAN_STUB_MODE KEYSCAN_STUB_ANSWER_PORT
}

# run_ssh_reload <args...>: run_ssh_hardening with fresh per-run spy state,
# then assert no bare-name tripwire fired. Every reload/rollback invocation
# goes through here so no run can reach a real tool unobserved.
run_ssh_reload() {
  : >"$LAUNCHCTL_SPY_LOG"
  : >"$KEYSCAN_SPY_LOG"
  : >"$SLEEP_SPY_LOG"
  : >"$SUDO_OK_SPY_LOG"
  : >"$SUDO_DENY_SPY_LOG"
  rm -f "$SSH_STUB_STATE/launchctl-print-count" "$SSH_STUB_STATE/sshd-resolve-count" \
    "$SSH_STUB_STATE/stdout-at-kickstart" "$SSH_STUB_STATE/stderr-at-kickstart"
  run_ssh_hardening "$@"
  if [[ -s $SSH_BARE_TOOL_SPY_LOG ]]; then
    printf 'FAIL: the script called a tool by bare name instead of its seam during %s; tripwire log:\n%s\n' \
      "run_ssh_reload $*" "$(cat "$SSH_BARE_TOOL_SPY_LOG")" >&2
    exit 1
  fi
}

# assert_kickstart_attempted <label> / assert_no_kickstart <label>: whether
# `launchctl kickstart` was invoked during the LAST run_ssh_reload, judged by
# the stub's own spy log, never by the script's output.
assert_kickstart_attempted() {
  if ! grep -q '^kickstart ' "$LAUNCHCTL_SPY_LOG"; then
    printf 'FAIL: %s: expected a kickstart attempt; launchctl spy log:\n%s\n' \
      "$1" "$(cat "$LAUNCHCTL_SPY_LOG")" >&2
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

# config_tree_fingerprint: every file under the sandbox drop-in directory with
# its checksum, so "this mode wrote nothing" is judged byte-for-byte rather
# than by file count.
config_tree_fingerprint() {
  local file
  find "$SSHD_CONFIG_D" -type f -print0 | LC_ALL=C sort -z |
    while IFS= read -r -d '' file; do
      printf '%s ' "$file"
      cksum <"$file"
    done
}
