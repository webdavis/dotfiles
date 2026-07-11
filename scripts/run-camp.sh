#!/usr/bin/env bash
# run-camp.sh <camp-dir> -- run one test camp: its executable *.sh tests, then
# its *.bats suites. Shared by the test-integration and test-e2e recipes so the
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
#      the camp. Closing fd 3 for the child severs that path.
set -euo pipefail

if [[ $# -ne 1 ]]; then
  printf 'usage: run-camp.sh <camp-dir>\n' >&2
  exit 2
fi
camp="$1"
if [[ ! -d $camp ]]; then
  printf 'FAIL: camp dir %s does not exist\n' "$camp" >&2
  exit 1
fi

sh_list="$(mktemp)"
bats_list="$(mktemp)"
trap 'rm -f "$sh_list" "$bats_list"' EXIT

if ! find "$camp" -maxdepth 1 -type f -name '*.sh' -perm -u+x -print0 | sort -z >"$sh_list"; then
  printf 'FAIL: %s .sh discovery failed; refusing to run a partial list\n' "$camp" >&2
  exit 1
fi

status=0
while IFS= read -r -u3 -d '' t; do
  printf '== %s ==\n' "$t"
  "$t" 3<&- || status=1
done 3<"$sh_list"

if ! find "$camp" -maxdepth 1 -type f -name '*.bats' -print0 | sort -z >"$bats_list"; then
  printf 'FAIL: %s .bats discovery failed; refusing to skip a partial list\n' "$camp" >&2
  exit 1
fi
bats_files=()
while IFS= read -r -d '' b; do
  bats_files+=("$b")
done <"$bats_list"
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

exit "$status"
