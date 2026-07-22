#!/usr/bin/env bash
#
# A failed or unconfirmed local banner ALWAYS has a durable row until the
# notifier confirms it was shown. _osquery_notify_local_durable is WRITE-AHEAD:
# it persists the pending_local_notifications row FIRST (atomic bootstrap +
# INSERT, same database and lock domain as the alert queue), then attempts the
# banner, and the row is deleted only on CONFIRMED success (the alerter watcher
# sees exit 0; the synchronous osascript fallback confirms inline). The DB is
# the truth; the caller-facing return status is advisory logging only. This
# closes two holes the old persist-on-failure design had: a kill between a
# failed banner and the persist lost the banner entirely, and an alerter that
# outlived the grace window and THEN failed was treated as delivered.
#
# Unit test: stubbed notifier binaries on PATH, throwaway store per pin,
# per-assert expectation messages.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DISPATCH="$REPO_ROOT/dot_local/libexec/osquery/executable_alert-dispatch.sh"

fail() {
  printf 'osquery-local-notify-durable: FAIL -- %s\n' "$*" >&2
  exit 1
}

command -v sqlite3 >/dev/null 2>&1 || {
  printf 'SKIP: sqlite3 not on PATH; cannot exercise the local-notification store\n'
  exit 0
}
[[ -f $DISPATCH ]] || fail "missing dispatch library: $DISPATCH"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin"
stub_path="$work/bin:/usr/bin:/bin"

set_alerter_stub() { # <exit-code>
  printf '#!/usr/bin/env bash\nexit %s\n' "$1" >"$work/bin/alerter"
  chmod +x "$work/bin/alerter"
}

# run_durable <db-path> <log-path> <args...> -- source the library against the
# given store in a clean set -e subshell and call the wrapper once.
run_durable() {
  local db="$1" log="$2"
  shift 2
  PATH="$stub_path" OSQUERY_UNDELIVERED_ALERTS_DB="$db" OSQUERY_DELIVERY_LOG="$log" \
    bash -c 'set -euo pipefail; source "$1"; shift; _osquery_notify_local_durable "$@"; echo SURVIVED' \
    _ "$DISPATCH" "$@"
}

query() { # <db> <sql>
  sqlite3 -readonly "$1" "$2"
}

row_count() { # <db> -- a missing DB or table reads as zero rows
  sqlite3 -readonly "$1" 'SELECT COUNT(*) FROM pending_local_notifications;' 2>/dev/null || echo 0
}

wait_for_row_count() { # <db> <expected> -- the alerter confirm is a WATCHER, so poll
  local db="$1" expected="$2" attempt count
  for ((attempt = 0; attempt < 30; attempt++)); do
    count="$(row_count "$db")"
    [[ $count == "$expected" ]] && return 0
    sleep 0.1
  done
  printf 'expected %s row(s) once the watcher settled, still %s\n' "$expected" "$count" >&2
  return 1
}

# --- a failing banner leaves exactly one durable row, fields intact -----------
db="$work/a/store.sqlite3"
log="$work/a/delivery.log"
set_alerter_stub 64
out="$(run_durable "$db" "$log" "banner title" "banner message")" || fail "(a) wrapper aborted its caller"
[[ $out == *SURVIVED* ]] || fail "(a) caller did not survive"
[[ "$(row_count "$db")" == "1" ]] ||
  fail "(a) a failed banner must leave exactly one durable row"
row="$(query "$db" "SELECT title, message, sound, attempts, next_attempt_after FROM pending_local_notifications;")"
[[ $row == "banner title|banner message|Funk|0|0" ]] ||
  fail "(a) persisted fields wrong: $row"
notification_id="$(query "$db" 'SELECT notification_id FROM pending_local_notifications;')"
[[ -n $notification_id ]] || fail "(a) empty notification_id"
occurrence_ts="$(query "$db" 'SELECT occurrence_ts FROM pending_local_notifications;')"
[[ $occurrence_ts =~ ^[0-9]+$ && $occurrence_ts -gt 0 ]] || fail "(a) occurrence_ts not a real timestamp: $occurrence_ts"
columns="$(query "$db" "SELECT group_concat(name, ',') FROM pragma_table_info('pending_local_notifications');")"
for column in sequence_number notification_id occurrence_ts title message sound attempts next_attempt_after created_at; do
  [[ ",$columns," == *",$column,"* ]] || fail "(a) schema missing column $column (got: $columns)"
done

# --- a banner confirmed shown leaves no row behind (alerter watcher) ----------
# Write-ahead means the row EXISTS during the attempt; the watcher's confirmed
# exit 0 deletes it. (The old no-bootstrap-on-success property is gone by
# design: durability beats a pristine success path.)
db="$work/b/store.sqlite3"
log="$work/b/delivery.log"
set_alerter_stub 0
out="$(run_durable "$db" "$log" "shown title" "shown message")" || fail "(b) wrapper aborted its caller"
wait_for_row_count "$db" 0 ||
  fail "(b) a CONFIRMED banner must leave no row once the watcher settles"

# --- the synchronous osascript fallback confirms and deletes inline -----------
db="$work/b2/store.sqlite3"
log="$work/b2/delivery.log"
rm -f "$work/bin/alerter"
printf '#!/usr/bin/env bash\nexit 0\n' >"$work/bin/osascript"
chmod +x "$work/bin/osascript"
out="$(run_durable "$db" "$log" "fallback title" "fallback message")" || fail "(b2) wrapper aborted its caller"
[[ "$(row_count "$db")" == "0" ]] ||
  fail "(b2) the synchronous fallback's exit 0 must delete the row inline, no watcher needed"
rm -f "$work/bin/osascript"

# --- a caller killed mid-banner still leaves a durable row (kill window) ------
# The banner attempt is held open (the stub sleeps) and the CALLER is SIGKILLed
# mid-attempt: with write-ahead the row was committed BEFORE the attempt, so
# the kill cannot lose the banner. The old persist-on-failure design had
# nothing on disk at this instant.
db="$work/f/store.sqlite3"
log="$work/f/delivery.log"
marker="$work/f/banner-attempt-started"
mkdir -p "$work/f"
cat >"$work/bin/alerter" <<STUB
#!/usr/bin/env bash
touch '$marker'
sleep 30
exit 0
STUB
chmod +x "$work/bin/alerter"
PATH="$stub_path" OSQUERY_UNDELIVERED_ALERTS_DB="$db" OSQUERY_DELIVERY_LOG="$log" \
  bash -c 'set -euo pipefail; source "$1"; _osquery_notify_local_durable "kill title" "kill message" "seed-kill"' \
  _ "$DISPATCH" &
caller_pid=$!
for _ in $(seq 1 50); do
  [[ -f $marker ]] && break
  sleep 0.1
done
[[ -f $marker ]] || fail "(f) the banner attempt never started (stub marker missing)"
kill -KILL "$caller_pid" 2>/dev/null || true
wait "$caller_pid" 2>/dev/null || true
[[ "$(row_count "$db")" == "1" ]] ||
  fail "(f) a caller killed mid-banner must still leave the durable row (write-ahead)"

# --- an alerter that outlives the grace window and THEN fails stays durable ---
# sol's repro: sleep past the ~0.6s grace window, then exit nonzero. The old
# design read still-alive as delivered and had persisted nothing; write-ahead
# plus watcher-confirm keeps the row because no confirmation ever arrived.
db="$work/g/store.sqlite3"
log="$work/g/delivery.log"
printf '#!/usr/bin/env bash\nsleep 0.7\nexit 64\n' >"$work/bin/alerter"
chmod +x "$work/bin/alerter"
out="$(run_durable "$db" "$log" "late fail title" "late fail message")" || fail "(g) wrapper aborted its caller"
[[ "$(row_count "$db")" == "1" ]] ||
  fail "(g) the row must exist the moment the wrapper returns (write-ahead)"
sleep 1.2 # let the watcher see the late nonzero exit
[[ "$(row_count "$db")" == "1" ]] ||
  fail "(g) a banner that failed after the grace window must keep its durable row"

# --- the same notification seed persisted twice stays ONE row (idempotent) ----
db="$work/c/store.sqlite3"
log="$work/c/delivery.log"
set_alerter_stub 64
run_durable "$db" "$log" "t" "m" "same-seed" >/dev/null || fail "(c) first persist aborted"
run_durable "$db" "$log" "t" "m" "same-seed" >/dev/null || fail "(c) second persist aborted"
[[ "$(row_count "$db")" == "1" ]] ||
  fail "(c) the same seed stored twice must keep exactly one row"

# --- a broken store is fail-soft: loud log, caller survives, banner still fires
mkdir -p "$work/d"
printf 'a file, not a dir\n' >"$work/d/notadir"
db="$work/d/notadir/store.sqlite3"
log="$work/d/delivery.log"
set_alerter_stub 64
out="$(run_durable "$db" "$log" "lost title" "lost message")" || fail "(d) a store failure aborted the caller"
[[ $out == *SURVIVED* ]] || fail "(d) caller did not survive the store failure"
grep -q 'LOCAL-NOTIFY-STORE-FAILED' "$log" || fail "(d) no LOCAL-NOTIFY-STORE-FAILED log line"

# --- apostrophes and SQL-looking text round-trip byte-identical ---------------
db="$work/e/store.sqlite3"
log="$work/e/delivery.log"
set_alerter_stub 64
tricky_title="it's broken, isn't it?"
tricky_message="the operator's page: '); DROP TABLE pending_local_notifications; --"
run_durable "$db" "$log" "$tricky_title" "$tricky_message" >/dev/null || fail "(e) persist aborted"
[[ "$(row_count "$db")" == "1" ]] ||
  fail "(e) quote-carrying notification did not persist as one row"
[[ "$(query "$db" 'SELECT title FROM pending_local_notifications;')" == "$tricky_title" ]] ||
  fail "(e) title did not round-trip intact"
[[ "$(query "$db" 'SELECT message FROM pending_local_notifications;')" == "$tricky_message" ]] ||
  fail "(e) message did not round-trip intact"

printf 'osquery-local-notify-durable: OK (write-ahead, confirmed-delete, kill-safe, late-fail-safe, idempotent, fail-soft, quote-safe)\n'
