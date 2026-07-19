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
  mkdir -p "$HARNESS_HOME/bin" "$HARNESS_HOME/.local/log/osquery"

  export ALERTER_LOG="$HARNESS_HOME/alerter.log"
  : >"$ALERTER_LOG"
  printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$*" >>"%s"\nexit 0\n' "$ALERTER_LOG" >"$HARNESS_HOME/bin/alerter"

  # curl stub: record the invocation, then emit the next queued HTTP code (one
  # per line in $CURL_CODES_FILE, popped per call), defaulting to 200 when the
  # queue is empty.
  cat >"$HARNESS_HOME/bin/curl" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$CURL_LOG"
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
  export PATH="$HARNESS_HOME/bin:$PATH"
  export HOME="$HARNESS_HOME"

  export OSQUERY_WEBHOOK_SECRET="testsecret"
  export OSQUERY_DELIVERY_LOG="$HARNESS_HOME/.local/log/osquery/webhook-delivery.log"
  export OSQUERY_UNDELIVERED_ALERTS_DIR="$HARNESS_HOME/.local/state/osquery-undelivered-alerts"
  export OSQUERY_RETRY_BACKOFF_BASE=0 # do not really sleep between retries in tests

  DISPATCH="${BATS_TEST_DIRNAME}/../../dot_local/libexec/osquery/executable_alert-dispatch.sh"
  # shellcheck source=/dev/null
  source "$DISPATCH"
}

# teardown_dispatch_harness -- remove the temp HOME (safe when setup never ran).
teardown_dispatch_harness() {
  [[ -n ${HARNESS_HOME:-} ]] && rm -rf "$HARNESS_HOME"
}

# set_curl_codes <code>... -- queue the HTTP codes the curl stub returns, one per
# send/retry POST.
set_curl_codes() {
  printf '%s\n' "$@" >"$CURL_CODES_FILE"
}

# first_undelivered_alert_file -- print the path of one stored undelivered-alert
# file, or nothing when none exist.
first_undelivered_alert_file() {
  find "$OSQUERY_UNDELIVERED_ALERTS_DIR" -type f 2>/dev/null | head -1
}

# assert_undelivered_alert_count <n> -- exactly <n> undelivered-alert files exist.
assert_undelivered_alert_count() {
  local count=0
  [[ -d $OSQUERY_UNDELIVERED_ALERTS_DIR ]] &&
    count=$(find "$OSQUERY_UNDELIVERED_ALERTS_DIR" -type f 2>/dev/null | wc -l | tr -d ' ')
  if [[ $count -ne $1 ]]; then
    printf 'expected %s undelivered-alert file(s), got %s: %s\n' \
      "$1" "$count" "$(ls -la "$OSQUERY_UNDELIVERED_ALERTS_DIR" 2>/dev/null)" >&2
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
