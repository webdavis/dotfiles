#!/usr/bin/env bash
# run-test-suite.sh [--shuffle[=seed]] [--warn-slow-ms N] [--only-bashunit]
# <suite-dir> -- run one test suite: its bashunit `<name>.test.sh` files, then
# its executable *.sh tests. Shared by every test recipe so the
# checked-discovery and fd-closing rules below live in ONE place.
#
# Two correctness rules the gate depends on:
#
#   1. Discovery must be able to FAIL the gate. A `find ... | sort` inside a
#      process substitution cannot propagate its exit status, so a traversal or
#      sort error would yield a short list and a GREEN gate with tests silently
#      omitted. Discovery runs as a CHECKED foreground pipeline (pipefail is on
#      via `set -o pipefail`) into a temp file; its status is verified before
#      the list is read. No `2>/dev/null` on discovery -- a real error must be
#      seen, not swallowed.
#
#   2. Every child test is invoked with fd 3 CLOSED (`"$t" 3<&-`). The loop
#      streams the discovery list on fd 3; a test that reads fd 3 (directly, or
#      by inheriting it) would drain the remaining entries and silently truncate
#      the suite. Closing fd 3 for the child severs that path.
#
# Options (the unit suite uses both; the other suites run plain):
#
#   --shuffle[=seed]   randomize the *.sh order to flush hidden ordering
#                      dependence; the seed is printed so a failure replays with
#                      TEST_SEED=<seed>.
#   --warn-slow-ms N   print a WARN-ONLY summary of *.sh tests over N ms; the
#                      warnings never fail the run.
#   --only-bashunit    run the bashunit lane alone and stop, for focused
#                      iteration through `just test-bashunit`.
#
# TEST_SEED and UNIT_WARN_MS still work as env fallbacks; a flag wins over the
# matching env var. Timing uses bash's built-in EPOCHREALTIME (no external
# process); the shuffle uses gshuf/shuf when present and degrades to sorted
# order otherwise.
set -euo pipefail

usage='usage: run-test-suite.sh [--shuffle[=seed]] [--warn-slow-ms N] [--only-bashunit] <suite-dir>'

# Parsed by parse_args; declared here so every function can see them.
shuffle=0
seed=""
warn=0
warn_ms=""
only_bashunit=0
suite_directory=""
status=0
any_sh=0
any_bashunit=0
slow=()
# Set in main; global so the EXIT trap can still see it after main returns.
workdir=""

die_usage() {
  printf '%s\n' "$usage" >&2
  exit 2
}

# Option values that feed arithmetic or the seed must be plain unsigned decimal
# integers: anything else (an expression, a negative, an empty string, another
# flag) is a usage error. Checked at parse time, before any test runs, because
# a crafted value like "status=0" would otherwise be evaluated as bash
# arithmetic and could flip a failing suite to exit 0.
require_unsigned_integer() { # <value>
  [[ $1 =~ ^[0-9]+$ ]] || die_usage
}

# Milliseconds since epoch from EPOCHREALTIME ("seconds<sep>microseconds"):
# strip the separator (a dot, or a comma in some locales; stripping every
# non-digit avoids exporting LC_ALL=C, which would leak into child tests) to
# get integer microseconds, divide by 1000. No external process.
now_ms() {
  local r="${EPOCHREALTIME//[!0-9]/}"
  printf '%s' "$((r / 1000))"
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --shuffle) shuffle=1 ;;
      --shuffle=*)
        shuffle=1
        seed="${1#*=}"
        require_unsigned_integer "$seed"
        ;;
      --warn-slow-ms)
        shift
        [[ $# -gt 0 ]] || die_usage
        warn=1
        warn_ms="$1"
        require_unsigned_integer "$warn_ms"
        ;;
      --warn-slow-ms=*)
        warn=1
        warn_ms="${1#*=}"
        require_unsigned_integer "$warn_ms"
        ;;
      --only-bashunit) only_bashunit=1 ;;
      -*) die_usage ;;
      *)
        [[ -z $suite_directory ]] || die_usage
        suite_directory="$1"
        ;;
    esac
    shift
  done

  # Env fallbacks (a flag already set above wins). An empty env var is ignored.
  if [[ $shuffle -eq 0 && -n ${TEST_SEED:-} ]]; then
    shuffle=1
  fi
  if [[ $shuffle -eq 1 && -z $seed ]]; then
    seed="${TEST_SEED:-${RANDOM}${RANDOM}}"
    require_unsigned_integer "$seed"
  fi
  if [[ $warn -eq 0 && -n ${UNIT_WARN_MS:-} ]]; then
    warn=1
    warn_ms="${UNIT_WARN_MS}"
    require_unsigned_integer "$warn_ms"
  fi
  [[ -n $warn_ms ]] || warn_ms=200
  # Force base 10 so a leading zero (e.g. 08) is not read as broken octal by
  # bash arithmetic later. The SEED is deliberately not normalized: it feeds
  # `yes "$seed"` and the printed replay line as the same validated digit
  # string, and bash arithmetic on it would wrap values above the signed
  # 64-bit range into a negative number the validator rejects on replay.
  warn_ms=$((10#$warn_ms))
}

# Checked discovery of one file kind into <outfile>. The find/sort pipeline runs
# with pipefail on, so a traversal or sort error is the function's exit status.
# LC_ALL=C is scoped to the sort only (deterministic ordering for seed replay)
# so it never leaks into child tests.
discover_tests() { # <suite_directory> <outfile> <find-args...>
  local suite_directory="$1" outfile="$2"
  shift 2
  find "$suite_directory" -maxdepth 1 -type f "$@" -print0 | LC_ALL=C sort -z >"$outfile"
}

# Run the suite's *.sh tests from <sh_list_file>: optional seeded shuffle, then
# each test with fd 3 closed and (when warn is on) timed.
run_sh_tests() { # <sh_list_file>
  local sh_list="$1"
  if [[ $shuffle -eq 1 ]]; then
    printf 'suite tests: seed=%s (replay with TEST_SEED=%s)\n' "$seed" "$seed"
    local shuf_bin="" cand
    for cand in gshuf shuf; do
      command -v "$cand" >/dev/null 2>&1 && {
        shuf_bin="$cand"
        break
      }
    done
    if [[ -n $shuf_bin ]]; then
      local shuffled="$sh_list.shuffled"
      if "$shuf_bin" -z --random-source=<(yes "$seed") <"$sh_list" >"$shuffled" 2>/dev/null; then
        # A NUL-delimited stream must end with a NUL byte. Checked BEFORE the
        # sorted comparison below: sort restores the missing terminator, so an
        # unterminated final record would compare equal, yet the read loop
        # drops it, silently skipping the last test.
        if [[ -s $shuffled ]]; then
          local final_byte_value
          final_byte_value="$(tail -c 1 "$shuffled" | od -An -tu1 | tr -d ' \n')"
          if [[ $final_byte_value != 0 ]]; then
            printf 'FAIL: shuffle corrupted the test list (%s output is not NUL-terminated); refusing to run\n' "$shuf_bin" >&2
            exit 1
          fi
        fi
        # A reordering must not add, drop, or alter entries. Compare the sorted
        # shuffled list against the sorted discovery list; a mismatch means the
        # shuffler corrupted the list (an empty list once green-gated a failing
        # suite as "no tests found"), and a broken shuffler is a broken gate.
        if ! LC_ALL=C sort -z <"$sh_list" >"$sh_list.expected" ||
          ! LC_ALL=C sort -z <"$shuffled" >"$sh_list.actual" ||
          ! cmp -s "$sh_list.expected" "$sh_list.actual"; then
          printf 'FAIL: shuffle corrupted the test list (%s output does not match discovery); refusing to run\n' "$shuf_bin" >&2
          exit 1
        fi
        mv "$shuffled" "$sh_list"
      else
        rm -f "$shuffled"
      fi
    fi
  fi

  local t start ms
  while IFS= read -r -u3 -d '' t; do
    any_sh=1
    printf '== %s ==\n' "$t"
    start=0
    [[ $warn -eq 1 ]] && start="$(now_ms)"
    if ! "$t" 3<&-; then
      printf '== FAIL: %s ==\n' "$t"
      status=1
    fi
    if [[ $warn -eq 1 ]]; then
      ms=$(($(now_ms) - start))
      ((ms > warn_ms)) && slow+=("$(printf '%6dms  %s' "$ms" "$t")")
    fi
  done 3<"$sh_list"
  # Return success explicitly: the loop body's last command is an arithmetic
  # test that is false (exit 1) whenever the final test is under the threshold,
  # which would otherwise make this function (and `set -e`) treat the run as
  # failed.
  return 0
}

# Run the suite's bashunit files from <bashunit_list_file>.
#
# The list is an EXACT, suite-local set of `*.test.sh` paths rather than a
# directory handed to bashunit, and that is the whole point of running the lane
# from here. bashunit's own path argument makes it scan RECURSIVELY for
# `*[tT]est.sh` plus a `.bash` twin (bashunit::helper::find_files_recursive at
# 0.50.1), which reaches three things it must not: a fixture named `latest.sh`,
# an executable `contest.sh` that the *.sh lane below ALSO runs, and every other
# suite's files, so an integration test would run under the unit gate and be
# missed by its own recipe. Discovery belongs where it is checked and bounded to
# one flat suite, which is here.
run_bashunit_files() { # <bashunit_list_file>
  local bashunit_list="$1"
  local bashunit_files=() f
  while IFS= read -r -d '' f; do
    bashunit_files+=("$f")
  done <"$bashunit_list"
  ((${#bashunit_files[@]} > 0)) || return 0
  any_bashunit=1

  printf '== bashunit (%s) ==\n' "$suite_directory"
  if ! command -v bashunit >/dev/null 2>&1; then
    printf 'FAIL: bashunit is not installed; run "brew install bashunit" (it is declared in .chezmoidata/system_packages_autoinstall.yaml)\n' >&2
    status=1
    return
  fi
  # NO_COLOR, not --no-color: the flag is ignored in either position on 0.50.1.
  NO_COLOR=1 bashunit "${bashunit_files[@]}" || status=1
}

# The suite directory must be given and must exist before anything runs.
require_suite_directory() {
  [[ -n $suite_directory ]] || die_usage
  if [[ ! -d $suite_directory ]]; then
    printf 'FAIL: suite dir %s does not exist\n' "$suite_directory" >&2
    exit 1
  fi
}

discover_sh_tests() { # <outfile>
  if ! discover_tests "$suite_directory" "$1" -name '*.sh' -perm -u+x; then
    printf 'FAIL: %s .sh discovery failed; refusing to run a partial list\n' "$suite_directory" >&2
    exit 1
  fi
}

# `*.test.sh` exactly, and only the ones sitting flat in THIS suite. Note this
# runs before the *.sh discovery below and matches a strict subset of it, except
# that a bashunit file is never executable, so the two lanes cannot both claim
# the same file: test/validate-tests.sh is what pins that mode rule.
discover_bashunit_tests() { # <outfile>
  if ! discover_tests "$suite_directory" "$1" -name '*.test.sh'; then
    printf 'FAIL: %s .test.sh discovery failed; refusing to run a partial list\n' "$suite_directory" >&2
    exit 1
  fi
}

# An empty suite is a green no-op: say so and stop.
exit_zero_if_no_tests_found() {
  if [[ $any_sh -eq 0 && $any_bashunit -eq 0 ]]; then
    printf 'no tests found in %s\n' "$suite_directory"
    exit 0
  fi
}

print_slow_test_summary() {
  if [[ $warn -eq 1 && ${#slow[@]} -gt 0 ]]; then
    printf '\nPERFORMANCE WARNING: tests over %sms (refactor, or move to integration/e2e):\n' "$warn_ms"
    printf '  %s\n' "${slow[@]}"
  fi
}

main() {
  parse_args "$@"
  require_suite_directory

  workdir="$(mktemp -d)"
  trap 'rm -rf "$workdir"' EXIT
  local sh_list="$workdir/sh" bashunit_list="$workdir/bashunit"

  discover_bashunit_tests "$bashunit_list"
  run_bashunit_files "$bashunit_list"
  if [[ $only_bashunit -eq 1 ]]; then
    [[ $any_bashunit -eq 1 ]] || printf 'no bashunit tests found in %s\n' "$suite_directory"
    exit "$status"
  fi

  discover_sh_tests "$sh_list"
  run_sh_tests "$sh_list"
  exit_zero_if_no_tests_found
  print_slow_test_summary

  exit "$status"
}

main "$@"
