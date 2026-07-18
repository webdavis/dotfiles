#!/usr/bin/env bash
# validate-tests.sh [root] -- placement / mode / symlink guard for the test pyramid.
# A dependency of every test recipe (root defaults to `test`). Root is an
# argument so the test-system suite can point it at scratch trees. It fails when
# a test file cannot be seen by, or could escape, the gate:
#
#   - a *.sh OR *.bats not sitting DIRECTLY in a recognized suite
#     (test/unit, test/integration, test/e2e, test/test-system); a suite's
#     helpers/ and test/fixtures/** are exempt (sourced libs and fixture data,
#     never run directly); only validate-tests.sh and run-test-suite.sh may
#     sit at test/ root;
#   - a suite *.sh that is not executable (invisible to the runner's -perm probe);
#   - ANY symlink below test/. A physical `find -type f` skips symlinked files
#     and symlinked suite dirs, so a tracked symlink would evade this guard and
#     every gate. Following it risks out-of-tree traversal and cycles, so the
#     guard REJECTS symlinks rather than resolving them.
#
# Discovery is a CHECKED foreground pipeline (pipefail on) into a temp file, not
# a process substitution: a traversal or sort error must FAIL the guard, never
# yield a short list and a green pass. No `2>/dev/null` -- a real error is seen.
set -euo pipefail

# Symlink rejection. `-type l` matches symlinked files AND symlinked dirs (find
# does not follow symlinks without -L), catching a symlinked camp dir too.
check_symlinks() { # <root> <workdir>
  local root="$1"
  local symlink_paths_list="$2/symlinks"
  if ! find "$root" -type l -print0 >"$symlink_paths_list"; then
    printf 'FAIL: symlink scan of %s/ failed\n' "$root" >&2
    return 1
  fi
  if [[ -s $symlink_paths_list ]]; then
    printf 'FAIL: symlinks are not allowed below %s/ (out-of-tree traversal / cycle risk); remove:\n' "$root" >&2
    while IFS= read -r -d '' link; do
      printf '  %s\n' "$link" >&2
    done <"$symlink_paths_list"
    return 1
  fi
  return 0
}

# Placement / mode: every discovered *.sh and *.bats must sit directly in a
# recognized suite (or be an exempt helper / fixture / allowlisted root script),
# and each suite *.sh must be executable.
check_placement() { # <root> <workdir>
  local root="$1"
  local files_list="$2/files"
  if ! find "$root" -type f \( -name '*.sh' -o -name '*.bats' \) -print0 | sort -z >"$files_list"; then
    printf 'FAIL: test discovery failed; refusing to pass on a partial list\n' >&2
    return 1
  fi

  local bad="" file
  while IFS= read -r -d '' file; do
    case "$file" in
      "$root"/fixtures/*) continue ;;
      # A suite's helpers/ holds sourced, non-executable scripts, never run
      # directly, so it is exempt like fixtures/.
      "$root"/unit/helpers/* | "$root"/integration/helpers/* | "$root"/e2e/helpers/* | "$root"/test-system/helpers/*) continue ;;
      # The control scripts allowed to sit at test/ root, run by just, never
      # discovered as tests.
      "$root"/validate-tests.sh | "$root"/run-test-suite.sh) continue ;;
      "$root"/unit/*/* | "$root"/integration/*/* | "$root"/e2e/*/* | "$root"/test-system/*/*)
        bad+="$file (nested; camps are flat)"$'\n'
        ;;
      "$root"/unit/*.sh | "$root"/integration/*.sh | "$root"/e2e/*.sh | "$root"/test-system/*.sh)
        [[ -x $file ]] || bad+="$file (not executable; invisible to the gate)"$'\n'
        ;;
      "$root"/unit/*.bats | "$root"/integration/*.bats | "$root"/e2e/*.bats | "$root"/test-system/*.bats)
        :
        ;; # bats live flat in a camp; bats itself runs them (no +x needed)
      *)
        bad+="$file (outside the unit/integration/e2e/test-system suites and not an allowlisted root script)"$'\n'
        ;;
    esac
  done <"$files_list"

  if [[ -n $bad ]]; then
    printf 'FAIL: misplaced or misconfigured test scripts:\n%s' "$bad" >&2
    printf 'Fix placement/mode (and REPO_ROOT depth is ../.. inside a camp).\n' >&2
    return 1
  fi
  return 0
}

# BSD-first stat fallback chains. The BSD form of stat (the `-f` variant) placed
# first in a fallback chain does not fail on Linux (GNU coreutils), where that
# flag means "filesystem status": it succeeds with the wrong output, the GNU
# fallback never fires, and the caller silently reads garbage (this broke CI
# twice). The scan's contract, applied per chain segment (physical lines joined
# across backslash continuations, then split on `;` and `&&`):
#
#   - FLAGGED: a segment whose BSD-form stat sits in a `||` chain with no
#     GNU-form stat (`-c`, `--format`, `--printf`) before it. Comments and
#     fixture prose count; the scan reads raw text on purpose (copy-paste risk).
#   - ALLOWED: GNU-first chains, and capability-gated bare BSD-form calls.
#   - Boundary: literal chains in raw text only. Runtime-assembled chains and a
#     masking GNU call inside the same unseparated segment are out of scope.
#   - Fail closed: a grep or awk error fails the guard, never a silent pass.
#
# Each rule and boundary here is pinned as a named fixture + assertion in
# test/test-system/stat-order.sh; that test is the authoritative documentation
# and cannot drift. (The guard lives inside its own scan root, so no comment
# here may spell a literal BSD-first chain; hence this phrasing.)
check_stat_order() { # <root> <workdir>
  local root="$1"
  local stat_candidates_list="$2/stat-candidates"
  local chain_lines_list="$2/chain-lines"

  # Candidate discovery, checked: grep exit 1 means "no candidates" (a pass);
  # anything above 1 is a tool error and fails the guard. Token matching is
  # whitespace-tolerant (a tab or a run of spaces between `stat` and its flag is
  # the same command). LC_ALL=C because grep decides whether a file is binary
  # differently depending on the machine's locale; pinning the locale makes every
  # machine decide identically.
  local grep_status=0
  LC_ALL=C grep -rEIl 'stat[[:space:]]+-f' "$root" >"$stat_candidates_list" || grep_status=$?
  if [[ $grep_status -gt 1 ]]; then
    printf 'FAIL: stat-chain candidate scan of %s/ failed (grep exit %d); refusing to pass on a partial scan\n' "$root" "$grep_status" >&2
    return 1
  fi

  local bsd_first_chains="" scanned_file chain_start_line
  while IFS= read -r scanned_file; do
    [[ -n $scanned_file ]] || continue
    # awk idiom: awk has no `local`; extra function parameters ARE the locals,
    # and the wide gap in flush()'s parameter list separates real arguments
    # (none here) from those locals.
    if ! LC_ALL=C awk '
      function flush(   line_copy, segment_count, segment_number, segment, bsd_index, gnu_index) {
        if (joined == "") return
        # Segment split: `;` and `&&` both terminate a chain, so rewrite
        # `&&` to `;` and split the logical line once.
        line_copy = joined
        gsub(/&&/, ";", line_copy)
        segment_count = split(line_copy, segments, ";")
        for (segment_number = 1; segment_number <= segment_count; segment_number++) {
          segment = segments[segment_number]
          # BSD hit: skip any segment without the BSD form.
          bsd_index = match(segment, /stat[[:space:]]+-f/)
          if (bsd_index == 0) continue
          # Chain check: a bare capability-gated call is not a fallback chain.
          if (index(segment, "||") == 0) continue
          # GNU-before check: a GNU form earlier in the SAME segment means the
          # chain is GNU-first, the portable order.
          gnu_index = match(segment, /stat[[:space:]]+(-c|--(format|printf)[=[:space:]])/)
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
        # Backslash continuation: join into one logical line, keyed to the
        # starting physical line number.
        if (line ~ /\\[ \t]*$/) { sub(/\\[ \t]*$/, " ", line); joined = joined line; next }
        joined = joined line
        flush()
      }
      END { flush() }
    ' "$scanned_file" >"$chain_lines_list"; then
      printf 'FAIL: stat-chain scan of %s failed (awk error); refusing to pass on a partial scan\n' "$scanned_file" >&2
      return 1
    fi
    while IFS= read -r chain_start_line; do
      bsd_first_chains+="$scanned_file:$chain_start_line"$'\n'
    done <"$chain_lines_list"
  done <"$stat_candidates_list"
  bsd_first_chains="${bsd_first_chains%$'\n'}"

  if [[ -n $bsd_first_chains ]]; then
    printf 'FAIL: BSD-first stat fallback chain(s) below %s/ (break on Linux CI; put the GNU -c form first, the BSD -f form as the fallback):\n' "$root" >&2
    printf '%s\n' "$bsd_first_chains" | sed 's/^/  /' >&2
    return 1
  fi
  return 0
}

# Set in main; global so the EXIT trap can still see it after main returns.
workdir=""

main() {
  local root="${1:-test}"
  workdir="$(mktemp -d)"
  trap 'rm -rf "$workdir"' EXIT
  check_symlinks "$root" "$workdir" || exit 1
  check_placement "$root" "$workdir" || exit 1
  check_stat_order "$root" "$workdir" || exit 1
}

main "$@"
