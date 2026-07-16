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
# fallback CHAIN whose first `stat -f` has no GNU-form stat before it WITHIN the
# same chain segment: the logical line is split into segments on `;` and `&&`
# (both terminate a `||` chain), so an unrelated GNU stat earlier on the line
# (say, a previous command substitution) cannot mask a later BSD-first chain.
# Documented boundary of the approximation: `$( )`, `{ }`, and single-`|`
# transitions are NOT segment boundaries, so a GNU stat and a BSD-first chain
# packed into ONE segment with no `;`/`&&` between them still masks.
# A capability-gated bare `stat -f` with no chain (e.g. a
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
# Fail closed: candidate discovery and per-file matching are CHECKED commands
# into temp files, mirroring the discovery pipeline above. grep exit 1 means
# "no candidates" (a pass); anything above 1 is a tool error and MUST fail the
# guard, never silently yield an empty candidate list and a green pass. An awk
# failure likewise fails the guard instead of vanishing inside a process
# substitution. No `2>/dev/null` -- a real tool error is seen.
stat_candidates_list="$(mktemp)"
chain_lines_list="$(mktemp)"
trap 'rm -f "$links_list" "$files_list" "$stat_candidates_list" "$chain_lines_list"' EXIT

grep_status=0
grep -rlI 'stat -f' "$root" >"$stat_candidates_list" || grep_status=$?
if [[ $grep_status -gt 1 ]]; then
  printf 'FAIL: stat-chain candidate scan of %s/ failed (grep exit %d); refusing to pass on a partial scan\n' "$root" "$grep_status" >&2
  exit 1
fi

bsd_first_chains=""
while IFS= read -r scanned_file; do
  [[ -n $scanned_file ]] || continue
  if ! awk '
    function flush(   line_copy, segment_count, i, segment, bsd_index, gnu_index) {
      if (joined == "") return
      line_copy = joined
      gsub(/&&/, ";", line_copy)
      segment_count = split(line_copy, segments, ";")
      for (i = 1; i <= segment_count; i++) {
        segment = segments[i]
        bsd_index = index(segment, "stat -f")
        if (bsd_index == 0) continue
        if (index(segment, "||") == 0) continue
        gnu_index = index(segment, "stat -c")
        if (!(gnu_index > 0 && gnu_index < bsd_index)) {
          print start_line
          break
        }
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
  ' "$scanned_file" >"$chain_lines_list"; then
    printf 'FAIL: stat-chain scan of %s failed (awk error); refusing to pass on a partial scan\n' "$scanned_file" >&2
    exit 1
  fi
  while IFS= read -r chain_start_line; do
    bsd_first_chains+="$scanned_file:$chain_start_line"$'\n'
  done <"$chain_lines_list"
done <"$stat_candidates_list"
bsd_first_chains="${bsd_first_chains%$'\n'}"

if [[ -n $bsd_first_chains ]]; then
  printf 'FAIL: BSD-first stat fallback chain(s) below %s/ (break on Linux CI; put the GNU -c form first, the BSD -f form as the fallback):\n' "$root" >&2
  printf '%s\n' "$bsd_first_chains" | sed 's/^/  /' >&2
  exit 1
fi
