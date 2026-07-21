# shellcheck shell=bash
# build-dispatch-harness.sh -- stand up a throwaway HOME for exercising the REAL
# osquery dispatch library (dot_local/libexec/osquery/executable_alert-dispatch.sh).
# Sourced by the dispatch suites; no main.
#
# The harness records what the library does through two stubs on PATH: an alerter
# stub that appends its argv to $ALERTER_LOG (so a test can read the local
# notification's text) and a curl stub that appends its argv to $CURL_LOG and
# returns the next HTTP code queued in $CURL_CODES_FILE (so a test can script
# "503 503 503 then success"). It then exports the paths and secret the library
# reads and sources the library into the test's shell, so send_alert and
# retry_undelivered_alerts run for real.
#
# Being a fixture, this helper's whole contract is to EXPORT that environment;
# unlike the pure helpers it owns the harness state. It sets nothing at source
# time -- every export happens inside build_dispatch_harness, called from setup().

# build_dispatch_harness -- create the temp HOME, install the recording stubs,
# export the library's inputs, and source the library.
build_dispatch_harness() {
  HARNESS_HOME="$(mktemp -d)"
  # Record ownership ONLY after we created our own temp dir, so teardown removes
  # this path and never a pre-set or inherited HARNESS_HOME.
  _DISPATCH_HARNESS_OWNED_DIR="$HARNESS_HOME"
  mkdir -p "$HARNESS_HOME/bin" "$HARNESS_HOME/.local/log/osquery"

  export ALERTER_LOG="$HARNESS_HOME/alerter.log"
  : >"$ALERTER_LOG"
  printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$*" >>"%s"\nexit 0\n' "$ALERTER_LOG" >"$HARNESS_HOME/bin/alerter"

  # curl stub: record the invocation and the count of stored pending_alerts rows
  # AT CALL TIME (so a test can prove the write-ahead row already existed when
  # the first POST was attempted), then emit the next queued HTTP code (one per
  # line in $CURL_CODES_FILE, popped per call), defaulting to 200 when the queue
  # is empty.
  cat >"$HARNESS_HOME/bin/curl" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$CURL_LOG"
if [[ -n "${CURL_DB_PERSIST_WITNESS:-}" && -f "${OSQUERY_UNDELIVERED_ALERTS_DB:-/nonexistent}" ]]; then
  sqlite3 -readonly "$OSQUERY_UNDELIVERED_ALERTS_DB" 'SELECT COUNT(*) FROM pending_alerts;' 2>/dev/null >>"$CURL_DB_PERSIST_WITNESS" || true
fi
code=200
if [[ -s "$CURL_CODES_FILE" ]]; then
  code=$(head -1 "$CURL_CODES_FILE")
  tail -n +2 "$CURL_CODES_FILE" >"$CURL_CODES_FILE.tmp" 2>/dev/null && mv "$CURL_CODES_FILE.tmp" "$CURL_CODES_FILE"
fi
printf '%s' "$code"
STUB
  chmod +x "$HARNESS_HOME/bin/alerter" "$HARNESS_HOME/bin/curl"

  export CURL_LOG="$HARNESS_HOME/curl.log"
  : >"$CURL_LOG"
  export CURL_CODES_FILE="$HARNESS_HOME/curl_codes"
  : >"$CURL_CODES_FILE"
  # The write-ahead witness: the curl stub appends the pending_alerts row count
  # seen at each POST here, one line per call.
  export CURL_DB_PERSIST_WITNESS="$HARNESS_HOME/curl_db_persist_witness"
  : >"$CURL_DB_PERSIST_WITNESS"
  export PATH="$HARNESS_HOME/bin:$PATH"
  export HOME="$HARNESS_HOME"

  export OSQUERY_WEBHOOK_SECRET="testsecret"
  export OSQUERY_DELIVERY_LOG="$HARNESS_HOME/.local/log/osquery/webhook-delivery.log"
  export OSQUERY_UNDELIVERED_ALERTS_DB="$HARNESS_HOME/.local/state/osquery-undelivered-alerts.sqlite3"
  export OSQUERY_RETRY_BACKOFF_BASE=0 # do not really sleep between retries in tests

  DISPATCH="${BATS_TEST_DIRNAME}/../../dot_local/libexec/osquery/executable_alert-dispatch.sh"
  # shellcheck source=/dev/null
  source "$DISPATCH"
}

# teardown_dispatch_harness -- remove ONLY a temp dir this harness created. The
# ownership marker is set by build_dispatch_harness after its own mktemp, so a
# pre-set or inherited HARNESS_HOME (marker unset) is left untouched.
teardown_dispatch_harness() {
  [[ -n ${_DISPATCH_HARNESS_OWNED_DIR:-} ]] || return 0
  rm -rf "$_DISPATCH_HARNESS_OWNED_DIR"
  unset _DISPATCH_HARNESS_OWNED_DIR
}

# set_curl_codes <code>... -- queue the HTTP codes the curl stub returns, one per
# send/retry POST.
set_curl_codes() {
  printf '%s\n' "$@" >"$CURL_CODES_FILE"
}

# sqlite3_query <sql> -- run a read-only query against the undelivered-alerts DB
# and print its output. The inspection path for the SQLite store: a fresh
# read-only connection never mutates what the library persisted.
sqlite3_query() {
  sqlite3 -readonly "$OSQUERY_UNDELIVERED_ALERTS_DB" "$1"
}

# assert_pending_alert_count <n> -- exactly <n> rows sit in the pending_alerts
# table. A missing DB counts as zero rows (nothing has been stored yet).
assert_pending_alert_count() {
  local count=0
  if [[ -f $OSQUERY_UNDELIVERED_ALERTS_DB ]]; then
    count=$(sqlite3 -readonly "$OSQUERY_UNDELIVERED_ALERTS_DB" 'SELECT COUNT(*) FROM pending_alerts;' 2>/dev/null || echo 0)
  fi
  if [[ $count -ne $1 ]]; then
    printf 'expected %s pending_alerts row(s), got %s\n' "$1" "$count" >&2
    return 1
  fi
}

# assert_mode <octal> <path> -- the path carries the expected permission bits.
# GNU stat first (the nix shell), BSD stat as the fallback (the portable order).
assert_mode() {
  local mode
  mode=$(stat -c '%a' "$2" 2>/dev/null || stat -f '%Lp' "$2" 2>/dev/null)
  if [[ $mode != "$1" ]]; then
    printf 'expected mode %s on %s, got %s\n' "$1" "$2" "$mode" >&2
    return 1
  fi
}

# assert_post_count <n> -- the curl stub recorded exactly <n> POSTs.
assert_post_count() {
  local count
  count=$(grep -c 'POST' "$CURL_LOG" 2>/dev/null || echo 0)
  if [[ $count -ne $1 ]]; then
    printf 'expected %s POST(s), got %s: %s\n' "$1" "$count" "$(cat "$CURL_LOG")" >&2
    return 1
  fi
}

# assert_posted_to <substring> -- some POST targeted a URL containing <substring>.
assert_posted_to() {
  if ! grep -qF -- "$1" "$CURL_LOG"; then
    printf 'expected a POST to a URL containing %s; curl log: %s\n' "$1" "$(cat "$CURL_LOG")" >&2
    return 1
  fi
}

# assert_no_post -- no webhook POST happened at all.
assert_no_post() {
  if [[ -s $CURL_LOG ]]; then
    printf 'expected NO webhook POST, but curl was called: %s\n' "$(cat "$CURL_LOG")" >&2
    return 1
  fi
}
