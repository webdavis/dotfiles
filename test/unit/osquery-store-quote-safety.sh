#!/usr/bin/env bash
#
# The alert-row helpers must double embedded single quotes CORRECTLY under
# EVERY bash the library can run on. Their old escape shape (an unquoted \'
# pattern inside a double-quoted expansion) is bash-version-dependent,
# verified both ways: bash 5 removes the backslash and doubles correctly,
# while bash 3.2 (/bin/bash, the macOS system bash) keeps the backslash and
# corrupts the SQL. The values are quote-free by construction TODAY (hex
# request ids, base64 bodies, fixed reasons), and the URL comes from the
# environment: an apostrophe URL passes every store-time validation
# (non-empty, no whitespace or control characters) and reaches the INSERT,
# where under system bash the corrupted SQL failed the store, a FALSE loud
# rejection of a storable page (verified: rejection, not silent corruption).
# The fix (quote doubling via a helper variable) behaves identically on both
# bashes and turns the false rejection into correct storage.
#
# Unit test: the round-trips run inside /bin/bash when it exists (the macOS
# system bash 3.2, the strict case); elsewhere plain bash. A quote-carrying
# URL, dead-letter reason, and request id all round-trip byte-identical.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DISPATCH="$REPO_ROOT/dot_local/libexec/osquery/executable_alert-dispatch.sh"

fail() {
  printf 'osquery-store-quote-safety: FAIL -- %s\n' "$*" >&2
  exit 1
}

command -v sqlite3 >/dev/null 2>&1 || {
  printf 'SKIP: sqlite3 not on PATH; cannot exercise the store helpers\n'
  exit 0
}
[[ -f $DISPATCH ]] || fail "missing dispatch library: $DISPATCH"

# The strict interpreter: macOS system bash 3.2 when present, else plain bash.
strict_bash="/bin/bash"
[[ -x $strict_bash ]] || strict_bash="$(command -v bash)"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
export OSQUERY_UNDELIVERED_ALERTS_DB="$work/store.sqlite3"
export OSQUERY_DELIVERY_LOG="$work/delivery.log"

query() { sqlite3 -readonly "$OSQUERY_UNDELIVERED_ALERTS_DB" "$1"; }

# Drive every store helper inside ONE strict-bash script (source once, run the
# whole scenario), so the escapes execute under the version being pinned.
"$strict_bash" -s <<STRICT_SCRIPT || fail "the strict-bash round-trip script failed (see stderr above)"
set -euo pipefail
source '$DISPATCH'
body_b64="\$(printf '%s' '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')"

# (1) An apostrophe URL inside the allowed localhost prefix must STORE.
_osquery_store_alert_row 1000 osquery-apos-url \
  "http://127.0.0.1:8644/webhooks/o'brien-priority" "\$body_b64" || {
  echo "(1) the apostrophe URL was rejected by the store (corrupted SQL)" >&2
  exit 1
}

# (2) A dead-letter reason carrying an apostrophe must complete the move.
_osquery_store_alert_row 2000 osquery-apos-reason \
  'http://127.0.0.1:8644/webhooks/osquery-priority' "\$body_b64" || {
  echo "(2) plain store failed" >&2
  exit 1
}
_osquery_dead_letter_alert_row osquery-apos-reason none \
  "operator's note: gateway refused the page" || {
  echo "(2) the apostrophe reason failed the dead-letter move" >&2
  exit 1
}

# (3) An apostrophe request id must survive bookkeeping and delete-by-id.
_osquery_store_alert_row 3000 "osquery-o'brien" \
  'http://127.0.0.1:8644/webhooks/osquery-priority' "\$body_b64" || {
  echo "(3) store with an apostrophe request id failed" >&2
  exit 1
}
_osquery_record_transient_failure "osquery-o'brien" || {
  echo "(3) transient bookkeeping failed on the apostrophe id" >&2
  exit 1
}
_osquery_delete_alert_row "osquery-o'brien" || {
  echo "(3) delete failed on the apostrophe id" >&2
  exit 1
}
STRICT_SCRIPT

# Round-trip assertions on the stored bytes (from THIS shell; reads only).
apostrophe_url="http://127.0.0.1:8644/webhooks/o'brien-priority"
stored_url="$(query "SELECT url FROM pending_alerts WHERE request_id='osquery-apos-url';")"
[[ $stored_url == "$apostrophe_url" ]] ||
  fail "(1) URL did not round-trip intact: stored '$stored_url'"

# ...and the drain SELECT carries it intact (request id and URL on one row).
# shellcheck source=/dev/null
source "$DISPATCH"
drain_rows="$(_osquery_pending_alert_rows)" || fail "(1) drain SELECT failed"
printf '%s\n' "$drain_rows" | grep -qF "osquery-apos-url	$apostrophe_url" ||
  fail "(1) the drain SELECT does not carry the apostrophe URL intact: $drain_rows"

stored_reason="$(query "SELECT reason FROM dead_letter_alerts WHERE request_id='osquery-apos-reason';")"
[[ $stored_reason == "operator's note: gateway refused the page" ]] ||
  fail "(2) reason did not round-trip intact: stored '$stored_reason'"
[[ "$(query 'SELECT COUNT(*) FROM pending_alerts WHERE request_id="osquery-apos-reason";')" == "0" ]] ||
  fail "(2) the moved row is still pending"

[[ "$(query "SELECT COUNT(*) FROM pending_alerts WHERE request_id='osquery-o''brien';")" == "0" ]] ||
  fail "(3) the apostrophe-id row was not deleted"

printf 'osquery-store-quote-safety: OK (url, reason, and request id round-trip under %s)\n' "$strict_bash"
