#!/usr/bin/env bash
# ssh-hardening-verify-failclosed.sh -- --verify fails CLOSED whenever it
# cannot actually check anything (slice 7, acceptance criterion 6). Fail-open
# is the cardinal sin: "cannot run the verifier" and "cannot read a drop-in"
# must both resolve to FAILURE, never to a verified claim.
#
# The properties pinned:
#   1. SSHD_BIN pointing at a nonexistent path, no test seam: --verify exits
#      nonzero and says it is failing closed.
#   2. Same broken SSHD_BIN with the SSH_HARDENING_ALLOW_MISSING_SSHD seam
#      set: --verify skips cleanly (exit 0) and does NOT print a verified
#      claim.
#   3. An unreadable drop-in in the tree: --verify exits nonzero and names the
#      unreadable file, from the raw scan's own "cannot read" branch (distinct
#      from sshd's Permission denied wording, so the scan's fail-closed branch
#      is pinned even though sshd -G also refuses).
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite runs from the pre-commit and pre-push hooks.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

# shellcheck source=../fixtures/ssh-hardening-lib.bash
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../fixtures" && pwd)/ssh-hardening-lib.bash"

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

ssh_sandbox_setup
trap 'ssh_sandbox_teardown' EXIT

# --- 1: missing verifier, no seam -> nonzero, says failing closed -----------

SSHD_BIN="$SSH_SANDBOX/no-such-sshd" run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'with SSHD_BIN nonexistent and no seam, --verify must exit nonzero'
grep -qi 'failing closed' <<<"$SSH_RUN_ERR" ||
  fail "the failure must say it is failing closed (stderr: $SSH_RUN_ERR)"
grep -qF "$SSH_SANDBOX/no-such-sshd" <<<"$SSH_RUN_ERR" ||
  fail "the failure must name the unrunnable verifier (stderr: $SSH_RUN_ERR)"

# --- 2: missing verifier, seam set -> clean skip, no verified claim ---------

SSHD_BIN="$SSH_SANDBOX/no-such-sshd" SSH_HARDENING_ALLOW_MISSING_SSHD=1 \
  run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "with the seam set, --verify must skip cleanly (exit 0), got $SSH_RUN_STATUS (stderr: $SSH_RUN_ERR)"
grep -qi 'skip' <<<"$SSH_RUN_OUT" ||
  fail "the seam path must say it skipped (stdout: $SSH_RUN_OUT)"
refute_contains "$SSH_RUN_OUT" 'verified' \
  'the seam path must not print a verified claim'
refute_contains "$SSH_RUN_OUT" 'PASS' \
  'the seam path must not print a PASS claim'

# --- 3: unreadable drop-in -> nonzero, names the file ------------------------

[[ -x /usr/sbin/sshd ]] || {
  printf 'SKIP: /usr/sbin/sshd not present; cannot exercise the unreadable-drop-in case\n'
  exit 0
}

# A fully hardened tree, then one unreadable sibling: the tree WOULD verify,
# so only the fail-closed handling of the unreadable file can fail it.
/bin/bash "$SSH_HARDENING_SCRIPT" --print-config \
  >"$SSHD_CONFIG_D/000-ssh-hardening.conf" 2>/dev/null || true
unreadable="$SSHD_CONFIG_D/060-unreadable.conf"
printf '# contents beside the point; the mode is the case\n' >"$unreadable"
chmod 000 "$unreadable"

run_ssh_hardening --verify
chmod 644 "$unreadable" # so teardown can remove it regardless of the verdict
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'with an unreadable drop-in, --verify must exit nonzero'
grep -qF "$unreadable" <<<"$SSH_RUN_ERR" ||
  fail "the failure must name the unreadable file (stderr: $SSH_RUN_ERR)"
grep -qi 'cannot read' <<<"$SSH_RUN_ERR" ||
  fail "the raw scan's own cannot-read branch must fire, not only sshd's refusal (stderr: $SSH_RUN_ERR)"
refute_contains "$SSH_RUN_OUT" 'verify: PASS' \
  'an unreadable drop-in must never produce a pass claim'

# --- 4: the skip seam is ON only for a TRUE-ish value ------------------------
# The seam was tested for being NONEMPTY, so every value enabled it -- setting
# it to `0`, the value that reads as "off", switched verification off. Both
# directions are asserted, because "ignore the seam entirely" would satisfy
# only half of this.

for off_value in 0 false no off FALSE Off; do
  SSHD_BIN="$SSH_SANDBOX/no-such-sshd" \
    SSH_HARDENING_ALLOW_MISSING_SSHD="$off_value" run_ssh_hardening --verify
  [[ $SSH_RUN_STATUS -ne 0 ]] ||
    fail "seam='$off_value' reads as OFF and must NOT enable the skip; --verify exited 0 (stdout: $SSH_RUN_OUT)"
  grep -qi 'failing closed' <<<"$SSH_RUN_ERR" ||
    fail "seam='$off_value': the failure must say it is failing closed (stderr: $SSH_RUN_ERR)"
  refute_contains "$SSH_RUN_OUT" 'skip' \
    "seam='$off_value' must not report a skip"
done

for on_value in 1 true yes on TRUE On; do
  SSHD_BIN="$SSH_SANDBOX/no-such-sshd" \
    SSH_HARDENING_ALLOW_MISSING_SSHD="$on_value" run_ssh_hardening --verify
  [[ $SSH_RUN_STATUS -eq 0 ]] ||
    fail "seam='$on_value' reads as ON and must still skip cleanly, got $SSH_RUN_STATUS (stderr: $SSH_RUN_ERR)"
  grep -qi 'skip' <<<"$SSH_RUN_OUT" ||
    fail "seam='$on_value': the seam path must say it skipped (stdout: $SSH_RUN_OUT)"
  refute_contains "$SSH_RUN_OUT" 'verified' \
    "seam='$on_value' must not print a verified claim"
done

# --- 5: a file listing that could not be built is not an empty tree ----------
# The list of files to scan was produced by a pipeline inside a process
# substitution, whose exit status nothing can observe. A failing `sort` there
# yields ZERO files, and a scan of nothing finds nothing wrong: --verify
# printed PASS over a tree it had never read. The tree here is fully hardened
# on purpose, so the listing failure is the only thing that can fail it.

write_hardened_dropin
run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "positive control: the hardened sandbox must verify before the listing is broken (stderr: $SSH_RUN_ERR)"

run_ssh_hardening_without sort --verify
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'a configuration listing that could not be built must FAIL the verify, not pass it as an empty tree'
grep -qi 'list the configuration files' <<<"$SSH_RUN_ERR" ||
  fail "the failure must name the listing it could not build (stderr: $SSH_RUN_ERR)"
refute_contains "$SSH_RUN_OUT" 'verify: PASS' \
  'a scan that read no files must never produce a pass claim'

run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "--verify must PASS again once the listing works; state leaked (stderr: $SSH_RUN_ERR)"

# --- 6: every command the verify depends on is checked BY NAME ---------------
# Each of these used to be visible only through errexit, which the install path
# switched off. Asserting the WORDING and not merely the exit status is what
# pins the individual guard: without it the run still fails, but for a reason
# nobody can act on -- a spec built from an empty user name, or an unset value
# read as an absent directive.

assert_named_failure() { # <broken command> <message needle>
  run_ssh_hardening_without "$1" --verify
  [[ $SSH_RUN_STATUS -ne 0 ]] ||
    fail "a failing '$1' must FAIL the verify (stdout: $SSH_RUN_OUT)"
  grep -qi -- "$2" <<<"$SSH_RUN_ERR" ||
    fail "a failing '$1' must be reported as '$2' rather than left to surface as some downstream confusion (stderr: $SSH_RUN_ERR)"
  refute_contains "$SSH_RUN_OUT" 'verify: PASS' \
    "a failing '$1' must never produce a pass claim"
  run_ssh_hardening --verify
  [[ $SSH_RUN_STATUS -eq 0 ]] ||
    fail "--verify must PASS again once '$1' works (stderr: $SSH_RUN_ERR)"
}

assert_named_failure id 'could not determine the invoking user'
assert_named_failure awk 'could not read'

printf 'ssh-hardening-verify-failclosed: OK (missing verifier fails closed, the skip seam honours only true-ish values, an unreadable drop-in and an unbuildable listing both fail and are named)\n'
