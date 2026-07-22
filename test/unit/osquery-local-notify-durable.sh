#!/usr/bin/env bash
#
# _osquery_notify_local_durable makes the loud local CRIT channel durable: it
# fires _loud_local and, ONLY when the banner fails, persists the notification
# as a pending_local_notifications row in the alert store (same database, same
# lock domain), so a later drain can retry it. A shown banner persists nothing
# and must not even bootstrap the table. The persist follows the alert store's
# crash-safety rules: schema bootstrap and INSERT in one atomic batch,
# idempotent on notification_id, quote-safe for arbitrary title/message text,
# and fail-soft (a store failure logs LOCAL-NOTIFY-STORE-FAILED, never aborts
# the caller).
#
# Unit test: stubbed notifier binaries on PATH, throwaway store per pin.
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

# (a) Failing notifier: exactly ONE row persisted, fields intact.
db="$work/a/store.sqlite3"
log="$work/a/delivery.log"
set_alerter_stub 64
out="$(run_durable "$db" "$log" "banner title" "banner message")" || fail "(a) wrapper aborted its caller"
[[ $out == *SURVIVED* ]] || fail "(a) caller did not survive"
[[ "$(query "$db" 'SELECT COUNT(*) FROM pending_local_notifications;')" == "1" ]] ||
  fail "(a) expected exactly one persisted row"
row="$(query "$db" "SELECT title, message, sound, attempts, next_attempt_after FROM pending_local_notifications;")"
[[ $row == "banner title|banner message|Funk|0|0" ]] ||
  fail "(a) persisted fields wrong: $row"
notification_id="$(query "$db" 'SELECT notification_id FROM pending_local_notifications;')"
[[ -n $notification_id ]] || fail "(a) empty notification_id"
occurrence_ts="$(query "$db" 'SELECT occurrence_ts FROM pending_local_notifications;')"
[[ $occurrence_ts =~ ^[0-9]+$ && $occurrence_ts -gt 0 ]] || fail "(a) occurrence_ts not a real timestamp: $occurrence_ts"
# The schema carries every column the retry drain (T3) will consume.
columns="$(query "$db" "SELECT group_concat(name, ',') FROM pragma_table_info('pending_local_notifications');")"
for column in sequence_number notification_id occurrence_ts title message sound attempts next_attempt_after created_at; do
  [[ ",$columns," == *",$column,"* ]] || fail "(a) schema missing column $column (got: $columns)"
done

# (b) Succeeding notifier: nothing persisted, table NOT bootstrapped, and a
# never-existing database file stays absent.
db="$work/b/store.sqlite3"
log="$work/b/delivery.log"
set_alerter_stub 0
out="$(run_durable "$db" "$log" "shown title" "shown message")" || fail "(b) wrapper aborted its caller"
[[ ! -e $db ]] || fail "(b) a SHOWN banner created the store"

# (b2) Succeeding notifier against an EXISTING store: the table is still not
# bootstrapped (no schema work on the success path).
db="$work/b2/store.sqlite3"
mkdir -p "$work/b2"
sqlite3 "$db" 'CREATE TABLE unrelated (x);'
out="$(run_durable "$db" "$work/b2/delivery.log" "shown title" "shown message")" || fail "(b2) wrapper aborted its caller"
[[ "$(query "$db" "SELECT COUNT(*) FROM sqlite_master WHERE name='pending_local_notifications';")" == "0" ]] ||
  fail "(b2) the success path bootstrapped the table"

# (c) The same notification seed persisted twice stays ONE row (idempotent).
db="$work/c/store.sqlite3"
log="$work/c/delivery.log"
set_alerter_stub 64
run_durable "$db" "$log" "t" "m" "same-seed" >/dev/null || fail "(c) first persist aborted"
run_durable "$db" "$log" "t" "m" "same-seed" >/dev/null || fail "(c) second persist aborted"
[[ "$(query "$db" 'SELECT COUNT(*) FROM pending_local_notifications;')" == "1" ]] ||
  fail "(c) the same seed stored twice kept more than one row"

# (d) A store failure is fail-soft: unwritable DB parent, the set -e caller
# survives and the loud LOCAL-NOTIFY-STORE-FAILED line lands in the log.
mkdir -p "$work/d"
printf 'a file, not a dir\n' >"$work/d/notadir"
db="$work/d/notadir/store.sqlite3"
log="$work/d/delivery.log"
set_alerter_stub 64
out="$(run_durable "$db" "$log" "lost title" "lost message")" || fail "(d) a store failure aborted the caller"
[[ $out == *SURVIVED* ]] || fail "(d) caller did not survive the store failure"
grep -q 'LOCAL-NOTIFY-STORE-FAILED' "$log" || fail "(d) no LOCAL-NOTIFY-STORE-FAILED log line"

# (e) Quote safety: apostrophes and SQL-looking text round-trip byte-identical.
db="$work/e/store.sqlite3"
log="$work/e/delivery.log"
set_alerter_stub 64
tricky_title="it's broken, isn't it?"
tricky_message="the operator's page: '); DROP TABLE pending_local_notifications; --"
run_durable "$db" "$log" "$tricky_title" "$tricky_message" >/dev/null || fail "(e) persist aborted"
[[ "$(query "$db" 'SELECT COUNT(*) FROM pending_local_notifications;')" == "1" ]] ||
  fail "(e) quote-carrying notification did not persist as one row"
[[ "$(query "$db" 'SELECT title FROM pending_local_notifications;')" == "$tricky_title" ]] ||
  fail "(e) title did not round-trip intact"
[[ "$(query "$db" 'SELECT message FROM pending_local_notifications;')" == "$tricky_message" ]] ||
  fail "(e) message did not round-trip intact"

printf 'osquery-local-notify-durable: OK (persist-on-fail, silent-on-success, idempotent, fail-soft, quote-safe)\n'
