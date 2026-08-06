#!/usr/bin/env bash
#
# Retention: each log keeps at most ROTATE_LOGS_ARCHIVES_KEPT compressed
# archives, numbered .1 (newest) through .N (oldest). A rotation shifts every
# archive one slot older and discards whatever falls off the end.
#
# The pruning rule is "keep exactly the retention window", not "delete the one
# that falls off the end". The window is bounded on both sides: a LOWERED keep
# count orphans strays above the new bound, and index 0 sits below every slot
# the shift ever touches, so a one-sided rule leaves either kind on disk
# forever. That is the unbounded growth this whole mechanism exists to prevent,
# reappearing one level up inside the fix.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROTATE_LOGS="$REPO_ROOT/dot_local/libexec/executable_compress-and-truncate-local-logs.sh"

failures=0
fail() {
  printf 'rotate-logs-retention: FAIL -- %s\n' "$*" >&2
  failures=$((failures + 1))
}

[[ -f $ROTATE_LOGS ]] || {
  printf 'rotate-logs-retention: FAIL -- missing script: %s\n' "$ROTATE_LOGS" >&2
  exit 1
}

# --- the retention window, as a pure predicate -----------------------------
# shellcheck source=/dev/null
source "$ROTATE_LOGS"

assert_index() {
  local description="$1" index="$2" keep="$3"
  is_retainable_archive_index "$index" "$keep" ||
    fail "$description: is_retainable_archive_index $index $keep should hold"
}
refute_index() {
  local description="$1" index="$2" keep="$3"
  if is_retainable_archive_index "$index" "$keep"; then
    fail "$description: is_retainable_archive_index $index $keep should not hold"
  fi
}

# With keep=3 the shift repopulates slots 2 and 3 from slots 1 and 2, so exactly
# 1 and 2 are worth carrying forward.
assert_index "newest generation is retained" 1 3
assert_index "last generation that still has room is retained" 2 3
refute_index "the generation that would fall off the end is pruned" 3 3
refute_index "anything past the end is pruned" 4 3
# The lower bound is the one that is easy to forget: nothing ever shifts index
# 0, so without this it would survive every pass forever.
refute_index "index 0 is outside our numbering and is pruned" 0 3
refute_index "keep=1 retains no shifted generation at all" 1 1
refute_index "a malformed index is pruned rather than kept" "x" 3

sandbox="$(mktemp -d)"
trap 'rm -rf "$sandbox"' EXIT
log_root="$sandbox/log"
mkdir -p "$log_root"
log="$log_root/busy.log"

# Seed a distinguishable payload per archive slot so a mis-ordered shift is
# visible, not merely a count that happens to match.
seed_archive() { # <index> <marker>
  printf 'marker=%s\n' "$2" | /usr/bin/gzip -c >"$log.$1.gz"
}

marker_in() { # <index> -- echo the marker recorded in that archive slot
  /usr/bin/gzip -dc "$log.$1.gz" 2>/dev/null | /usr/bin/sed -n 's/^marker=//p'
}

printf 'CURRENT-CONTENT\n' >"$log"
# Pad past the threshold so this log actually rotates.
/usr/bin/head -c 4096 /dev/zero | /usr/bin/tr '\0' 'y' >>"$log"

seed_archive 1 oldgen1
seed_archive 2 oldgen2
seed_archive 3 oldgen3
seed_archive 4 oldgen4 # already beyond a keep count of 3: a stray to clean up
# newsyslog(8) numbers its archives from .0. A log this machine rotated by any
# other means arrives carrying one, and nothing in the shift ever moves it, so
# it has to be pruned explicitly or it outlives every generation forever.
seed_archive 0 strayzero

ROTATE_LOGS_ROOT="$log_root" \
  ROTATE_LOGS_AT_BYTES=1024 \
  ROTATE_LOGS_ARCHIVES_KEPT=3 \
  bash "$ROTATE_LOGS" >"$sandbox/report.txt" 2>&1 ||
  fail "rotation pass exited non-zero: $(cat "$sandbox/report.txt")"

# --- the newest slot holds what was just rotated out -----------------------
if [[ -f $log.1.gz ]]; then
  /usr/bin/gzip -dc "$log.1.gz" | /usr/bin/grep -q 'CURRENT-CONTENT' ||
    fail "archive .1 does not hold the content that was just rotated out"
else
  fail "no archive .1 after rotation"
fi

# --- every surviving generation shifted exactly one slot older -------------
[[ "$(marker_in 2)" == "oldgen1" ]] ||
  fail "archive .2 holds '$(marker_in 2)', want the previous .1 (oldgen1)"
[[ "$(marker_in 3)" == "oldgen2" ]] ||
  fail "archive .3 holds '$(marker_in 3)', want the previous .2 (oldgen2)"

# --- nothing survives past the keep count ----------------------------------
if [[ -e $log.4.gz ]]; then
  fail "archive .4 still exists; the keep count of 3 must bound the chain"
fi
if [[ -e $log.5.gz ]]; then
  fail "archive .5 exists; rotation must never grow the chain past the keep count"
fi
if [[ -e $log.0.gz ]]; then
  fail "archive .0 survived; an index outside our numbering is never shifted, so it must be pruned"
fi

# --- completeness, not a count: assert the whole archive set --------------
mapfile -t archives < <(/usr/bin/find "$log_root" -name 'busy.log.*.gz' -type f | /usr/bin/sort)
expected=("$log.1.gz" "$log.2.gz" "$log.3.gz")
if [[ ${archives[*]} != "${expected[*]}" ]]; then
  fail "archive set is [${archives[*]}], want exactly [${expected[*]}]"
fi

# --- an archive is never itself treated as a log to rotate -----------------
if [[ -e $log.1.gz.1.gz ]]; then
  fail "an archive was rotated as if it were a log (recursive archiving)"
fi

if [[ $failures -gt 0 ]]; then
  printf 'rotate-logs-retention: %d assertion(s) failed\n' "$failures" >&2
  exit 1
fi

printf 'rotate-logs-retention: OK (chain shifts by one, bounded at the keep count)\n'
