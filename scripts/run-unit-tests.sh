#!/usr/bin/env bash
# run-unit-tests.sh -- the commit-gate test runner (unit camp only).
#
# Design (operator, 2026-07-11):
# - Unit tests live in test/unit/ and are admitted by DESIGN (single component,
#   stub/fixture driven, no flows, no sleeps); FAST is the admission rule.
# - Seeded shuffle: order is randomized each run to flush hidden ordering
#   dependence; the seed is printed so any failure is replayable with
#   TEST_SEED=<seed>. (Bats 1.11 has no built-in shuffle; this wrapper is the
#   standard community workaround and covers plain .sh tests too.)
# - Per-test timing with a WARN-ONLY performance summary: tests slower than
#   UNIT_WARN_MS (default 200ms) are listed as refactor-or-move-camp
#   candidates. Warnings never fail the run; if measurement ever meaningfully
#   slows the run, the operator's fallback is a pre-push hard ceiling.
#
# Dependency-free by design: timing uses bash's built-in EPOCHREALTIME (bash
# 5+, both the host and the Nix devshell), NOT python3/date; the shuffle uses
# gshuf/shuf when present and degrades to sorted order otherwise. Nothing here
# assumes a tool that only exists via a leaked host PATH (the portability class
# that has bitten this repo before).
set -euo pipefail
export LC_ALL=C # force EPOCHREALTIME to use a '.' decimal separator

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT" || exit 1

UNIT_WARN_MS="${UNIT_WARN_MS:-200}"
seed="${TEST_SEED:-${RANDOM}${RANDOM}}"
printf 'unit tests: seed=%s (replay with TEST_SEED=%s)\n' "$seed" "$seed"

# Milliseconds since epoch from EPOCHREALTIME ("seconds.microseconds"): drop
# the dot to get integer microseconds, divide by 1000. No external process.
now_ms() {
  local r="${EPOCHREALTIME/./}"
  printf '%s' "$((r / 1000))"
}

tests=()
while IFS= read -r t; do
  tests+=("$t")
done < <(find test/unit -maxdepth 1 -type f -name '*.sh' -perm -u+x | sort)
[[ ${#tests[@]} -gt 0 ]] || {
  printf 'no unit tests found\n'
  exit 0
}

# Seeded shuffle when a shuf is available (gshuf or shuf); otherwise keep the
# sorted order (reproducible, just not randomized). Shuffle is a hidden-order
# probe, not a correctness requirement, so degrading is safe.
shuf_bin=""
for cand in gshuf shuf; do
  command -v "$cand" >/dev/null 2>&1 && {
    shuf_bin="$cand"
    break
  }
done
if [[ -n $shuf_bin ]]; then
  shuffled=()
  while IFS= read -r t; do
    shuffled+=("$t")
  done < <(printf '%s\n' "${tests[@]}" | "$shuf_bin" --random-source=<(yes "$seed") 2>/dev/null || printf '%s\n' "${tests[@]}")
  [[ ${#shuffled[@]} -eq ${#tests[@]} ]] && tests=("${shuffled[@]}")
fi

status=0
slow=()
for t in "${tests[@]}"; do
  start="$(now_ms)"
  if ! "$t"; then
    printf '== FAIL: %s ==\n' "$t"
    status=1
  fi
  ms=$(($(now_ms) - start))
  if ((ms > UNIT_WARN_MS)); then
    slow+=("$(printf '%6dms  %s' "$ms" "$t")")
  fi
done

if [[ ${#slow[@]} -gt 0 ]]; then
  printf '\nPERFORMANCE WARNING: unit tests over %sms (refactor, or move to integration/e2e):\n' "$UNIT_WARN_MS"
  printf '  %s\n' "${slow[@]}"
fi

exit "$status"
