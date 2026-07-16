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

# BSD-first stat fallback chains. `stat -f ... || stat -c ...` runs the BSD form
# first; on Linux CI (GNU coreutils) `stat -f` means "filesystem status" and
# SUCCEEDS with the wrong output, so the `|| stat -c` fallback never fires and the
# test silently reads garbage. Two CI failures (PRs #49, #50) came from exactly
# this. The portable idiom is GNU-first: `stat -c ... || stat -f ...`. Flag a
# fallback CHAIN (a logical line containing `||`) whose first `stat -f` has no
# `stat -c` before it. A capability-gated bare `stat -f` with no chain (e.g. a
# `find -exec stat -f` in a GNU-probed else-branch) is not a fallback chain and is
# left alone. Scans every text file below root (fixtures included) since a sourced
# lib carries the same trap.
#
# FX11: a chain split across a backslash continuation (the `||` on the next physical
# line) slipped past a per-physical-line scan: line 1 held `stat -f` but no `||`,
# line 2 held `||` but no `stat -f` (and a GNU-first chain split the same way
# false-POSITIVED on the `|| stat -f` continuation line). Join backslash
# continuations into one logical line, keyed by the starting physical line number,
# before matching.
bsd_first_chains=""
while IFS= read -r scanned_file; do
  [[ -n $scanned_file ]] || continue
  while IFS= read -r chain_start_line; do
    bsd_first_chains+="$scanned_file:$chain_start_line"$'\n'
  done < <(awk '
    function flush(   bsd_index, gnu_index) {
      if (joined == "") return
      bsd_index = index(joined, "stat -f")
      if (bsd_index > 0) {
        gnu_index = index(joined, "stat -c")
        if (!(gnu_index > 0 && gnu_index < bsd_index) && index(joined, "||") > 0) print start_line
      }
      joined = ""
    }
    {
      if (joined == "") start_line = NR
      line = $0
      if (line ~ /\\[ \t]*$/) { sub(/\\[ \t]*$/, " ", line); joined = joined line; next }
      joined = joined line
      flush()
    }
    END { flush() }
  ' "$scanned_file")
done < <(grep -rlI 'stat -f' "$root" 2>/dev/null || true)
bsd_first_chains="${bsd_first_chains%$'\n'}"

if [[ -n $bsd_first_chains ]]; then
  printf 'FAIL: BSD-first stat fallback chain(s) below %s/ (break on Linux CI; put the GNU -c form first, the BSD -f form as the fallback):\n' "$root" >&2
  printf '%s\n' "$bsd_first_chains" | sed 's/^/  /' >&2
  exit 1
fi
