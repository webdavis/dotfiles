#!/usr/bin/env bats
# The scheduled drainer (executable_drain-undelivered-alerts.sh) sweeps the
# undelivered-alerts SQLite store on a timer. Two drains that overlap -- the
# StartInterval fires again while a slow drain is still running -- must never
# both replay the same stored page: a single-instance lock serializes them so
# each stored page reaches the webhook AT MOST ONCE across the overlapping runs.
# Without the lock both runs read the same row snapshot and double-send every
# page. This runs the drainer as a real subprocess and widens the delivery
# window with a slow POST stub, so it is a whole-script, timing-bound flow (the
# e2e suite, beside the osquery durability suite it builds on).

setup() {
  local helpers="$BATS_TEST_DIRNAME/../helpers"
  # shellcheck source=test/helpers/build-dispatch-harness.sh
  source "$helpers/build-dispatch-harness.sh"
  build_dispatch_harness
  # The drainer sources the library from its DEPLOYED path
  # ($HOME/.local/libexec/osquery/alert-dispatch.sh, the same path the three
  # producers use). Mirror a chezmoi apply by copying the source library (with
  # its executable_ prefix stripped) into the harness HOME, so the drainer
  # subprocess finds it exactly where it will in production.
  mkdir -p "$HARNESS_HOME/.local/libexec/osquery"
  cp "$DISPATCH" "$HARNESS_HOME/.local/libexec/osquery/alert-dispatch.sh"
  DRAINER="$BATS_TEST_DIRNAME/../../dot_local/libexec/osquery/executable_drain-undelivered-alerts.sh"
}
teardown() { teardown_dispatch_harness; }

@test "T-DRAIN-lock-single-send: two overlapping drains deliver each stored page exactly once" {
  # The single-instance lock is a kernel lock (/usr/bin/lockf). A host without
  # it (any non-darwin box) runs the drain unlocked by design, so the lock
  # cannot be exercised there; skip rather than assert a guarantee the platform
  # does not provide.
  [[ -x /usr/bin/lockf ]] || skip "no /usr/bin/lockf; the single-instance lock is a darwin-only guarantee"

  # Seed three undelivered pages directly as pending_alerts rows.
  local url='http://127.0.0.1:8644/webhooks/osquery-priority' body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  _osquery_store_alert_row 1000 osquery-drain-a "$url" "$body_b64"
  _osquery_store_alert_row 2000 osquery-drain-b "$url" "$body_b64"
  _osquery_store_alert_row 3000 osquery-drain-c "$url" "$body_b64"

  # A deliberately slow POST holds the delivery window open long enough that an
  # UNLOCKED pair of drains reliably overlaps and double-sends. Each POST is
  # logged and returns 200 so the winning drain clears every row.
  cat >"$HARNESS_HOME/bin/curl" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$CURL_LOG"
sleep 0.3
printf '200'
STUB
  chmod +x "$HARNESS_HOME/bin/curl"
  : >"$CURL_LOG"

  # Fire two overlapping drain runs, exactly as two StartInterval ticks would.
  bash "$DRAINER" &
  local first_pid=$!
  bash "$DRAINER" &
  local second_pid=$!
  local first_status=0 second_status=0
  wait "$first_pid" || first_status=$?
  wait "$second_pid" || second_status=$?
  [[ $first_status -eq 0 ]]  # a drain always exits 0 (best-effort background sweep)
  [[ $second_status -eq 0 ]]

  # The bound: every seeded page reached the webhook exactly once across BOTH
  # runs. The winner (whichever drain took the lock) delivers all three; the
  # other skips immediately, so no page is POSTed twice.
  local page post_count
  for page in osquery-drain-a osquery-drain-b osquery-drain-c; do
    post_count=$(grep -cF "X-Request-ID: $page" "$CURL_LOG" || true)
    if [[ $post_count -ne 1 ]]; then
      printf 'page %s was POSTed %s time(s), expected exactly 1: the drains were not serialized\n' \
        "$page" "$post_count" >&2
      return 1
    fi
  done
  # Delivery really happened: the store is empty, no row left behind.
  assert_pending_alert_count 0
}

# --- fail-closed lock setup (a mutual-exclusion lock must never run unlocked) ---

# Seed one deliverable page and queue a 200: if the sweep RUNS it delivers, if it
# is SKIPPED nothing is POSTed and the row is retained. Sets a present lockf stub
# so the "lock required" path is reached on any platform (the stub is never
# actually called; the setup failure happens before the acquire).
_seed_one_page_and_require_lock() {
  local url='http://127.0.0.1:8644/webhooks/osquery-priority' body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  _osquery_store_alert_row 1000 osquery-failclosed "$url" "$body_b64"
  : >"$CURL_LOG"
  set_curl_codes 200
  printf '#!/usr/bin/env bash\nexit 0\n' >"$HARNESS_HOME/bin/lockf-stub"
  chmod +x "$HARNESS_HOME/bin/lockf-stub"
  export OSQUERY_DRAIN_LOCKF_BIN="$HARNESS_HOME/bin/lockf-stub"
}

@test "T-DRAIN-lock-failclosed-exec: a lockfile that cannot be opened SKIPS the sweep, never runs unlocked" {
  _seed_one_page_and_require_lock
  # Point the lock file at a DIRECTORY so `exec 9>>` cannot open it for writing:
  # a genuine lock-setup failure. A fail-closed lock must SKIP the sweep rather
  # than fall through and run unlocked (two overlapping runs would double-POST).
  export OSQUERY_DRAIN_LOCK_FILE="$HARNESS_HOME/lock-is-a-directory"
  mkdir -p "$OSQUERY_DRAIN_LOCK_FILE"

  run bash "$DRAINER"

  [[ $status -eq 0 ]]          # main still exits 0 (a skip is a clean no-op, not an error)
  assert_no_post               # the sweep was SKIPPED, not run unlocked
  assert_pending_alert_count 1 # the row is retained for the next 300s tick
}

@test "T-DRAIN-lock-failclosed-mkdir: a lock dir that cannot be created SKIPS the sweep, never runs unlocked" {
  _seed_one_page_and_require_lock
  # Put the lock file UNDER a regular file, so mkdir -p of its parent fails.
  printf 'i am a file, not a directory\n' >"$HARNESS_HOME/a-file"
  export OSQUERY_DRAIN_LOCK_FILE="$HARNESS_HOME/a-file/drain.lock"

  run bash "$DRAINER"

  [[ $status -eq 0 ]]
  assert_no_post
  assert_pending_alert_count 1
}

@test "T-DRAIN-lock-absent-proceeds: with no lockf available the drain proceeds UNLOCKED (platform fallback)" {
  # On a non-darwin host /usr/bin/lockf is absent and the drain must still run,
  # unlocked, or the Linux path would never drain. Simulate absence via the
  # lockf-binary override so the documented fallback is pinned on any platform.
  local url='http://127.0.0.1:8644/webhooks/osquery-priority' body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  _osquery_store_alert_row 1000 osquery-noflockf "$url" "$body_b64"
  : >"$CURL_LOG"
  set_curl_codes 200
  export OSQUERY_DRAIN_LOCKF_BIN="$HARNESS_HOME/bin/nonexistent-lockf" # not executable -> absent

  run bash "$DRAINER"

  [[ $status -eq 0 ]]
  # The drain PROCEEDED unlocked: the page was delivered and the store cleared.
  grep -qF 'X-Request-ID: osquery-noflockf' "$CURL_LOG"
  assert_pending_alert_count 0
}
