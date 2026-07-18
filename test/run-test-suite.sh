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

shuffle=0
seed=""
warn=0
warn_ms=""
camp=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --shuffle) shuffle=1 ;;
    --shuffle=*)
      shuffle=1
      seed="${1#*=}"
      ;;
    --warn-slow-ms)
      shift
      [[ $# -gt 0 ]] || {
        printf '%s\n' "$usage" >&2
        exit 2
      }
      warn=1
      warn_ms="$1"
      ;;
    --warn-slow-ms=*)
      warn=1
      warn_ms="${1#*=}"
      ;;
    -*)
      printf '%s\n' "$usage" >&2
      exit 2
      ;;
    *)
      [[ -z $camp ]] || {
        printf '%s\n' "$usage" >&2
        exit 2
      }
      camp="$1"
      ;;
  esac
  shift
done

[[ -n $camp ]] || {
  printf '%s\n' "$usage" >&2
  exit 2
}
if [[ ! -d $camp ]]; then
  printf 'FAIL: suite dir %s does not exist\n' "$camp" >&2
  exit 1
fi

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

# Milliseconds since epoch from EPOCHREALTIME ("seconds.microseconds"): drop the
# dot to get integer microseconds, divide by 1000. No external process.
now_ms() {
  local r="${EPOCHREALTIME/./}"
  printf '%s' "$((r / 1000))"
}

sh_list="$(mktemp)"
bats_list="$(mktemp)"
trap 'rm -f "$sh_list" "$bats_list"' EXIT

if ! find "$camp" -maxdepth 1 -type f -name '*.sh' -perm -u+x -print0 | sort -z >"$sh_list"; then
  printf 'FAIL: %s .sh discovery failed; refusing to run a partial list\n' "$camp" >&2
  exit 1
fi

# Seeded shuffle when requested and a shuf is available; otherwise keep the
# sorted order (reproducible, just not randomized).
if [[ $shuffle -eq 1 ]]; then
  printf 'suite tests: seed=%s (replay with TEST_SEED=%s)\n' "$seed" "$seed"
  shuf_bin=""
  for cand in gshuf shuf; do
    command -v "$cand" >/dev/null 2>&1 && {
      shuf_bin="$cand"
      break
    }
  done
  if [[ -n $shuf_bin ]]; then
    shuffled="$(mktemp)"
    if "$shuf_bin" -z --random-source=<(yes "$seed") <"$sh_list" >"$shuffled" 2>/dev/null; then
      mv "$shuffled" "$sh_list"
    else
      rm -f "$shuffled"
    fi
  fi
fi

status=0
slow=()
any_sh=0
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

if ! find "$camp" -maxdepth 1 -type f -name '*.bats' -print0 | sort -z >"$bats_list"; then
  printf 'FAIL: %s .bats discovery failed; refusing to skip a partial list\n' "$camp" >&2
  exit 1
fi
bats_files=()
while IFS= read -r -d '' b; do
  bats_files+=("$b")
done <"$bats_list"

if [[ $any_sh -eq 0 && ${#bats_files[@]} -eq 0 ]]; then
  printf 'no tests found in %s\n' "$camp"
  exit 0
fi

if ((${#bats_files[@]} > 0)); then
  printf '== bats (%s) ==\n' "$camp"
  # bats --jobs needs GNU parallel; the flake provides both. On a host without
  # bats (the usual case -- `just test` runs on the host), fall back into the
  # Nix devshell, mirroring the aggregate `test` recipe.
  if command -v bats >/dev/null 2>&1; then
    bats --jobs 4 "${bats_files[@]}" || status=1
  else
    nix develop .#run --command bats --jobs 4 "${bats_files[@]}" || status=1
  fi
fi

if [[ $warn -eq 1 && ${#slow[@]} -gt 0 ]]; then
  printf '\nPERFORMANCE WARNING: tests over %sms (refactor, or move to integration/e2e):\n' "$warn_ms"
  printf '  %s\n' "${slow[@]}"
fi

exit "$status"
