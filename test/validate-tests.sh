#!/usr/bin/env bash
# validate-tests.sh [root] -- placement / mode / symlink guard for the test suites.
# A dependency of every test recipe (root defaults to `test`). Root is an
# argument so the test-system suite can point it at scratch trees. It fails when
# a test file cannot be seen by, or could escape, the gate:
#
#   - a *.sh OR *.bats not sitting DIRECTLY in a recognized suite
#     (test/unit, test/integration, test/e2e, test/test-system); a suite's
#     helpers/ AND the shared, cross-suite test/helpers/ may hold only
#     NON-executable *.sh (sourced libs; an executable file there is a misplaced
#     test, and bats never belong there); test/fixtures/** is exempt; only
#     validate-tests.sh and run-test-suite.sh may sit at test/ root;
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
# does not follow symlinks without -L), catching a symlinked suite dir too.
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
      # fixtures/ holds data and sourced libs, and nothing ever runs bats from
      # there, so a *.bats under fixtures/ is a test that would never run.
      "$root"/fixtures/*.bats)
        bad+="$file (bats never run from fixtures/; move it into a suite)"$'\n'
        ;;
      "$root"/fixtures/*) continue ;;
      # A suite's helpers/ holds sourced, non-executable *.sh only. An
      # executable file there is a misplaced test that no runner would ever
      # discover, and bats never belong there, so both fail the guard.
      "$root"/unit/helpers/*.sh | "$root"/integration/helpers/*.sh | "$root"/e2e/helpers/*.sh | "$root"/test-system/helpers/*.sh)
        if [[ -x $file ]]; then
          bad+="$file (helpers are sourced, not executed; remove the executable bit, or move the test into its suite)"$'\n'
        fi
        ;;
      "$root"/unit/helpers/* | "$root"/integration/helpers/* | "$root"/e2e/helpers/* | "$root"/test-system/helpers/*)
        bad+="$file (only sourced *.sh belong in a suite's helpers/)"$'\n'
        ;;
      # The shared, cross-suite test/helpers/ dir (sourced libs used by more than
      # one suite) follows the same rule as a suite's helpers/: sourced,
      # non-executable *.sh only. An executable file there is a misplaced test no
      # runner discovers, and bats never belong there, so both fail the guard.
      "$root"/helpers/*.sh)
        if [[ -x $file ]]; then
          bad+="$file (helpers are sourced, not executed; remove the executable bit, or move the test into a suite)"$'\n'
        fi
        ;;
      "$root"/helpers/*)
        bad+="$file (only sourced *.sh belong in test/helpers/)"$'\n'
        ;;
      # The control scripts allowed to sit at test/ root, run by just, never
      # discovered as tests.
      "$root"/validate-tests.sh | "$root"/run-test-suite.sh) continue ;;
      "$root"/unit/*/* | "$root"/integration/*/* | "$root"/e2e/*/* | "$root"/test-system/*/*)
        bad+="$file (nested; suites are flat)"$'\n'
        ;;
      "$root"/unit/*.sh | "$root"/integration/*.sh | "$root"/e2e/*.sh | "$root"/test-system/*.sh)
        [[ -x $file ]] || bad+="$file (not executable; run chmod +x on it)"$'\n'
        ;;
      "$root"/unit/*.bats | "$root"/integration/*.bats | "$root"/e2e/*.bats | "$root"/test-system/*.bats)
        :
        ;; # bats live flat in a suite; bats itself runs them (no +x needed)
      *)
        bad+="$file (outside the unit/integration/e2e/test-system suites and not an allowlisted root script)"$'\n'
        ;;
    esac
  done <"$files_list"

  if [[ -n $bad ]]; then
    printf 'FAIL: misplaced or misconfigured test scripts:\n%s' "$bad" >&2
    printf 'Move each into a suite (unit/integration/e2e/test-system) and make suite *.sh executable; a suite test uses REPO_ROOT depth ../.. .\n' >&2
    return 1
  fi
  return 0
}

# BSD-first stat usage. The BSD form of stat (the `-f` variant) tried before
# the GNU form does not fail on Linux (GNU coreutils), or on macOS whenever
# a PATH fronts Homebrew's coreutils gnubin: there that flag means
# "filesystem status", so it succeeds with the wrong output, the GNU form
# never runs, and the caller silently reads garbage (this broke CI twice as a
# `||` chain, then a third time as an `if`-gated probe the chain-only scan
# could not see). The property enforced is ORDER, not one syntax: no control
# flow may try the BSD form before a GNU form (`-c`, `--format`, `--printf`)
# has appeared. Two analyses over raw text (comments and fixture prose count;
# the scan reads raw text on purpose, since examples get copy-pasted), both on
# chain segments (physical lines joined across backslash continuations, then
# split on `;` and `&&`):
#
#   - Per segment: a BSD-form stat sitting in a `||` chain with no GNU-form
#     stat before it in the same segment is FLAGGED (catches a chain masked
#     by an earlier GNU call elsewhere in the file).
#   - Per scope, reading every segment's stat forms in raw-text order (two
#     scopes: the whole file, and afresh from each function-definition line):
#     a BSD form seen before any GNU form is FLAGGED at its line as soon as a
#     GNU form follows in the same scope. This catches `if`-gated probes,
#     `&&`-early-exit probes, a case dispatch with the BSD branch first, and
#     a variable assigned the BSD command before the GNU form appears.
#   - ALLOWED: GNU-first order in any shape, and a bare BSD-form call with no
#     GNU form after it in scope (capability-gated, darwin-only use).
#   - Boundary (documented misses): an unrelated GNU call earlier in the same
#     segment or scope absolves a later BSD-first construct (order, not data
#     flow); runtime-assembled commands whose tokens are declared GNU-first
#     are invisible; dispatch conditions are not understood, so a case
#     listing a GNU branch before a BSD fallback branch passes; function
#     scopes reset at each function-definition line, not at closing braces.
#   - Fail closed: a grep or awk error fails the guard, never a silent pass.
#
# Each rule and boundary here is pinned as a named fixture + assertion in
# test/test-system/stat-order.sh; that test is the authoritative documentation
# and cannot drift. (The guard lives inside its own scan root, so no comment
# here may spell a literal BSD-first sequence; hence this phrasing.)
# find_bsd_first_chains_in_file <file> -- print the line number of every
# BSD-first stat usage in the file, one per line, ascending, deduplicated.
# Returns awk's exit status so the caller can fail closed on a tool error.
#
# awk idiom: awk has no `local`; extra function parameters ARE the locals, and
# the wide gap in each parameter list separates real arguments from those
# locals.
find_bsd_first_chains_in_file() { # <file>
  LC_ALL=C awk '
    BEGIN {
      bsd_regex = "stat[[:space:]]+-f"
      gnu_regex = "stat[[:space:]]+(-c|--(format|printf)[=[:space:]])"
      function_definition_regex = "^[[:space:]]*(function[[:space:]]+)?[A-Za-z_][A-Za-z0-9_]*[[:space:]]*\\(\\)[[:space:]]*\\{"
    }
    function report(line_number) {
      if (line_number in reported) return
      reported[line_number] = 1
      reported_lines[++reported_count] = line_number
    }
    # One stat-form occurrence, in raw-text order, feeding two identical
    # state machines: file scope (never resets) and function scope (reset at
    # each function-definition line). A GNU form flags any pending BSD form
    # and absolves every later one; a BSD form seen before any GNU form
    # becomes the pending line of each scope that has not seen a GNU form.
    function process_occurrence(is_gnu_form, line_number) {
      if (is_gnu_form) {
        if (file_pending) { report(file_pending); file_pending = 0 }
        if (func_pending) { report(func_pending); func_pending = 0 }
        file_gnu_seen = 1
        func_gnu_seen = 1
        return
      }
      if (!file_gnu_seen && !file_pending) file_pending = line_number
      if (!func_gnu_seen && !func_pending) func_pending = line_number
    }
    # Feed the stat forms of one segment to process_occurrence in position
    # order (a segment may hold both forms in either order).
    function scan_segment_forms(segment, line_number,   gnu_position, gnu_length, bsd_position, bsd_length) {
      while (1) {
        gnu_position = 0
        bsd_position = 0
        if (match(segment, gnu_regex)) { gnu_position = RSTART; gnu_length = RLENGTH }
        if (match(segment, bsd_regex)) { bsd_position = RSTART; bsd_length = RLENGTH }
        if (!gnu_position && !bsd_position) return
        if (gnu_position && (!bsd_position || gnu_position < bsd_position)) {
          process_occurrence(1, line_number)
          segment = substr(segment, gnu_position + gnu_length)
        } else {
          process_occurrence(0, line_number)
          segment = substr(segment, bsd_position + bsd_length)
        }
      }
    }
    function flush(   line_copy, segment_count, segment_number, segment, bsd_index, gnu_index) {
      if (joined == "") return
      # Function boundary: a function-definition line starts a fresh function
      # scope, judged on its own even when an earlier function was GNU-first.
      if (joined ~ function_definition_regex) {
        func_gnu_seen = 0
        func_pending = 0
      }
      # Segment split: `;` and `&&` both terminate a chain, so rewrite
      # `&&` to `;` and split the logical line once.
      line_copy = joined
      gsub(/&&/, ";", line_copy)
      segment_count = split(line_copy, segments, ";")
      for (segment_number = 1; segment_number <= segment_count; segment_number++) {
        segment = segments[segment_number]
        # Per-segment chain analysis: a BSD form in a `||` chain with no GNU
        # form earlier in the SAME segment is BSD-first regardless of scope
        # state (an earlier GNU call elsewhere must not mask it).
        bsd_index = match(segment, bsd_regex)
        if (bsd_index > 0 && index(segment, "||") > 0) {
          gnu_index = match(segment, gnu_regex)
          if (!(gnu_index > 0 && gnu_index < bsd_index)) report(start_line)
        }
        # Scope analysis: every stat form feeds the ordered state machines.
        scan_segment_forms(segment, start_line)
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
    END {
      flush()
      for (i = 1; i <= reported_count; i++)
        for (j = i + 1; j <= reported_count; j++)
          if (reported_lines[j] + 0 < reported_lines[i] + 0) {
            swap = reported_lines[i]
            reported_lines[i] = reported_lines[j]
            reported_lines[j] = swap
          }
      for (i = 1; i <= reported_count; i++) print reported_lines[i]
    }
  ' "$1"
}

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
    if ! find_bsd_first_chains_in_file "$scanned_file" >"$chain_lines_list"; then
      printf 'FAIL: stat-chain scan of %s failed (awk error); refusing to pass on a partial scan\n' "$scanned_file" >&2
      return 1
    fi
    while IFS= read -r chain_start_line; do
      bsd_first_chains+="$scanned_file:$chain_start_line"$'\n'
    done <"$chain_lines_list"
  done <"$stat_candidates_list"
  bsd_first_chains="${bsd_first_chains%$'\n'}"

  if [[ -n $bsd_first_chains ]]; then
    printf 'FAIL: BSD-first stat usage(s) below %s/ (the BSD form does not fail under GNU coreutils, it succeeds with garbage; try the GNU -c form first, in any control flow, and fall back to the BSD -f form):\n' "$root" >&2
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
