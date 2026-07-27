#!/usr/bin/env bash
# ssh-hardening-dropin-mode.sh -- install pins the drop-in to mode 0644 even
# under umask 0077 (slice 7). The mode is not tidiness: a root-owned 0600
# drop-in makes UNPRIVILEGED `sshd -G` fail outright (permission denied), so
# the entire three-way verification becomes unrunnable without elevation. An
# explicit chmod, never the ambient umask, is what makes 0644 deterministic.
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite runs from the pre-push hook.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

# shellcheck source=../fixtures/ssh-hardening-lib.bash
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../fixtures" && pwd)/ssh-hardening-lib.bash"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -x /usr/sbin/sshd ]] || {
  printf 'SKIP: /usr/sbin/sshd not present; install cannot verify\n'
  exit 0
}

# file_mode <path> -> octal mode. GNU form first, BSD form as the fallback:
# under the nix dev shell GNU coreutils shadows the system stat even on macOS,
# and GNU's `-f` flag means "filesystem status", which SUCCEEDS with garbage
# output instead of failing over. See test/test-system/stat-order.sh.
file_mode() { stat -c '%a' "$1" 2>/dev/null || stat -f '%Lp' "$1"; }

ssh_sandbox_setup
trap 'ssh_sandbox_teardown' EXIT

# The hostile umask: without the explicit chmod, tee would land the file 0600.
umask 0077

run_ssh_hardening
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "install must succeed (stderr: $SSH_RUN_ERR)"

dropin="$SSHD_CONFIG_D/000-ssh-hardening.conf"
[[ -f $dropin ]] || fail "install must write $dropin"
mode="$(file_mode "$dropin")"
[[ $mode == '644' ]] ||
  fail "the drop-in must be pinned to 0644 under umask 0077, got 0$mode"

printf 'ssh-hardening-dropin-mode: OK (0644 pinned by explicit chmod, umask 0077 notwithstanding)\n'
