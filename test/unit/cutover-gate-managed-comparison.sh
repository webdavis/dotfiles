#!/usr/bin/env bash
# cutover-gate-managed-comparison.sh: gate 1's deployable-but-untracked check
# must name exactly the files that would deploy without a commit describing
# them, and nothing else.
#
# WHY THIS EXISTS. Measured on dresden 2026-08-04, mid-cutover: gate 1 refused
# with 281 offenders against a source tree that had none, so D1 could not start.
# Two independent defects stacked in one comparison, and each alone is enough to
# make the gate unpassable:
#
#   1. `chezmoi managed` names managed DIRECTORIES; `git ls-files` never names a
#      directory. Every managed directory was therefore an offender no commit
#      could clear. 73 of the 281.
#   2. The two inputs were sorted under LC_ALL=C while `comm` ran under the
#      login locale, so comm judged correctly-sorted input to be unsorted and
#      emitted nonsense. 208 of the 281 were files that ARE tracked and appear
#      verbatim in both lists.
#
# The second is the dangerous one: a gate that names tracked files as
# unclassified-and-deployable trains the operator to disbelieve its refusals,
# which is worse than having no gate. Both cases are pinned below against a
# throwaway repository, so neither can come back.
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

command -v chezmoi >/dev/null 2>&1 ||
  fail "chezmoi is required to exercise the managed-set comparison"

# The function under test, sourced out of the runner rather than reimplemented,
# so a future edit to the runner is what this test sees.
extract_function() {
  awk '/^managed_but_untracked\(\) \{/,/^\}/' "$RUNNER"
}
[[ -n "$(extract_function)" ]] ||
  fail "managed_but_untracked is missing from $RUNNER; the comparison moved and this test is blind"

# A throwaway chezmoi source tree: one tracked file in a managed DIRECTORY (the
# directory is what defect 1 flagged), one tracked file whose sorted position
# differs between the C and en_US collations (defect 2's shape), and one
# genuinely untracked deployable file that the gate MUST still catch.
src="$work/src"
mkdir -p "$src/dot_config/nested"
printf 'tracked\n' >"$src/dot_config/nested/tracked.conf"
printf 'tracked\n' >"$src/dot_Aa_collation_probe"
printf 'tracked\n' >"$src/dot_ab_collation_probe"
git -C "$src" init -q
git -C "$src" add dot_config/nested/tracked.conf dot_Aa_collation_probe dot_ab_collation_probe
git -C "$src" -c user.email=t@t -c user.name=t commit -qm seed

# The sourced function calls die on a chezmoi or git failure. shellcheck cannot
# see that call site (the definition it belongs to arrives at runtime through
# source), so it reads the body as unreachable and the function as uninvoked.
# shellcheck disable=SC2317,SC2329
die() {
  printf 'die: %s\n' "$*" >&2
  exit 9
}

run_comparison() { # prints the offenders the runner's own function reports
  local repo=$1 scratch
  scratch="$(mktemp -d "$work/scratch.XXXXXX")"
  # shellcheck source=/dev/null
  source /dev/stdin <<<"$(extract_function)"
  repo="$repo" scratch="$scratch" managed_but_untracked
}

# 1. A tree whose every deployable file is tracked reports NOTHING. This is the
#    case that was impossible before: directories and collation noise each made
#    it fail.
offenders="$(run_comparison "$src" 2>/dev/null || true)"
[[ -z $offenders ]] ||
  fail "a fully tracked source tree reported offenders, so the gate cannot pass:"$'\n'"$offenders"

# 2. Directories are never offenders. Prove the managed set really did contain
#    one, so case 1 is not passing because the fixture happens to be flat.
managed_all="$(chezmoi managed --source "$src" --path-style source-relative 2>/dev/null || true)"
grep -qx 'dot_config/nested' <<<"$managed_all" ||
  fail "the fixture no longer exercises a managed directory; case 1 proves nothing"

# 3. A genuinely untracked deployable file IS caught. Without this the gate
#    could be made to pass by never reporting anything, which is the failure
#    mode the whole check exists to prevent.
printf 'unclassified\n' >"$src/dot_config/nested/stowaway.conf"
offenders="$(run_comparison "$src" 2>/dev/null || true)"
grep -qx 'dot_config/nested/stowaway.conf' <<<"$offenders" ||
  fail "an untracked deployable file was NOT reported; the gate would pass a source tree carrying it"
rm -f "$src/dot_config/nested/stowaway.conf"

printf 'cutover-gate-managed-comparison: OK (clean tree silent, directories exempt, stowaway caught)\n'
