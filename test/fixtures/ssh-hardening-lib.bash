#!/usr/bin/env bash
#
# ssh-hardening-lib.bash - sandbox harness for dot_local/bin/executable_ssh-hardening.sh.
#
# Every test drives the script against a scratch sshd_config.d tree through the
# SSHD_CONFIG_D / SSHD_MAIN_CONFIG / SSH_HARDENING_SUDO seams and NEVER touches
# /etc/ssh. Two independent guards keep even a regressed script away from the
# live tree:
#
#   1. SSH_HARDENING_SUDO is exported EMPTY, so the script performs its writes
#      unprivileged inside the user-owned sandbox; a write to /etc/ssh would
#      need privilege the test never grants.
#   2. A deliberately FAILING `sudo` stub is prepended to PATH and records every
#      invocation, so a regressed script that runs a bare `sudo tee
#      /etc/ssh/...` fails loudly (exit 97) instead of reaching the live tree,
#      and the spy log convicts it.
#
# Every invocation goes through run_ssh_hardening below, which runs the script
# via /bin/bash: the deployed script runs under the system bash 3.2, so a
# bashism from a newer bash must fail here, not on the machine.

# Path to the script under test, resolved from this library's location.
SSH_HARDENING_SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/dot_local/bin/executable_ssh-hardening.sh"

# A literal carriage return, for fixtures that exercise sshd's whitespace set
# (misc.c WHITESPACE is " \t\r\n"). Named because a bare $'\r' inside a
# fixture line is invisible in a diff and in a terminal.
# shellcheck disable=SC2034  # read by the sourcing test, not by this library
SSH_CARRIAGE_RETURN=$'\r'

# ssh_sandbox_setup: build the scratch tree and export the seams.
# Exports SSH_SANDBOX, SSHD_CONFIG_D, SSHD_MAIN_CONFIG, SSH_HARDENING_SUDO,
# SSH_SUDO_SPY_LOG; prepends the failing sudo stub to PATH.
ssh_sandbox_setup() {
  SSH_SANDBOX="$(mktemp -d)"
  SSHD_CONFIG_D="$SSH_SANDBOX/sshd_config.d"
  SSHD_MAIN_CONFIG="$SSH_SANDBOX/sshd_config"
  SSH_SUDO_SPY_LOG="$SSH_SANDBOX/sudo-spy.log"
  mkdir -p "$SSHD_CONFIG_D" "$SSH_SANDBOX/bin"
  printf 'Include %s/*\n' "$SSHD_CONFIG_D" >"$SSHD_MAIN_CONFIG"
  cat >"$SSH_SANDBOX/bin/sudo" <<'STUB'
#!/bin/bash
printf '%s\n' "$*" >>"${SSH_SUDO_SPY_LOG:?}"
echo 'sudo: blocked by test stub (tests must never escalate)' >&2
exit 97
STUB
  chmod +x "$SSH_SANDBOX/bin/sudo"
  : >"$SSH_SUDO_SPY_LOG"
  PATH="$SSH_SANDBOX/bin:$PATH"
  SSH_HARDENING_SUDO=""
  export SSH_SANDBOX SSHD_CONFIG_D SSHD_MAIN_CONFIG SSH_HARDENING_SUDO \
    SSH_SUDO_SPY_LOG PATH
}

ssh_sandbox_teardown() {
  [[ -n ${SSH_SANDBOX:-} ]] && rm -rf "$SSH_SANDBOX"
}

# run_ssh_hardening <args...>: run the script under /bin/bash, capturing
# stdout, stderr, and the exit status into SSH_RUN_OUT / SSH_RUN_ERR /
# SSH_RUN_STATUS. Never lets a nonzero status kill the calling test.
# shellcheck disable=SC2034  # the three run results are read by the sourcing test
run_ssh_hardening() {
  local out_file="$SSH_SANDBOX/run.out" err_file="$SSH_SANDBOX/run.err"
  SSH_RUN_STATUS=0
  /bin/bash "$SSH_HARDENING_SCRIPT" "$@" >"$out_file" 2>"$err_file" ||
    SSH_RUN_STATUS=$?
  SSH_RUN_OUT="$(cat "$out_file")"
  SSH_RUN_ERR="$(cat "$err_file")"
}

# run_ssh_hardening_bounded <seconds> <args...>: run_ssh_hardening with a wall
# clock. If the script has not finished inside <seconds> it is killed and
# SSH_RUN_TIMED_OUT is set to 1, so a test can assert TERMINATION as a
# property instead of hanging the whole suite.
#
# No timeout(1): coreutils is not on a stock macOS runner, and a bound that
# only holds where Homebrew is installed is not a bound.
# shellcheck disable=SC2034  # the run results are read by the sourcing test
run_ssh_hardening_bounded() {
  local limit="$1"
  shift
  local out_file="$SSH_SANDBOX/run.out" err_file="$SSH_SANDBOX/run.err"
  local child waited=0 deadline=$((limit * 10))
  SSH_RUN_TIMED_OUT=0
  SSH_RUN_STATUS=0
  /bin/bash "$SSH_HARDENING_SCRIPT" "$@" >"$out_file" 2>"$err_file" &
  child=$!
  while kill -0 "$child" 2>/dev/null; do
    if [[ $waited -ge $deadline ]]; then
      kill -9 "$child" 2>/dev/null || true
      SSH_RUN_TIMED_OUT=1
      break
    fi
    sleep 0.1
    waited=$((waited + 1))
  done
  wait "$child" 2>/dev/null || SSH_RUN_STATUS=$?
  SSH_RUN_OUT="$(cat "$out_file")"
  SSH_RUN_ERR="$(cat "$err_file")"
}

# write_hostile_apple_conf: a 100-macos.conf that reopens EVERY hole the
# drop-in closes. Deliberately hostile, unlike the benign file Apple really
# ships (which sets only UsePAM, AcceptEnv, and the sftp Subsystem): the
# rename to 000- is insurance against a future Apple file competing on these
# directives, and the guard must prove precedence against the worst case, not
# against today's harmless reality.
write_hostile_apple_conf() {
  cat >"$SSHD_CONFIG_D/100-macos.conf" <<'EOF'
PasswordAuthentication yes
KbdInteractiveAuthentication yes
UsePAM no
PubkeyAuthentication no
PermitRootLogin yes
GSSAPIAuthentication yes
HostbasedAuthentication yes
EOF
}

# The protected directives and the value policy demands, as one list every
# test iterates. Kept here so adding a directive to the policy cannot leave a
# guard silently checking the old, shorter set.
# shellcheck disable=SC2034  # read by the sourcing tests, not by this library
SSH_HARDENED_PAIRS=(
  'passwordauthentication no'
  'kbdinteractiveauthentication no'
  'usepam yes'
  'pubkeyauthentication yes'
  'permitrootlogin no'
  'gssapiauthentication no'
  'hostbasedauthentication no'
)

# The value the hostile 100-macos.conf above sets for each of those, in the
# same order: the exact opposite, so precedence is proven against the worst
# case rather than against a file that agrees with policy by accident.
# shellcheck disable=SC2034  # read by the sourcing tests, not by this library
SSH_HOSTILE_PAIRS=(
  'passwordauthentication yes'
  'kbdinteractiveauthentication yes'
  'usepam no'
  'pubkeyauthentication no'
  'permitrootlogin yes'
  'gssapiauthentication yes'
  'hostbasedauthentication yes'
)

# effective_global_value <lowercase-key>: the test's OWN sshd -G resolution
# against the sandbox tree, independent of the script under test. Prints the
# effective value; fails if sshd -G itself fails.
effective_global_value() {
  local output
  output="$(/usr/sbin/sshd -G -f "$SSHD_MAIN_CONFIG" 2>&1)" || {
    printf 'sshd -G failed: %s\n' "$output" >&2
    return 1
  }
  printf '%s\n' "$output" | awk -v k="$1" '$1 == k { print $2; exit }'
}

# effective_spec_value <connection-spec> <lowercase-key>: the same independent
# resolution, but for one CONNECTION, so a Match block is applied. This is what
# turns a hostile fixture from an assertion about text into a demonstration:
# real sshd really does resolve the unsafe value for that spec.
effective_spec_value() {
  local output
  output="$(/usr/sbin/sshd -G -T -C "$1" -f "$SSHD_MAIN_CONFIG" 2>&1)" || {
    printf 'sshd -G -T -C %s failed: %s\n' "$1" "$output" >&2
    return 1
  }
  printf '%s\n' "$output" | awk -v k="$2" '$1 == k { print $2; exit }'
}

# assert_no_sudo_and_no_sandbox_write <label> <expected-listing>: the sudo spy
# never fired and the sandbox config tree still matches the given `ls -A`
# listing (purity check for the print modes).
assert_no_sudo_and_no_sandbox_write() {
  local label="$1" expected_listing="$2" actual_listing
  if [[ -s $SSH_SUDO_SPY_LOG ]]; then
    printf 'FAIL: %s escalated privilege; sudo spy log: %s\n' \
      "$label" "$(cat "$SSH_SUDO_SPY_LOG")" >&2
    return 1
  fi
  actual_listing="$(ls -A "$SSHD_CONFIG_D")"
  if [[ $actual_listing != "$expected_listing" ]]; then
    printf 'FAIL: %s wrote into the sandbox config dir (before: %s; after: %s)\n' \
      "$label" "$expected_listing" "$actual_listing" >&2
    return 1
  fi
}
