#!/usr/bin/env bash
# ssh-hardening-install-strictness.sh -- install judges the tree by exactly the
# rules --verify applies on its own.
#
# bash suppresses `set -e` for every command inside an `if !` or `||` test, and
# that suppression reaches into called functions and even into subshells
# (confirmed on the bash 3.2 this script is deployed under). install called
# `if ! verify`, so every check inside ran with errexit switched off: a command
# whose failure aborts --verify on its own was stepped over in the install
# path, execution carried on past it, and install printed its success line.
#
# Demonstrated with a failing `id`: standalone --verify exited 91 while install
# exited 0 and claimed "install complete" on the same tree.
#
# The assertion here is DIFFERENTIAL, not mechanism-specific. For every
# injected environmental fault: if standalone --verify fails, install must fail
# too and must not claim success. That holds a future unguarded command to the
# same rule without this file having to know which command it is, and it fails
# in both directions -- an install that refused everything would break the
# control row.
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

refute_contains() { # <haystack> <fixed-string> <message>
  if grep -qiF -- "$2" <<<"$1"; then
    fail "$3"
  fi
}

[[ -x /usr/sbin/sshd ]] || {
  printf 'SKIP: /usr/sbin/sshd not present; cannot resolve effective configuration\n'
  exit 0
}

ssh_sandbox_setup
trap 'ssh_sandbox_teardown' EXIT

# reset_tree: a known-good hardened tree, built WITHOUT install, so each case
# starts from the same place regardless of what the previous install did.
reset_tree() {
  rm -f "$SSHD_CONFIG_D"/* "$SSHD_CONFIG_D"/.[!.]* 2>/dev/null || true
  write_hardened_dropin
}

# assert_install_no_weaker_than_verify <label> <command to break, or ''>
assert_install_no_weaker_than_verify() {
  local label="$1" broken="$2" verify_status install_status install_output

  reset_tree
  if [[ -n $broken ]]; then
    run_ssh_hardening_without "$broken" --verify
  else
    run_ssh_hardening --verify
  fi
  verify_status=$SSH_RUN_STATUS

  reset_tree
  if [[ -n $broken ]]; then
    run_ssh_hardening_without "$broken"
  else
    run_ssh_hardening
  fi
  install_status=$SSH_RUN_STATUS
  install_output="$SSH_RUN_OUT"

  if [[ $verify_status -eq 0 ]]; then
    [[ $install_status -eq 0 ]] ||
      fail "$label: standalone --verify passed but install exited $install_status; install must not be STRICTER than the verify it runs"
    grep -qF 'install complete' <<<"$install_output" ||
      fail "$label: a passing verify must let install claim success (stdout: $install_output)"
    return 0
  fi

  [[ $install_status -ne 0 ]] ||
    fail "$label: standalone --verify exited $verify_status but install exited 0; the install path is running the checks with errexit disabled"
  refute_contains "$install_output" 'install complete' \
    "$label: install claimed success over a fault that fails --verify on its own"
}

# The control row. Without it, an install that refused every tree would pass
# every other row in this table.
assert_install_no_weaker_than_verify 'control: no injected fault' ''

# The demonstrated one. `id -un` builds the connection spec; its failure used
# to be stepped over, leaving a spec built from an empty user name.
assert_install_no_weaker_than_verify 'failing id' id

# The file listing for the scan.
assert_install_no_weaker_than_verify 'failing sort' sort

# The text extraction each protected value is read with.
assert_install_no_weaker_than_verify 'failing awk' awk

# A fault the script cannot guard away, so this row stays meaningful however
# thoroughly the individual commands are checked: the verifier itself refuses
# to run.
reset_tree
verify_status=0
SSHD_BIN="$SSH_SANDBOX/no-such-sshd" run_ssh_hardening --verify || true
verify_status=$SSH_RUN_STATUS
[[ $verify_status -ne 0 ]] ||
  fail 'unrunnable verifier: standalone --verify must fail closed'
reset_tree
SSHD_BIN="$SSH_SANDBOX/no-such-sshd" run_ssh_hardening
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'unrunnable verifier: install must fail closed too'
refute_contains "$SSH_RUN_OUT" 'install complete' \
  'unrunnable verifier: install must not claim success'

printf 'ssh-hardening-install-strictness: OK (install is never weaker than standalone --verify under an injected id, sort or awk failure, nor with an unrunnable verifier; the control row keeps a blanket refusal from passing)\n'
