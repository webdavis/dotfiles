#!/usr/bin/env bash
# update-skills-publish-lock-owner.sh (integration-fix F1): lock publication must
# never grant two owners. The audit found a three-writer interleave in
# __update_skills_publish_lock:
#   - B's staging is moved INSIDE A's held lock (macOS mv-into-dir semantics);
#   - A exits and deletes the whole lock tree (including B's nested staging);
#   - C acquires the now-absent FINAL path with its own owner token inside;
#   - B looks, finds NO nested staging, and (pre-fix) declares success — while C
#     also owns the lock. Two owners then recover/prune/exchange concurrently.
# The fix: after the publish `mv`, read $LOCKDIR/owner back and require it to
# equal our token; an absent nested staging is not proof of ownership. This test
# drives that exact interleave deterministically by stubbing the mv/exit sequence
# and asserts EXACTLY ONE owner (C), with B reporting a failed acquisition.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.agents"

export UPDATE_SKILLS_LIB_ONLY=1
# shellcheck disable=SC1090
source "$SCRIPT"

# B is this process; A and C are simulated peers with distinct tokens.
LOCK_MY_TOKEN="B-token-$$"
A_TOKEN="A-token-1"
C_TOKEN="C-token-2"

# Baseline: A holds the final lock (LOCKDIR present, A's owner token inside).
mkdir -p "$LOCKDIR"
printf '%s' "$A_TOKEN" >"$LOCK_OWNER_FILE"

# B builds its own staging lock dir with its token inside (as acquisition does).
staging="$(mktemp -d "${LOCKDIR}.stage.XXXXXX")"
printf '%s' "$LOCK_MY_TOKEN" >"$staging/owner"

# Stub `mv` to enact the three-writer interleave on B's publish move ONLY. The
# net observable state after the race is: the move "succeeded" (rc 0), B's
# staging is gone, and the FINAL lock now belongs to C. Every other mv delegates
# to the real binary so the function under test is otherwise untouched.
# shellcheck disable=SC2317,SC2329 # invoked indirectly by __update_skills_publish_lock
mv() {
  if [[ ${1:-} == "$staging" && ${2:-} == "$LOCKDIR" ]]; then
    command rm -rf "$staging" # B's staging: moved inside A's lock, then deleted by A on exit
    command rm -rf "$LOCKDIR" # A exits and tears the whole lock tree down
    command mkdir -p "$LOCKDIR"
    printf '%s' "$C_TOKEN" >"$LOCK_OWNER_FILE" # C acquires the now-absent final path
    return 0
  fi
  command mv "$@"
}

set +e
__update_skills_publish_lock "$staging"
rc=$?
set -e
unset -f mv

# 1) B must NOT declare ownership: the readback owner is C's token, not B's.
[[ $rc -ne 0 ]] ||
  fail "publish_lock returned success for B though C owns the final lock (two owners)"

# 2) Exactly one owner survives, and it is C (B never clobbered C's token).
[[ -f $LOCK_OWNER_FILE ]] || fail "the final lock owner file vanished"
owner="$(cat "$LOCK_OWNER_FILE")"
[[ $owner == "$C_TOKEN" ]] ||
  fail "the surviving lock owner is '$owner', expected C's token '$C_TOKEN'"

# 3) Sanity: the uncontended path still grants ownership. A fresh staging renamed
#    onto an ABSENT final path (owner token inside) is a genuine win.
command rm -rf "$LOCKDIR"
staging2="$(mktemp -d "${LOCKDIR}.stage.XXXXXX")"
printf '%s' "$LOCK_MY_TOKEN" >"$staging2/owner"
set +e
__update_skills_publish_lock "$staging2"
rc2=$?
set -e
[[ $rc2 -eq 0 ]] || fail "publish_lock did not grant the uncontended win"
[[ "$(cat "$LOCK_OWNER_FILE")" == "$LOCK_MY_TOKEN" ]] ||
  fail "the uncontended win did not record our owner token"

echo "update-skills-publish-lock-owner: OK"
