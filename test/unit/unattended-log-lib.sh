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
#      post up to 24 entries. One per class per week per job, so one in an
#      ordinary week and two in a week that defers before it completes.
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
printf 'date %s\n' "$*" >>"${DATE_CALL_LOG:-/dev/null}"
# FAKE_DATE_BROKEN simulates a clock that cannot be read at all (every
# invocation fails), which is the state the entry header's fallback names.
[[ -n ${FAKE_DATE_BROKEN:-} ]] && exit 1
for arg in "$@"; do
  case "$arg" in
    +%G-%V) printf '%s\n' "${FAKE_WEEK:-2026-31}"; exit 0 ;;
    "+%s %Y-%m-%dT%H:%M:%SZ")
      printf '%s %s\n' "${FAKE_NOW:-1785000000}" "${FAKE_NOW_ISO:-2026-07-25T12:00:00Z}"
      exit 0
      ;;
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
# The real relay prints its delivery outcome on stdout and exits 0 whatever
# happened, so RELAY_STUB_OUTCOME is the only way a caller can tell the two
# apart -- which is the point of the assertions below.
printf '%s\n' "${RELAY_STUB_OUTCOME:-relay: posted HTTP 200}"
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

# ── 2e. The entry HEADER: the run timestamp and the gap, from ONE clock
#       reading. Two readings is how a two-hour run ends up printing timestamps
#       seven days and two hours apart above a gap that reads seven days, and
#       nothing in the entry tells the reader which figure to trust. The call
#       COUNT is the assertion, because one call is the only thing that makes
#       the two figures the same instant by construction. ────────────────────
printf '%s %s\n' "$((1785000000 - 259200))" "2026-07-22T12:00:00Z" >"$marker"
DATE_CALL_LOG="$tmp/date-calls.log"
export DATE_CALL_LOG
: >"$DATE_CALL_LOG"
header="$(FAKE_NOW=1785000000 FAKE_NOW_ISO=2026-07-25T12:00:00Z \
  unattended_log_entry_header "$marker")"
grep -qF 'run at 2026-07-25T12:00:00Z' <<<"$header" ||
  fail "the entry header does not carry this run's timestamp: '$header'"
grep -qF '(3d 0h ago)' <<<"$header" ||
  fail "the entry header does not carry the gap to the previous success: '$header'"
date_calls="$(grep -c . "$DATE_CALL_LOG" || true)"
[[ $date_calls -eq 1 ]] ||
  fail "the entry header read the clock $date_calls times, want exactly 1: $(cat "$DATE_CALL_LOG")"
: >"$DATE_CALL_LOG"

# ── 2f. The header when the clock CANNOT BE READ still carries both lines. The
#       fallback names the missing timestamp instead of inventing one, and the
#       gap line must survive into it: dropping it there passed every test
#       before this (mutation-verified), and a broken clock is exactly when the
#       recorded previous-success ISO is the only time figure the entry has. ──
header="$(FAKE_DATE_BROKEN=1 unattended_log_entry_header "$marker")"
grep -qiF 'run at UNKNOWN' <<<"$header" ||
  fail "an unreadable clock did not name the missing run timestamp: '$header'"
grep -qF 'last successful run:' <<<"$header" ||
  fail "the unreadable-clock fallback dropped the gap line, leaving a one-line entry with no previous-success record: '$header'"
grep -qF '2026-07-22T12:00:00Z' <<<"$header" ||
  fail "the fallback gap line lost the recorded previous-success timestamp: '$header'"
: >"$DATE_CALL_LOG"

# ── 3. The weekly guard. A Monday fires 24 hourly slots; without this the
#      channel would take up to 24 entries a week. ────────────────────────────
guard="$tmp/log-week-claims"

# 3a. 24 consecutive DEFERRED slots in one week yield exactly ONE entry.
rm -rf "$guard"
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

# 3c-2. A week whose FIRST outcome is completed refuses a later deferral too.
#     3c above stages deferred-then-completed, where the deferral's own token
#     already refuses the repeat, so 3c passes even with the bury-guard deleted
#     (mutation-verified). This is the sequence only the guard can refuse: the
#     week completes on an early slot with no prior deferral, then a later slot
#     hits lock contention and defers. Posting that deferral would leave
#     "deferred, nothing attempted" as the newest message of a finished week.
FAKE_WEEK=2026-45 unattended_log_claim_week "$guard" completed ||
  fail "3c-2 setup: the completed-first claim was refused"
FAKE_WEEK=2026-45 unattended_log_claim_week "$guard" deferred &&
  fail "a deferral in a week that completed FIRST was emitted; it would bury the completed entry without ever owning a deferred token"

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

# 3f. A CORRUPT guard must not WEDGE the week. The previous shape read one file
#     and treated any class that was not literally `deferred` as completed, so a
#     single malformed line refused BOTH claim types for the rest of the week,
#     with no warning and no rewrite: a week that posted nothing while its guard
#     asserted it had. Staged here as the exact reproduction -- a guard holding
#     "<this week> garbage".
rm -rf "$guard"
printf '2026-36 garbage\n' >"$guard"
claim_rc=0
warn="$(FAKE_WEEK=2026-36 unattended_log_claim_week "$guard" deferred 2>&1)" || claim_rc=$?
[[ $claim_rc -eq 0 ]] ||
  fail "a malformed guard suppressed the week's record instead of failing open"
grep -qiE 'guard|could not' <<<"$warn" ||
  fail "a malformed guard was not reported: '$warn'"
rm -f "$guard"

# 3g. Junk sitting INSIDE the guard is ignored rather than parsed. Only this
#     function's own token names mean anything to it.
mkdir -p "$guard"
: >"$guard/2026-35 garbage"
: >"$guard/junk"
FAKE_WEEK=2026-35 unattended_log_claim_week "$guard" deferred ||
  fail "a corrupt entry inside the guard suppressed the week's record"

# 3h. An unrecognised CLASS is stated and ungated, never silently trusted. A
#     class this function cannot describe cannot be gated by it either.
rm -rf "$guard"
claim_rc=0
warn="$(FAKE_WEEK=2026-37 unattended_log_claim_week "$guard" nonsense 2>&1)" || claim_rc=$?
[[ $claim_rc -eq 0 ]] ||
  fail "an unrecognised entry class was silently suppressed"
grep -qiE 'unrecognised|unrecognized' <<<"$warn" ||
  fail "an unrecognised entry class was not reported: '$warn'"

# 3h-2. An UNREADABLE ISO WEEK is stated and ungated too. The library's header
#     comment promises exactly this ("Empty when the clock cannot be read,
#     which every caller treats as 'do not gate', never as 'already claimed'"),
#     and inverting it to fail closed passed every test before this
#     (mutation-verified): a machine whose date broke would silently post
#     nothing, forever, which is the invisibility the record exists to end.
rm -rf "$guard"
claim_rc=0
warn="$(FAKE_DATE_BROKEN=1 unattended_log_claim_week "$guard" deferred 2>&1)" || claim_rc=$?
[[ $claim_rc -eq 0 ]] ||
  fail "an unreadable ISO week suppressed the entry (must fail open, rc=$claim_rc)"
grep -qiE 'week' <<<"$warn" ||
  fail "an unreadable ISO week was not reported: '$warn'"

# 3i. CONCURRENT slots claiming the same fresh week: exactly ONE wins. These runs
#     genuinely overlap -- a contending slot posts its "another run holds the
#     lock" entry while the holder is still working -- and a read-then-write
#     guard lets every one of them read an unclaimed week and post. Measured on
#     the read-then-write shape: 200 of 200 concurrent pairs both claimed.
#     The claimers are released together through a start GATE: each announces
#     itself and then spins (no sleeps) until the gate file appears, so they all
#     reach the claim at once instead of serializing behind their own fork cost,
#     which is what makes a read-then-write shape visibly lose.
rm -rf "$guard"
winners="$tmp/claim-winners"
: >"$winners"
claim_racers=40
ready="$tmp/claim-ready"
gate="$tmp/claim-gate"
rm -rf "$ready" "$gate"
mkdir -p "$ready"
for racer in $(seq 1 "$claim_racers"); do
  (
    : >"$ready/$racer"
    while [[ ! -e $gate ]]; do :; done
    FAKE_WEEK=2026-34 unattended_log_claim_week "$guard" deferred 2>/dev/null &&
      printf 'won\n' >>"$winners"
  ) &
done
while [[ "$(find "$ready" -type f | wc -l | tr -d ' ')" -lt $claim_racers ]]; do :; done
: >"$gate"
wait
concurrent_winners="$(grep -c . "$winners" || true)"
[[ $concurrent_winners -eq 1 ]] ||
  fail "$claim_racers concurrent slots claimed the same week $concurrent_winners times, want exactly 1; the claim is not atomic"

# 3i-2. The claim is an EXCLUSIVE create, pinned DETERMINISTICALLY. The race
#     above is real-world evidence but a probabilistic one: measured against a
#     check-then-create mutant ([[ -e ]] before a plain redirect) it failed only
#     one run in three, because the racers' arrival jitter dwarfs that mutant's
#     microsecond window. O_EXCL has a second observable that needs no timing at
#     all: it REFUSES to create through a dangling symlink (EEXIST on the link
#     itself), while any plain redirect follows the link and creates its target.
#     So a dangling symlink squatting on the token name must leave the claim
#     failing OPEN (stated, ungated) with the link's target still absent; a
#     non-exclusive create would silently "win" and write the target file.
rm -rf "$guard"
mkdir -p "$guard"
excl_target="$tmp/excl-probe-target"
rm -f "$excl_target"
ln -s "$excl_target" "$guard/2026-48.deferred"
excl_rc=0
excl_warn="$(FAKE_WEEK=2026-48 unattended_log_claim_week "$guard" deferred 2>&1)" || excl_rc=$?
[[ $excl_rc -eq 0 ]] ||
  fail "a squatted token name suppressed the entry (rc=$excl_rc); an unusable guard must fail open"
[[ ! -e $excl_target ]] ||
  fail "the claim wrote through a dangling symlink; it is not an exclusive create, so two slots can both claim a fresh week"
grep -qiE 'guard|could not' <<<"$excl_warn" ||
  fail "a claim that could not take its token said nothing: '$excl_warn'"

# 3j. A CLAIM CAN BE GIVEN BACK. The week is claimed before delivery is
#     attempted (so concurrent slots cannot both post) and released when that
#     delivery failed, which is what lets a later slot retry a week that has no
#     entry yet.
rm -rf "$guard"
FAKE_WEEK=2026-38 unattended_log_claim_week "$guard" deferred ||
  fail "the first claim of a fresh week was refused"
FAKE_WEEK=2026-38 unattended_log_claim_week "$guard" deferred &&
  fail "the week was claimable twice before any release"
FAKE_WEEK=2026-38 unattended_log_release_week "$guard" deferred
FAKE_WEEK=2026-38 unattended_log_claim_week "$guard" deferred ||
  fail "a released week was not claimable again, so a failed delivery would silence the week"

# 3j-2. A release frees ONLY its own class. Both entry classes coexist in a
#     week that defers and then completes, and each release is taken for one
#     failed delivery -- a release that also frees the sibling would let a later
#     slot repeat an entry that WAS delivered (mutation-verified: a release
#     deleting both entry tokens passed every suite before this).
#     Releasing deferred must leave completed claimed:
rm -rf "$guard"
FAKE_WEEK=2026-46 unattended_log_claim_week "$guard" deferred ||
  fail "3j-2 setup: the deferred claim was refused"
FAKE_WEEK=2026-46 unattended_log_claim_week "$guard" completed ||
  fail "3j-2 setup: the completed claim was refused"
FAKE_WEEK=2026-46 unattended_log_release_week "$guard" deferred
FAKE_WEEK=2026-46 unattended_log_claim_week "$guard" completed &&
  fail "releasing the deferred claim also freed the completed one; a delivered completed entry would repeat"
#     ...and releasing completed must leave deferred claimed (the completed
#     token is gone, so only the deferred token itself can refuse this).
rm -rf "$guard"
FAKE_WEEK=2026-47 unattended_log_claim_week "$guard" deferred ||
  fail "3j-2 setup: the deferred claim was refused (second stage)"
FAKE_WEEK=2026-47 unattended_log_claim_week "$guard" completed ||
  fail "3j-2 setup: the completed claim was refused (second stage)"
FAKE_WEEK=2026-47 unattended_log_release_week "$guard" completed
FAKE_WEEK=2026-47 unattended_log_claim_week "$guard" deferred &&
  fail "releasing the completed claim also freed the deferred one; a delivered deferral would repeat"

# 3j-3. A release does NOT depend on the clock. A slot can CLAIM in one ISO week
#     and, after a long inventory read that spans midnight into the next ISO week,
#     RELEASE. The old shape re-read the week at release time and deleted the NEW
#     week's token (which never existed), leaving the real claim to silence every
#     later slot. Claim in one week, release while the clock reads the next, and
#     the claim must be GONE.
rm -rf "$guard"
FAKE_WEEK=2026-52 unattended_log_claim_week "$guard" completed ||
  fail "3j-3 setup: the completed claim was refused"
FAKE_WEEK=2027-01 unattended_log_release_week "$guard" completed
[[ -z "$(find "$guard" -name '*.completed' 2>/dev/null)" ]] ||
  fail "a release that read the clock instead of the guard missed the token across a week boundary: $(ls -A "$guard")"
#     ...and it stays class-scoped across the boundary: a deferred token in the
#     claimed week survives a completed release taken under a different week.
rm -rf "$guard"
FAKE_WEEK=2026-52 unattended_log_claim_week "$guard" deferred ||
  fail "3j-3 setup: the deferred claim was refused"
FAKE_WEEK=2026-52 unattended_log_claim_week "$guard" completed ||
  fail "3j-3 setup: the completed claim was refused"
FAKE_WEEK=2027-01 unattended_log_release_week "$guard" completed
FAKE_WEEK=2026-52 unattended_log_claim_week "$guard" deferred &&
  fail "a cross-boundary completed release also freed the deferred claim"

# 3k. The guard keeps only THIS week, so it stays readable as "what did this week
#     do" instead of growing a file per class per week forever.
rm -rf "$guard"
FAKE_WEEK=2026-39 unattended_log_claim_week "$guard" deferred || fail "3k setup: the first claim was refused"
FAKE_WEEK=2026-40 unattended_log_claim_week "$guard" deferred || fail "3k setup: the new week was refused"
[[ -z "$(find "$guard" -name '2026-39.*' 2>/dev/null)" ]] ||
  fail "the guard kept a previous week's claim: $(ls -A "$guard")"

# ── 4. Delivery. The entry must go out over --remote-only to the LOG route, and
#      a missing relay must be stated, never swallowed. ─────────────────────────
RELAY_CALL_LOG="$tmp/relay-calls.log"
export RELAY_CALL_LOG
: >"$RELAY_CALL_LOG"
out="$(UNATTENDED_LOG_RELAY="$tmp/stubs/relay-stub.sh" \
  UNATTENDED_LOG_HERMES_URL="http://hermes.test/webhooks/unattended-upgrades" \
  unattended_log_post update-skills completed dresden $'run at X\nlast successful run: never' 2>&1)"
post_rc=$?
[[ $post_rc -eq 0 ]] || fail "a DELIVERED entry reported failure (rc=$post_rc)"
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
# The broken-channel alert is a relay call too, from under the same lock.
: >"$RELAY_CALL_LOG"
rm -rf "$tmp/fd9-alert-claims"
(
  exec 9>>"$tmp/fd9-lock"
  FAKE_WEEK=2026-43 UNATTENDED_LOG_RELAY="$tmp/stubs/relay-stub.sh" \
    unattended_log_alert_delivery_failure "$tmp/fd9-alert-claims" update-skills >/dev/null 2>&1
)
grep -qF 'FD9: closed' "$RELAY_CALL_LOG" ||
  fail "the broken-channel alert inherited the caller's serialize-lock fd: $(cat "$RELAY_CALL_LOG")"

# 4b. relay.sh absent: state it, report the failure, deliver nothing.
: >"$RELAY_CALL_LOG"
post_rc=0
out="$(UNATTENDED_LOG_RELAY="$tmp/does-not-exist.sh" \
  unattended_log_post update-skills completed dresden "body" 2>&1)" || post_rc=$?
[[ $post_rc -ne 0 ]] ||
  fail "a missing relay.sh reported the entry as DELIVERED; the week would be marked done with nothing sent"
grep -qiE 'relay|not delivered|not executable' <<<"$out" ||
  fail "a missing relay.sh produced NO line; silence reads as a delivered entry: '$out'"
[[ ! -s $RELAY_CALL_LOG ]] || fail "a missing relay.sh somehow logged a call"

# 4c. A REFUSED delivery is reported as one. This is the seam the whole week
#     guard hangs on: relay exits 0 whatever the gateway answered (by design, so
#     a broken record never breaks the job), so an entry that 401s or 404s is
#     indistinguishable from a delivered one unless this function reads the
#     outcome. Marking the week done on a refused delivery silences the other 23
#     slots and leaves the week with no entry while the guard asserts one.
for outcome in 'relay: post FAILED HTTP 401' 'relay: post FAILED HTTP 000 (no response; is the hermes gateway up?)' 'relay: post SKIPPED -- no hermes signing key'; do
  : >"$RELAY_CALL_LOG"
  post_rc=0
  out="$(RELAY_STUB_OUTCOME="$outcome" UNATTENDED_LOG_RELAY="$tmp/stubs/relay-stub.sh" \
    unattended_log_post update-skills completed dresden "body" 2>&1)" || post_rc=$?
  [[ $post_rc -ne 0 ]] ||
    fail "'$outcome' was reported as a delivered entry"
  grep -qF "$outcome" <<<"$out" ||
    fail "relay's outcome line did not reach the caller's run log: '$out'"
done

# ── 4d. A BROKEN RECORD CHANNEL is an actionable failure, so it goes to the
#       ALERT route -- the one that reaches the priority channel -- and not only
#       to a run log nobody opens. That is the same reasoning that rejected
#       drift-watch: a passive signal goes unnoticed, and this one is passive by
#       construction, because the channel that would carry it is the broken one.
#       At most once a week, and only when it is actually broken. ─────────────
alert_guard="$tmp/alert-claims"
rm -rf "$alert_guard"
: >"$RELAY_CALL_LOG"
FAKE_WEEK=2026-41 UNATTENDED_LOG_RELAY="$tmp/stubs/relay-stub.sh" \
  UNATTENDED_LOG_HERMES_URL="http://hermes.test/webhooks/unattended-upgrades" \
  unattended_log_alert_delivery_failure "$alert_guard" update-skills >/dev/null 2>&1
calls="$(cat "$RELAY_CALL_LOG")"
[[ -n $calls ]] || fail "a broken record channel raised no alert at all"
grep -qF 'URL: <unset>' <<<"$calls" ||
  fail "the broken-channel alert went to the LOG route, which is the route that is broken: $calls"
refute '[-][-]remote-only' "$calls" \
  "the broken-channel alert used the log path's flag, so it would neither banner nor buzz"
grep -qF -- '--agent update-skills' <<<"$calls" ||
  fail "the broken-channel alert does not name the job it is about: $calls"
grep -qiE 'record|channel' <<<"$calls" ||
  fail "the broken-channel alert does not say what is broken: $calls"
# ...once. A weekly job that cannot deliver would otherwise alert on every slot.
: >"$RELAY_CALL_LOG"
FAKE_WEEK=2026-41 UNATTENDED_LOG_RELAY="$tmp/stubs/relay-stub.sh" \
  unattended_log_alert_delivery_failure "$alert_guard" update-skills >/dev/null 2>&1
[[ ! -s $RELAY_CALL_LOG ]] ||
  fail "the broken-channel alert fired twice in one week: $(cat "$RELAY_CALL_LOG")"
# ...and a new week may say it again, because it is still broken.
: >"$RELAY_CALL_LOG"
FAKE_WEEK=2026-42 UNATTENDED_LOG_RELAY="$tmp/stubs/relay-stub.sh" \
  unattended_log_alert_delivery_failure "$alert_guard" update-skills >/dev/null 2>&1
[[ -s $RELAY_CALL_LOG ]] ||
  fail "a new week did not re-report a record channel that is still broken"

# A run that CANNOT alert must not spend the week's alert token. The token is the
# only thing between a relay restored later that same week and the remaining 23
# slots: consumed by a call that sent nothing, every retry for the rest of the
# week is suppressed, and the operator hears that the record channel is broken
# next week at the earliest. Both halves of a delivery failure would then be
# silent at once, which is the exact shape this whole record exists to end.
: >"$RELAY_CALL_LOG"
out="$(FAKE_WEEK=2026-45 UNATTENDED_LOG_RELAY="$tmp/does-not-exist.sh" \
  unattended_log_alert_delivery_failure "$alert_guard" update-skills 2>&1)"
[[ ! -s $RELAY_CALL_LOG ]] || fail "an absent relay somehow logged a call: $(cat "$RELAY_CALL_LOG")"
grep -qiE 'not delivered|not executable' <<<"$out" ||
  fail "an absent relay produced NO line; the alert vanished silently: '$out'"
: >"$RELAY_CALL_LOG"
FAKE_WEEK=2026-45 UNATTENDED_LOG_RELAY="$tmp/stubs/relay-stub.sh" \
  unattended_log_alert_delivery_failure "$alert_guard" update-skills >/dev/null 2>&1
[[ -s $RELAY_CALL_LOG ]] ||
  fail "a relay restored later in the same week found the alert token already spent by a call that sent nothing"
# ...and that later call DID spend it, so the week is still capped at one alert.
: >"$RELAY_CALL_LOG"
FAKE_WEEK=2026-45 UNATTENDED_LOG_RELAY="$tmp/stubs/relay-stub.sh" \
  unattended_log_alert_delivery_failure "$alert_guard" update-skills >/dev/null 2>&1
[[ ! -s $RELAY_CALL_LOG ]] ||
  fail "the alert that WAS sent did not claim the week; every later slot would alert again: $(cat "$RELAY_CALL_LOG")"

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
# shellcheck disable=SC2016 # the backticks are Discord code-span syntax, not a substitution
grep -qF 'npx-tracked skills: 1 of 2 tracked entries changed (`alpha`).' <<<"$line" ||
  fail "an opaque change did not name the subject: '$line'"
refute 'aaa1|aaa2' "$line" "the opaque style printed a fingerprint, which tells the reader nothing"
grep -qF "$CAVEAT" <<<"$line" || fail "the change line dropped its caveat: '$line'"

# VERSIONS style prints the transition, which is the whole value on a subject
# that actually has version numbers.
write_snapshot "$before" "jq:1.7.1" "yq:4.53.3"
write_snapshot "$after" "jq:1.8.0" "yq:4.53.3"
line="$(unattended_log_change_line "$before" "$after" "formulae" "brew reports these" versions)"
# shellcheck disable=SC2016 # the backticks are Discord code-span syntax, not a substitution
grep -qF 'formulae: 1 of 2 tracked entries changed (`jq` `1.7.1` -> `1.8.0`).' <<<"$line" ||
  fail "the versions style did not render the transition: '$line'"

# ADDED and REMOVED are changes too. The removal is the single most worth-seeing
# line: something left without being asked to.
write_snapshot "$before" "alpha:aaa1"
write_snapshot "$after" "alpha:aaa1" "delta:ddd1"
# shellcheck disable=SC2016 # the backticks are Discord code-span syntax, not a substitution
grep -qF '1 of 2 tracked entries changed (`delta` (added)).' \
  <<<"$(unattended_log_change_line "$before" "$after" subject "$CAVEAT" opaque)" ||
  fail "an added entry was not reported"
write_snapshot "$before" "alpha:aaa1" "beta:bbb1"
write_snapshot "$after" "alpha:aaa1"
# shellcheck disable=SC2016 # the backticks are Discord code-span syntax, not a substitution
grep -qF '`beta` (removed)' \
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

# REMOVALS COUNT IN THE TOTAL. Counting only the after-rows renders the
# impossible "2 of 0 tracked entries changed" on an emptied snapshot, and a total
# that disagrees with its own count is a number nobody can act on.
write_snapshot "$before" "alpha:aaa1" "beta:bbb1"
: >"$after"
line="$(unattended_log_change_line "$before" "$after" subject "$CAVEAT" opaque)"
grep -qF 'subject: 2 of 2 tracked entries changed' <<<"$line" ||
  fail "an emptied snapshot rendered a count larger than its own total: '$line'"

# A NAME CARRYING A BACKSLASH matches itself. `jq @tsv` renders a newline as a
# literal backslash-n and any backslash as a doubled one, and awk's -v processes
# escape sequences in the VALUE, so the lookup for such a name silently missed
# and the entry was reported as removed AND re-added, every week, forever. The
# removal line is the one a reader trusts most, so it is the worst one to fake.
write_snapshot "$before" 'we\nird:aaa1' 'back\\slash:bbb1'
write_snapshot "$after" 'we\nird:aaa1' 'back\\slash:bbb1'
line="$(unattended_log_change_line "$before" "$after" subject "$CAVEAT" opaque)"
grep -qF 'subject: 0 of 2 tracked entries changed' <<<"$line" ||
  fail "a name carrying a backslash did not match itself, so it reads as removed and re-added: '$line'"

# THIRD-PARTY TEXT IS QUOTED, NOT RENDERED. Names and versions here are chosen by
# whoever published the package, and they land in a channel whose entire value is
# that its contents are trustworthy machine records. A masked link would render
# as a clickable link the operator never authored.
write_snapshot "$before" 'evil:1.0'
write_snapshot "$after" 'evil:[urgent: click here](https://evil.example)'
line="$(unattended_log_change_line "$before" "$after" subject "$CAVEAT" versions)"
# shellcheck disable=SC2016 # the backticks are Discord code-span syntax, not a substitution
grep -qF '`[urgent: click here](https://evil.example)`' <<<"$line" ||
  fail "a publisher-chosen version was not wrapped in a code span, so its markdown renders: '$line'"
refute '[^`]\[urgent' "$line" "the masked link escaped its code span: '$line'"

# ...and a version carrying a control character cannot forge a second entry or a
# second column.
write_snapshot "$before" 'sneak:1.0'
# shellcheck disable=SC2016 # the backticks are Discord code-span syntax, not a substitution
printf 'sneak\t%s\n' '2.0`x`' >"$after"
line="$(unattended_log_change_line "$before" "$after" subject "$CAVEAT" versions)"
# shellcheck disable=SC2016 # the backticks are Discord code-span syntax, not a substitution
refute '2\.0`x`' "$line" "a backtick in a version string was not stripped, so it can close the code span"

# ...nor can a CONTROL character. It is the other class the code-span comment
# names (a control character can break the span or the message framing), and
# the backtick test above passes with the control-char strip deleted
# (mutation-verified), so each needs its own pin. Checked with a bash pattern
# match, not grep: grep is line-oriented and a carriage return splits its view.
write_snapshot "$before" 'ctl:1.0'
printf 'ctl\t2.0%b3.0\n' '\r' >"$after"
line="$(unattended_log_change_line "$before" "$after" subject "$CAVEAT" versions)"
[[ $line != *$'\r'* ]] ||
  fail "a carriage return in a publisher-chosen version survived into the rendered entry"
grep -qF '2.03.0' <<<"$line" ||
  fail "the control character was not stripped in place (want the joined 2.03.0): '$line'"

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

printf 'unattended-log-lib: OK (elapsed boundaries in both directions, a backwards clock named; the gap line reports never/recorded/unreadable and reads a leading-zero epoch in base 10; the entry header takes ONE clock reading and keeps its gap line when the clock breaks; the week claim is atomic under %d concurrent racers and an exclusive create by the symlink probe, survives a corrupt guard, an unusable guard path, an unknown class and an unreadable week, admits one entry per class with completed-first refusing a late deferral, releases per class without freeing the sibling, and prunes; delivery reports its outcome either way, closes fd 9, names its host, and alerts the priority route once a week when the channel is broken while never spending the alert token for a week on a call it could not make; the change line counts removals in the total, matches names carrying backslashes, quotes third-party text incl. control characters and caps a whole-subject move)\n' "$claim_racers"
