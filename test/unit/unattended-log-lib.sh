#!/usr/bin/env bash
#
# unattended-log-lib.sh: the shared entry shape the weekly unattended jobs post
# to the #unattended-upgrades channel.
#
# Three behaviours, and each exists because of a specific way this record could
# lie about the health of the job it reports on:
#
#   1. EVERY ENTRY STATES ITS OWN GAP. `man launchd.plist` on StartCalendarInterval:
#      "If multiple intervals transpire before the computer is woken, those events
#      will be coalesced into one event upon wake from sleep." So a healthy job can
#      legitimately produce ONE entry covering three weeks, and a MISSING entry
#      cannot distinguish a dead LaunchAgent from a closed laptop. Counting
#      messages therefore cannot measure health. Instead the newest entry carries
#      its own gap figure, which survives coalescing, sleep, shutdown and a wedged
#      deferral loop identically.
#   2. THE WEEK GUARD. Entries are emitted on the deferral and refusal exits too,
#      and a Monday fires 24 hourly slots, so without a guard a normal week would
#      post up to 24 entries. One per week per job.
#   3. NOTHING IS EVER SILENT. A missing timestamp, an unwritable guard, an absent
#      relay: each produces a stated line, never a quiet no-op that reads as a
#      delivered entry.
#
# Unit test: pure function tests against a PATH-stubbed `date` (so the ISO week
# and the wall clock are pinned without adding a test-only seam to the library)
# and a stub relay. No sleeps.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="$REPO_ROOT/dot_local/bin/unattended-log-lib.sh"

fail() {
  printf 'unattended-log-lib: FAIL -- %s\n' "$*" >&2
  exit 1
}

# An explicit refutation. `! cmd` under set -e never fails a test, and this repo
# has shipped that bug; every negative check here goes through this.
refute() {
  local pattern="$1" haystack="$2" message="$3"
  if grep -qE "$pattern" <<<"$haystack"; then
    printf '=== haystack ===\n%s\n' "$haystack" >&2
    fail "$message"
  fi
}

[[ -r $LIB ]] || fail "library not found: $LIB"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# date stub: FAKE_WEEK pins +%G-%V and FAKE_NOW pins +%s; everything else falls
# through to the real date so the ISO formatter is exercised for real.
mkdir -p "$tmp/stubs"
cat >"$tmp/stubs/date" <<'STUB'
#!/usr/bin/env bash
for arg in "$@"; do
  case "$arg" in
    +%G-%V) printf '%s\n' "${FAKE_WEEK:-2026-31}"; exit 0 ;;
    +%s) printf '%s\n' "${FAKE_NOW:-1785000000}"; exit 0 ;;
  esac
done
exec /bin/date "$@"
STUB
# relay stub: record the full invocation, the environment bits that matter, and
# whether it inherited fd 9 (the caller's serialize lock).
cat >"$tmp/stubs/relay-stub.sh" <<'STUB'
#!/usr/bin/env bash
printf 'ARGV: %s\n' "$*" >>"$RELAY_CALL_LOG"
printf 'URL: %s\n' "${RELAY_HERMES_URL:-<unset>}" >>"$RELAY_CALL_LOG"
if { : >&9; } 2>/dev/null; then
  printf 'FD9: inherited\n' >>"$RELAY_CALL_LOG"
else
  printf 'FD9: closed\n' >>"$RELAY_CALL_LOG"
fi
printf 'relay: posted HTTP 200\n'
STUB
chmod +x "$tmp/stubs/date" "$tmp/stubs/relay-stub.sh"
export PATH="$tmp/stubs:$PATH"

# shellcheck source=dot_local/bin/unattended-log-lib.sh
source "$LIB"

# ── 1. Elapsed formatting. The reader must see the gap at a glance, so the
#      units shift with the magnitude. Boundaries are pinned in both directions
#      because an off-by-one at 86400 turns "1d 0h" into "24h 0m". ────────────
check_elapsed() {
  local seconds="$1" want="$2" got
  got="$(unattended_log_elapsed "$seconds")"
  [[ $got == "$want" ]] || fail "elapsed $seconds: want '$want', got '$got'"
}
check_elapsed 0 "0s"
check_elapsed 59 "59s"
check_elapsed 60 "1m"
check_elapsed 3599 "59m"
check_elapsed 3600 "1h 0m"
check_elapsed 86399 "23h 59m"
check_elapsed 86400 "1d 0h"
check_elapsed 1987200 "23d 0h" # the dispatch's "last successful run: 23 days ago"
check_elapsed 604800 "7d 0h"

# A negative elapsed means the clock moved backwards. It must be reported as
# that, not silently rendered as a plausible small gap.
got="$(unattended_log_elapsed -5)"
grep -qiE 'clock|future|unknown' <<<"$got" ||
  fail "a negative elapsed rendered as a plausible gap instead of naming the clock: '$got'"

# ── 2. The gap line. Three states, three distinct sentences. ─────────────────
marker="$tmp/last-success-at"

# 2a. NEVER RECORDED. On this machine that is the true state today, and it must
#     read as an alarming fact rather than as a missing field.
rm -f "$marker"
line="$(unattended_log_gap_line "$marker")"
grep -qiE 'never' <<<"$line" ||
  fail "an absent marker did not say the previous run was never recorded: '$line'"
refute '\(0s ago\)|\(0m ago\)' "$line" "an absent marker rendered as a zero-length gap (reads as 'just ran')"

# 2b. RECORDED. FAKE_NOW is 1785000000; a marker 23 days earlier must read 23d.
printf '%s %s\n' "$((1785000000 - 1987200))" "2026-07-10T12:00:00Z" >"$marker"
line="$(unattended_log_gap_line "$marker")"
grep -qF '2026-07-10T12:00:00Z' <<<"$line" ||
  fail "the gap line dropped the recorded timestamp: '$line'"
grep -qF '23d 0h' <<<"$line" ||
  fail "the gap line did not carry the elapsed figure: '$line'"

# 2c. GARBAGE. An unparseable marker must say UNKNOWN, never arithmetic on junk
#     that yields a confident wrong number.
printf 'not-an-epoch whatever\n' >"$marker"
line="$(unattended_log_gap_line "$marker")"
grep -qiE 'unknown|unreadable' <<<"$line" ||
  fail "an unparseable marker produced a confident figure instead of UNKNOWN: '$line'"

# 2c-2. A digits-only epoch that begins with 0 is DECIMAL. Bash arithmetic reads
#     a leading zero as OCTAL, so `08...` and `09...` abort with "value too great
#     for base" -- and this line runs at START-UP in both weekly jobs, one of
#     which runs under set -e, so the whole run would die before its lock, its
#     alert or its record. Two of the ten digits produce it, and a truncated or
#     half-written marker is exactly how one appears.
printf '%s %s\n' "0837000000" "2026-07-10T12:00:00Z" >"$marker"
line="$(unattended_log_gap_line "$marker" 2>&1)" ||
  fail "an epoch with a leading zero aborted the gap line: '$line'"
grep -qF '10972d 5h' <<<"$line" ||
  fail "an epoch with a leading zero was not read in base 10 (want the 10972d 5h decimal gap): '$line'"
refute 'base|arithmetic|too great' "$line" "the gap line leaked a bash arithmetic error"

# 2d. A marker written by mark_success round-trips through the gap line.
rm -f "$marker"
unattended_log_mark_success "$marker"
[[ -s $marker ]] || fail "mark_success wrote nothing"
line="$(unattended_log_gap_line "$marker")"
grep -qiE 'unknown|never' <<<"$line" &&
  fail "a marker this library just wrote is not readable by its own gap line: '$line'"
grep -qE '[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z' <<<"$line" ||
  fail "mark_success did not record an ISO 8601 UTC timestamp: '$line'"

# ── 3. The weekly guard. A Monday fires 24 hourly slots; without this the
#      channel would take up to 24 entries a week. ────────────────────────────
guard="$tmp/last-log-week"

# 3a. 24 consecutive DEFERRED slots in one week yield exactly ONE entry.
rm -f "$guard"
emitted=0
for _ in $(seq 1 24); do
  if FAKE_WEEK=2026-31 unattended_log_claim_week "$guard" deferred; then
    emitted=$((emitted + 1))
  fi
done
[[ $emitted -eq 1 ]] ||
  fail "24 deferred slots in one week emitted $emitted entries, want exactly 1"

# 3b. A COMPLETED run later the same week still gets its entry. Suppressing it
#     would leave "deferred, nothing attempted" as the newest message of a week
#     the job actually finished -- a health signal inverted, which is the exact
#     class of bug this feature exists to end.
FAKE_WEEK=2026-31 unattended_log_claim_week "$guard" completed ||
  fail "a completed run was suppressed by an earlier deferred entry in the same week"

# 3c. ...but only once, and a later deferral must not overwrite it.
FAKE_WEEK=2026-31 unattended_log_claim_week "$guard" completed &&
  fail "a second completed entry was emitted in the same week"
FAKE_WEEK=2026-31 unattended_log_claim_week "$guard" deferred &&
  fail "a deferral after a completed entry was emitted; it would bury the truer message"

# 3d. A NEW week starts over.
FAKE_WEEK=2026-32 unattended_log_claim_week "$guard" deferred ||
  fail "a new ISO week did not emit"
FAKE_WEEK=2026-32 unattended_log_claim_week "$guard" deferred &&
  fail "a new week's second slot emitted"

# 3e. An unwritable guard fails OPEN (emit) and SAYS so. Silence would be the
#     worse failure: an entry nobody sees at all.
rm -rf "$tmp/nowrite"
mkdir -p "$tmp/nowrite"
chmod 500 "$tmp/nowrite"
warn="$(FAKE_WEEK=2026-33 unattended_log_claim_week "$tmp/nowrite/guard" deferred 2>&1)" || claim_rc=$?
claim_rc="${claim_rc:-0}"
chmod 700 "$tmp/nowrite"
[[ $claim_rc -eq 0 ]] || fail "an unwritable guard suppressed the entry (must fail open)"
grep -qiE 'guard|could not' <<<"$warn" ||
  fail "an unwritable guard was not reported: '$warn'"

# ── 4. Delivery. The entry must go out over --remote-only to the LOG route, and
#      a missing relay must be stated, never swallowed. ─────────────────────────
RELAY_CALL_LOG="$tmp/relay-calls.log"
export RELAY_CALL_LOG
: >"$RELAY_CALL_LOG"
out="$(UNATTENDED_LOG_RELAY="$tmp/stubs/relay-stub.sh" \
  UNATTENDED_LOG_HERMES_URL="http://hermes.test/webhooks/unattended-upgrades" \
  unattended_log_post update-skills completed dresden $'run at X\nlast successful run: never' 2>&1)"
post_rc=$?
[[ $post_rc -eq 0 ]] || fail "unattended_log_post exited $post_rc; it must never fail its caller"
calls="$(cat "$RELAY_CALL_LOG")"
grep -qF -- '--remote-only' <<<"$calls" ||
  fail "the entry was not posted with --remote-only (it would pop a banner and buzz the phone every week)"
grep -qF 'http://hermes.test/webhooks/unattended-upgrades' <<<"$calls" ||
  fail "the entry was not routed to the log route: $calls"
for field in '--agent update-skills' '--state completed' '--project dresden'; do
  grep -qF -- "$field" <<<"$calls" || fail "the entry lost $field: $calls"
done
grep -qF 'posted HTTP 200' <<<"$out" ||
  fail "relay's delivery outcome did not reach the caller's run log: '$out'"

# The entry NAMES ITS HOST. The channel aggregates both weekly jobs and the
# daemon-host role is expected to move to a second Mac, so an entry that does not
# say which machine it is about cannot be investigated. Checked at the function,
# because both callers pass its result straight through and neither one's own
# tests would notice it going empty.
host="$(unattended_log_host)"
[[ -n $host ]] || fail "unattended_log_host returned nothing; every entry would be posted with an empty --project"
refute '^[[:space:]]+$' "$host" "unattended_log_host returned only whitespace"
if real_host="$(hostname -s 2>/dev/null)" && [[ -n $real_host ]]; then
  [[ $host == "$real_host" ]] ||
    fail "unattended_log_host says '$host' but this machine is '$real_host'"
fi

# 4a-2. fd 9 is CLOSED for relay and everything it spawns. The caller holds its
# serialize lock as a kernel flock on fd 9, relay detaches channels that outlive
# the whole run, and a flock is released only when the LAST copy of the fd
# closes. An inherited copy in a detached curl therefore keeps the lock held
# after the job exited, and the next scheduled slot defers over a competing run
# that does not exist. This repo has shipped that bug twice (fix-A F8, and again
# on the fork advisory push), which is why it is asserted here rather than
# trusted to the `9>&-` staying put.
: >"$RELAY_CALL_LOG"
(
  exec 9>>"$tmp/fd9-lock"
  UNATTENDED_LOG_RELAY="$tmp/stubs/relay-stub.sh" \
    unattended_log_post update-skills completed dresden "body" >/dev/null 2>&1
)
grep -qF 'FD9: closed' "$RELAY_CALL_LOG" ||
  fail "relay inherited the caller's serialize-lock fd; a detached child would hold the lock after the run exited: $(cat "$RELAY_CALL_LOG")"

# 4b. relay.sh absent: state it, exit 0, deliver nothing.
: >"$RELAY_CALL_LOG"
out="$(UNATTENDED_LOG_RELAY="$tmp/does-not-exist.sh" \
  unattended_log_post update-skills completed dresden "body" 2>&1)"
post_rc=$?
[[ $post_rc -eq 0 ]] || fail "a missing relay.sh failed the caller (rc=$post_rc)"
grep -qiE 'relay|not delivered|not executable' <<<"$out" ||
  fail "a missing relay.sh produced NO line; silence reads as a delivered entry: '$out'"
[[ ! -s $RELAY_CALL_LOG ]] || fail "a missing relay.sh somehow logged a call"

# ── 4c. The CHANGE SUMMARY. Both weekly jobs render through this one function,
#       so the channel reads as one log rather than two. Fixture snapshots are
#       built through printf '\t' rather than embedded literal tabs, so a
#       whitespace-mangling edit cannot silently turn these into rows nothing
#       matches. ─────────────────────────────────────────────────────────────
row() { printf '%s\t%s' "$1" "$2"; }
write_snapshot() { # <file> <name:fingerprint>...
  local file="$1"
  shift
  : >"$file"
  local spec name fingerprint
  for spec in "$@"; do
    name="${spec%%:*}"
    fingerprint="${spec#*:}"
    printf '%s\n' "$(row "$name" "$fingerprint")" >>"$file"
  done
}
before="$tmp/change-before"
after="$tmp/change-after"
CAVEAT='no version number is knowable'

# NOTHING CHANGED. The count AND the total must both be right: "0 of 0" would
# read as a clean week on an empty subject, which is the "looks like success when
# nothing happened" shape this whole record exists to end.
write_snapshot "$before" "alpha:aaa1" "beta:bbb1"
write_snapshot "$after" "alpha:aaa1" "beta:bbb1"
line="$(unattended_log_change_line "$before" "$after" "npx-tracked skills" "$CAVEAT" opaque)"
[[ $line == "npx-tracked skills: 0 of 2 tracked entries changed. $CAVEAT" ]] ||
  fail "an unchanged subject rendered as: '$line'"

# OPAQUE style names the subject and prints no fingerprint: a 64-character
# content hash tells a reader nothing.
write_snapshot "$after" "alpha:aaa2" "beta:bbb1"
line="$(unattended_log_change_line "$before" "$after" "npx-tracked skills" "$CAVEAT" opaque)"
grep -qF 'npx-tracked skills: 1 of 2 tracked entries changed (alpha).' <<<"$line" ||
  fail "an opaque change did not name the subject: '$line'"
refute 'aaa1|aaa2' "$line" "the opaque style printed a fingerprint, which tells the reader nothing"
grep -qF "$CAVEAT" <<<"$line" || fail "the change line dropped its caveat: '$line'"

# VERSIONS style prints the transition, which is the whole value on a subject
# that actually has version numbers.
write_snapshot "$before" "jq:1.7.1" "yq:4.53.3"
write_snapshot "$after" "jq:1.8.0" "yq:4.53.3"
line="$(unattended_log_change_line "$before" "$after" "formulae" "brew reports these" versions)"
grep -qF 'formulae: 1 of 2 tracked entries changed (jq 1.7.1 -> 1.8.0).' <<<"$line" ||
  fail "the versions style did not render the transition: '$line'"

# ADDED and REMOVED are changes too. The removal is the single most worth-seeing
# line: something left without being asked to.
write_snapshot "$before" "alpha:aaa1"
write_snapshot "$after" "alpha:aaa1" "delta:ddd1"
grep -qF '1 of 2 tracked entries changed (delta (added)).' \
  <<<"$(unattended_log_change_line "$before" "$after" subject "$CAVEAT" opaque)" ||
  fail "an added entry was not reported"
write_snapshot "$before" "alpha:aaa1" "beta:bbb1"
write_snapshot "$after" "alpha:aaa1"
grep -qF 'beta (removed)' \
  <<<"$(unattended_log_change_line "$before" "$after" subject "$CAVEAT" opaque)" ||
  fail "a removed entry was not reported"

# A whole-subject move must not blow past Discord's 2000-character message cap
# and take the gap figure with it. Names are capped, the remainder is counted,
# and the true totals survive.
: >"$before"
: >"$after"
for i in $(seq 1 40); do
  printf '%s\n' "$(row "$(printf 'item%02d' "$i")" old)" >>"$before"
  printf '%s\n' "$(row "$(printf 'item%02d' "$i")" new)" >>"$after"
done
line="$(unattended_log_change_line "$before" "$after" subject "$CAVEAT" opaque)"
grep -qF 'subject: 40 of 40 tracked entries changed' <<<"$line" ||
  fail "the capped line lost the true totals: '$line'"
grep -qE 'and 28 more' <<<"$line" ||
  fail "the capped line did not count the names it withheld: '$line'"
[[ ${#line} -lt 800 ]] ||
  fail "a whole-subject move rendered ${#line} characters; Discord caps a message at 2000"

# ── 5. The route name the library posts to is the one the config declares and
#      the apply-time status check probes. A rename in one place and not the
#      others is a 404 on every entry, which relay reports but nobody reads. ──
[[ ${UNATTENDED_LOG_ROUTE:-} == "unattended-upgrades" ]] ||
  fail "UNATTENDED_LOG_ROUTE is '${UNATTENDED_LOG_ROUTE:-}', want 'unattended-upgrades'"
default_url="$(UNATTENDED_LOG_RELAY="$tmp/stubs/relay-stub.sh" unattended_log_url)"
[[ $default_url == *"/webhooks/$UNATTENDED_LOG_ROUTE" ]] ||
  fail "the default URL does not end in /webhooks/$UNATTENDED_LOG_ROUTE: $default_url"
grep -qE '^http://127\.0\.0\.1:8644/' <<<"$default_url" ||
  fail "the default URL does not point at the loopback hermes gateway: $default_url"

printf 'unattended-log-lib: OK\n'
