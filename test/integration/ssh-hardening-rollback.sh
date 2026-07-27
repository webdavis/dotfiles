#!/usr/bin/env bash
# ssh-hardening-rollback.sh -- --rollback is the way back in, expressed as
# code (slice 8, acceptance criterion 11). It removes the managed drop-in,
# re-verifies that the hardening is GONE from the effective configuration, and
# fails closed on every step it cannot prove. Everything here drives the stub
# seams from ssh-reload-lib.bash; no case can reach the live daemon or
# /etc/ssh.
#
# Properties pinned:
#   1. rollback removes the drop-in and a following --verify reports the
#      hardening absent
#   2. a second rollback is a clean no-op (exit 0, nothing to remove)
#   3. rollback never restarts anything: no launchctl call, no keyscan call
#   4. a removal that fails leaves a loud nonzero failure, not a success claim
#   5. a tree that STILL verifies hardened after the removal is a loud
#      failure: the drop-in was not the (only) thing enforcing the policy, so
#      password access is NOT back
#   6. an unrunnable verifier after the removal fails closed: the file is
#      gone, but rollback refuses to CLAIM the hardening is gone unchecked
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite runs from the pre-push hook.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

# shellcheck source=../fixtures/ssh-reload-lib.bash
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../fixtures" && pwd)/ssh-reload-lib.bash"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# A bare `! grep` is dead under `set -e` unless it is the last statement, so
# every negative goes through this helper.
refute_contains() { # <haystack> <fixed-string> <message>
  if grep -qiF -- "$2" <<<"$1"; then
    fail "$3"
  fi
}

assert_no_reload_side_effects() { # <label>
  [[ ! -s $LAUNCHCTL_SPY_LOG ]] ||
    fail "$1: rollback must never touch launchctl (spy: $(cat "$LAUNCHCTL_SPY_LOG"))"
  [[ ! -s $KEYSCAN_SPY_LOG ]] ||
    fail "$1: rollback must never probe the listener (spy: $(cat "$KEYSCAN_SPY_LOG"))"
}

reload_sandbox_setup
trap 'ssh_sandbox_teardown' EXIT

dropin="$SSHD_CONFIG_D/000-ssh-hardening.conf"

# --- 1: remove, and the hardening really is gone ------------------------------

write_hardened_dropin
run_ssh_reload --verify
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "setup: the hardened tree must verify before the rollback (stderr: $SSH_RUN_ERR)"

run_ssh_reload --rollback
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "1: rollback must succeed on a hardened tree (stderr: $SSH_RUN_ERR)"
[[ ! -e $dropin ]] ||
  fail '1: rollback must remove the drop-in'
grep -qF "$dropin" <<<"$SSH_RUN_OUT" ||
  fail "1: rollback must name the file it removed (stdout: $SSH_RUN_OUT)"
grep -qi 'no longer verifies fully hardened' <<<"$SSH_RUN_OUT" ||
  fail "1: rollback must report the hardening gone from the effective configuration (stdout: $SSH_RUN_OUT)"
assert_no_reload_side_effects '1'

run_ssh_reload --verify
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '1: after rollback, --verify must report the hardening ABSENT (it exited 0)'

# --- 2: a second rollback is a clean no-op ------------------------------------

run_ssh_reload --rollback
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "2: a second rollback must be a clean no-op (stderr: $SSH_RUN_ERR)"
grep -qi 'already absent' <<<"$SSH_RUN_OUT" ||
  fail "2: the no-op must say there was nothing to remove (stdout: $SSH_RUN_OUT)"
[[ ! -e $dropin ]] ||
  fail '2: the second rollback must leave the drop-in absent'
assert_no_reload_side_effects '2'

# --- 3: a failed removal is a loud failure, never a success claim -------------

write_hardened_dropin
SSH_HARDENING_SUDO="$SSH_SUDO_DENY_STUB" run_ssh_reload --rollback
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '3: rollback must fail when the removal command fails'
[[ -e $dropin ]] ||
  fail '3: with the removal refused, the drop-in must still be in place'
grep -qi 'could not remove' <<<"$SSH_RUN_ERR" ||
  fail "3: the failure must name the removal (stderr: $SSH_RUN_ERR)"
refute_contains "$SSH_RUN_OUT" 'rollback complete' \
  '3: a failed removal must not claim completion'
assert_no_reload_side_effects '3'

# --- 4: still hardened after removal -> loud failure --------------------------
# SSHD_STUB_FORCE_HARDENED models a sibling file still enforcing the policy:
# the drop-in is gone, yet the effective configuration still verifies
# hardened, so the way back in is NOT restored and rollback must say so.

SSHD_STUB_FORCE_HARDENED=1 run_ssh_reload --rollback
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '4: rollback must fail when the tree still verifies hardened after the removal'
[[ ! -e $dropin ]] ||
  fail '4: the drop-in itself must still have been removed'
grep -qi 'still verifies fully hardened' <<<"$SSH_RUN_ERR" ||
  fail "4: the failure must say the hardening is still in effect (stderr: $SSH_RUN_ERR)"
refute_contains "$SSH_RUN_OUT" 'rollback complete' \
  '4: a still-hardened tree must not produce a completion claim'
assert_no_reload_side_effects '4'

# --- 5: unrunnable verifier after removal -> fail closed ----------------------

write_hardened_dropin
SSHD_BIN="$SSH_SANDBOX/no-such-sshd" run_ssh_reload --rollback
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '5: rollback must fail closed when the removal cannot be verified'
[[ ! -e $dropin ]] ||
  fail '5: the removal itself must still happen (it is the way back in)'
grep -qi 'failing closed' <<<"$SSH_RUN_ERR" ||
  fail "5: the failure must say it is failing closed (stderr: $SSH_RUN_ERR)"
refute_contains "$SSH_RUN_OUT" 'rollback complete' \
  '5: an unverified removal must not claim completion'
assert_no_reload_side_effects '5'

printf 'ssh-hardening-rollback: OK (removal proven, no-op idempotent, every unprovable step fails closed)\n'
