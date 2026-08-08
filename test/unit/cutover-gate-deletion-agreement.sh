#!/usr/bin/env bash
# cutover-gate-deletion-agreement.sh: a path both branches deleted is agreement,
# not an unclassified blocker.
#
# WHY THIS EXISTS. Measured on dresden 2026-08-05, mid-cutover: after the real
# differences were classified and accepted, gate 1 still refused with 92
# blockers. Every one of them was a file that existed at the Phase A base and
# was deleted by BOTH integration and main, the tmux helpers, the retired Claude
# LaunchAgent, the sesh configs, the retired skills. The classifier treated two
# empty tree lookups as a failed lookup, on the stated premise that "the path is
# in the manifest, so it exists on at least one side". That premise is wrong:
# the manifest is `git diff PHASE_A_BASE INT_SHA`, which lists deletions, so a
# path integration removed is in the manifest and absent at INT_SHA.
#
# The cost of getting this wrong is not a missed defect, it is the opposite: the
# gate demands a written reason for 92 non-events, which trains the operator to
# write reasons that assert nothing, and a ledger full of hollow reasons is
# worse evidence than no ledger. The cases below pin the discriminator in both
# directions so the fix cannot decay into "empty means fine".
set -euo pipefail

# git exports GIT_DIR/GIT_INDEX_FILE when this runs under the pre-commit hook;
# unset so nothing here can reach the outer repository.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUNNER="$REPO_ROOT/scripts/cutover-gate.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# A throwaway history covering the three states the classifier must tell apart.
# EVERY fixture path must be one INTEGRATION changed relative to the base, because
# the manifest is `git diff PHASE_A_BASE INT_SHA`; a path only main touched never
# enters the manifest and so proves nothing. An earlier draft of this test got
# that wrong and its one-sided case silently exercised nothing.
#
#   both-deleted.txt   int deletes, main deletes  -> agreement, must NOT block
#   int-deleted-kept.txt  int deletes, main KEEPS -> one-sided, must still block
#   both-changed.txt   int edits, main edits the same way -> ordinary agreement
repo="$work/repo"
mkdir -p "$repo"
git -C "$repo" init -q
git_c() { git -C "$repo" -c user.email=t@t -c user.name=t "$@"; }

printf 'base\n' >"$repo/both-deleted.txt"
printf 'base\n' >"$repo/int-deleted-kept.txt"
printf 'base\n' >"$repo/both-changed.txt"
git_c add -A && git_c commit -qm base
BASE="$(git -C "$repo" rev-parse HEAD)"

# integration deletes two and edits the third
git_c rm -q both-deleted.txt int-deleted-kept.txt
printf 'edited\n' >"$repo/both-changed.txt"
git_c add -A && git_c commit -qm integration
INT="$(git -C "$repo" rev-parse HEAD)"

# main deletes only the first and makes the identical edit, so it KEEPS
# int-deleted-kept.txt: that path is the one-sided delta the gate must catch
git_c checkout -q -b mainline "$BASE"
git_c rm -q both-deleted.txt
printf 'edited\n' >"$repo/both-changed.txt"
git_c add -A && git_c commit -qm mainline
MAIN="$(git -C "$repo" rev-parse HEAD)"

ledger_dir="$work/state"
mkdir -p "$ledger_dir"

# Drive the runner's own classifier rather than reimplementing it, so an edit to
# the runner is what this test sees.
run_classifier() {
  local scratch
  scratch="$(mktemp -d "$work/scratch.XXXXXX")"
  # shellcheck disable=SC2317,SC2329  # invoked by the sourced function on failure
  die() {
    printf 'die: %s\n' "$*" >&2
    exit 9
  }
  # shellcheck disable=SC2317,SC2329  # invoked by the sourced classifier
  tree_entry() { # <ref> <path>
    git -C "$repo" ls-tree "$1" -- "$2" | awk '{print $1" "$3}'
  }
  # Same predicate as the runner's (scripts/cutover-gate.sh). Without it the
  # sourced function calls an undefined command, the pin check fails, and the
  # whole test dies at exit 9 while a piped invocation still reports success.
  # shellcheck disable=SC2317,SC2329  # invoked by the sourced classifier
  valid_sha() { [[ $1 =~ ^[0-9a-f]{40}$ ]]; }
  # The runner's reporting helpers. They only print, so stubbing them to no-ops
  # keeps the classifier's control flow identical while the test stays quiet.
  # shellcheck disable=SC2317,SC2329
  ok() { :; }
  # shellcheck disable=SC2317,SC2329
  say() { :; }
  # shellcheck disable=SC2317,SC2329
  checkpoint() { :; }
  # shellcheck source=/dev/null
  source /dev/stdin <<<"$(awk '/^build_delta_ledger\(\) \{/,/^\}/' "$RUNNER")"
  # stderr goes to a file rather than /dev/null: the refusal text is expected
  # here (case 2 asserts it), but a DIFFERENT failure would otherwise vanish and
  # leave the assertions reading empty ledgers, which is how an earlier draft of
  # this test reported success while dying at exit 9.
  repo="$repo" scratch="$scratch" LEDGER="$ledger_dir" \
    PHASE_A_BASE="$BASE" INT_SHA="$INT" MAIN_SHA="$MAIN" \
    build_delta_ledger >/dev/null 2>"$work/classifier.err" || true
}

[[ -n "$(awk '/^build_delta_ledger\(\) \{/,/^\}/' "$RUNNER")" ]] ||
  fail "build_delta_ledger is missing from $RUNNER; the classifier moved and this test is blind"

# In a SUBSHELL: the classifier ends by refusing whenever anything is still
# unclassified, and that refusal is exactly what case 2 asserts. Running it
# inline would let its exit kill this test before a single assertion ran. The
# ledger and blocker files are already on disk by then, so the subshell keeps
# the evidence while discarding the exit.
(run_classifier) || true
ledger="$ledger_dir/delta-ledger.tsv"
missing="$ledger_dir/delta-missing.tsv"
[[ -f $ledger ]] || fail "the classifier wrote no ledger"

kind_of() { # <path> -> the classification recorded for it, or empty
  awk -F'\t' -v p="$2" '$2 == p {print $1; exit}' "$1"
}

# 1. Deleted on BOTH sides is agreement, and must never reach the blocker list.
[[ "$(kind_of "$ledger" both-deleted.txt)" == landed-unchanged ]] ||
  fail "a path deleted by both branches was not recorded as agreement; the gate would demand a reason for a non-event"
if [[ -f $missing ]] && grep -q 'both-deleted.txt' "$missing"; then
  fail "a path deleted by both branches still blocks the cutover"
fi

# 2. Deleted by integration but KEPT on main is a real delta and MUST still
#    block. Without this the fix would degrade into treating every absence as
#    agreement, which is the failure the ledger exists to prevent.
grep -q 'int-deleted-kept.txt' "$missing" 2>/dev/null ||
  fail "a path integration deleted while main kept it did NOT block; the fix over-reached into treating any absence as agreement"

# 3. An ordinary matched edit is still agreement, so case 1 is not passing
#    because the classifier now calls everything agreement.
[[ "$(kind_of "$ledger" both-changed.txt)" == landed-unchanged ]] ||
  fail "a path both branches edited identically was not recorded as agreement"

printf 'cutover-gate-deletion-agreement: OK (both-deleted is agreement, one-sided deletion still blocks, survivor unchanged)\n'
