#!/usr/bin/env bash
# test-guard.sh [root] -- placement / mode / symlink guard for the test pyramid.
# A dependency of every test recipe (root defaults to `test`). It fails when a
# test file cannot be seen by, or could escape, the gate:
#
#   - a *.sh OR *.bats not sitting DIRECTLY in a recognized camp
#     (test/unit, test/integration, test/e2e); test/fixtures/** is exempt
#     (fixture data and sourced libs, never run directly);
#   - a camp *.sh that is not executable (invisible to the runner's -perm probe);
#   - ANY symlink below test/. A physical `find -type f` skips symlinked files
#     and symlinked camp dirs, so a tracked symlink would evade this guard and
#     every gate. Following it risks out-of-tree traversal and cycles, so the
#     guard REJECTS symlinks rather than resolving them.
#
# Discovery is a CHECKED foreground pipeline (pipefail on) into a temp file, not
# a process substitution: a traversal or sort error must FAIL the guard, never
# yield a short list and a green pass. No `2>/dev/null` -- a real error is seen.
set -euo pipefail

root="${1:-test}"

links_list="$(mktemp)"
files_list="$(mktemp)"
trap 'rm -f "$links_list" "$files_list"' EXIT

# Symlink rejection first. `-type l` matches symlinked files AND symlinked dirs
# (find does not follow symlinks without -L), catching a symlinked camp dir too.
if ! find "$root" -type l -print0 >"$links_list"; then
  printf 'FAIL: symlink scan of %s/ failed\n' "$root" >&2
  exit 1
fi
if [[ -s $links_list ]]; then
  printf 'FAIL: symlinks are not allowed below %s/ (out-of-tree traversal / cycle risk); remove:\n' "$root" >&2
  while IFS= read -r -d '' link; do
    printf '  %s\n' "$link" >&2
  done <"$links_list"
  exit 1
fi

if ! find "$root" -type f \( -name '*.sh' -o -name '*.bats' \) -print0 | sort -z >"$files_list"; then
  printf 'FAIL: test discovery failed; refusing to pass on a partial list\n' >&2
  exit 1
fi

bad=""
while IFS= read -r -d '' f; do
  case "$f" in
    "$root"/fixtures/*) continue ;;
    "$root"/unit/*/* | "$root"/integration/*/* | "$root"/e2e/*/*)
      bad+="$f (nested; camps are flat)"$'\n'
      ;;
    "$root"/unit/*.sh | "$root"/integration/*.sh | "$root"/e2e/*.sh)
      [[ -x $f ]] || bad+="$f (not executable; invisible to the gate)"$'\n'
      ;;
    "$root"/unit/*.bats | "$root"/integration/*.bats | "$root"/e2e/*.bats)
      :
      ;; # bats live flat in a camp; bats itself runs them (no +x needed)
    *)
      bad+="$f (outside the unit/integration/e2e camps)"$'\n'
      ;;
  esac
done <"$files_list"

if [[ -n $bad ]]; then
  printf 'FAIL: misplaced or misconfigured test scripts:\n%s' "$bad" >&2
  printf 'Fix placement/mode (and REPO_ROOT depth is ../.. inside a camp).\n' >&2
  exit 1
fi
