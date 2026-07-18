#!/usr/bin/env bash
# run-test-suite.sh [--shuffle[=seed]] [--warn-slow-ms N] <suite-dir> -- run one
# test suite: its executable *.sh tests, then its *.bats suites. Shared by every
# test recipe so the checked-discovery and fd-closing rules below live in ONE
# place.
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
#                      TEST_SEED=<seed>. (Bats 1.11 has no built-in shuffle.)
#   --warn-slow-ms N   print a WARN-ONLY summary of *.sh tests over N ms; the
#                      warnings never fail the run.
#
# TEST_SEED and UNIT_WARN_MS still work as env fallbacks; a flag wins over the
# matching env var. Timing uses bash's built-in EPOCHREALTIME (no external
# process); the shuffle uses gshuf/shuf when present and degrades to sorted
# order otherwise.
set -euo pipefail
export LC_ALL=C # force EPOCHREALTIME to use a '.' decimal separator

usage='usage: run-test-suite.sh [--shuffle[=seed]] [--warn-slow-ms N] <suite-dir>'

# Parsed by parse_args; declared here so every function can see them.
shuffle=0
seed=""
warn=0
warn_ms=""
camp=""
status=0
any_sh=0
slow=()
# Set in main; global so the EXIT trap can still see it after main returns.
workdir=""

die_usage() {
  printf '%s\n' "$usage" >&2
  exit 2
}

# Milliseconds since epoch from EPOCHREALTIME ("seconds.microseconds"): drop the
# dot to get integer microseconds, divide by 1000. No external process.
now_ms() {
  local r="${EPOCHREALTIME/./}"
  printf '%s' "$((r / 1000))"
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --shuffle) shuffle=1 ;;
      --shuffle=*)
        shuffle=1
        seed="${1#*=}"
        ;;
      --warn-slow-ms)
        shift
        [[ $# -gt 0 ]] || die_usage
        warn=1
        warn_ms="$1"
        ;;
      --warn-slow-ms=*)
        warn=1
        warn_ms="${1#*=}"
        ;;
      -*) die_usage ;;
      *)
        [[ -z $camp ]] || die_usage
        camp="$1"
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
  fi
  if [[ $warn -eq 0 && -n ${UNIT_WARN_MS:-} ]]; then
    warn=1
    warn_ms="${UNIT_WARN_MS}"
  fi
  [[ -n $warn_ms ]] || warn_ms=200
}

# Checked discovery of one file kind into <outfile>. The find/sort pipeline runs
# with pipefail on, so a traversal or sort error is the function's exit status.
discover_tests() { # <camp> <outfile> <find-args...>
  local camp="$1" outfile="$2"
  shift 2
  find "$camp" -maxdepth 1 -type f "$@" -print0 | sort -z >"$outfile"
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

# Run the suite's *.bats suites from <bats_list_file>.
run_bats_suites() { # <bats_list_file>
  local bats_list="$1"
  local bats_files=() b
  while IFS= read -r -d '' b; do
    bats_files+=("$b")
  done <"$bats_list"
  ((${#bats_files[@]} > 0)) || return 0

  printf '== bats (%s) ==\n' "$camp"
  # bats --jobs needs GNU parallel; the flake provides both. On a host without
  # bats (the usual case -- `just test` runs on the host), fall back into the
  # Nix devshell, mirroring the aggregate `test` recipe.
  if command -v bats >/dev/null 2>&1; then
    bats --jobs 4 "${bats_files[@]}" || status=1
  else
    nix develop .#run --command bats --jobs 4 "${bats_files[@]}" || status=1
  fi
}

main() {
  parse_args "$@"
  [[ -n $camp ]] || die_usage
  if [[ ! -d $camp ]]; then
    printf 'FAIL: suite dir %s does not exist\n' "$camp" >&2
    exit 1
  fi

  workdir="$(mktemp -d)"
  trap 'rm -rf "$workdir"' EXIT
  local sh_list="$workdir/sh" bats_list="$workdir/bats"

  if ! discover_tests "$camp" "$sh_list" -name '*.sh' -perm -u+x; then
    printf 'FAIL: %s .sh discovery failed; refusing to run a partial list\n' "$camp" >&2
    exit 1
  fi
  run_sh_tests "$sh_list"

  if ! discover_tests "$camp" "$bats_list" -name '*.bats'; then
    printf 'FAIL: %s .bats discovery failed; refusing to skip a partial list\n' "$camp" >&2
    exit 1
  fi
  local bats_files=() b
  while IFS= read -r -d '' b; do
    bats_files+=("$b")
  done <"$bats_list"

  if [[ $any_sh -eq 0 && ${#bats_files[@]} -eq 0 ]]; then
    printf 'no tests found in %s\n' "$camp"
    exit 0
  fi

  run_bats_suites "$bats_list"

  if [[ $warn -eq 1 && ${#slow[@]} -gt 0 ]]; then
    printf '\nPERFORMANCE WARNING: tests over %sms (refactor, or move to integration/e2e):\n' "$warn_ms"
    printf '  %s\n' "${slow[@]}"
  fi

  exit "$status"
}

main "$@"
