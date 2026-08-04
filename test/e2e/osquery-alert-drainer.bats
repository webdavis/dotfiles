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
# Reap any banner stub the fd-leak pin left running before the harness HOME is
# removed, so a failed assertion cannot orphan a sleeping process on the machine.
# Guarded on the pid file existing, so every other test in this file is unaffected.
teardown() {
  if [[ -n ${BANNER_PID_FILE:-} && -s ${BANNER_PID_FILE:-} ]]; then
    local banner_pid
    while read -r banner_pid || [[ -n $banner_pid ]]; do
      if [[ -n $banner_pid ]]; then
        kill "$banner_pid" 2>/dev/null || true
      fi
    done <"$BANNER_PID_FILE"
  fi
  teardown_dispatch_harness
}

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

@test "T-DRAIN-lock-fd-leak: a banner child outliving the drain does not keep the lock held" {
  # The lock lives on fd 9, and a fork inherits an open fd. Every child the drain
  # spawns while the lock is held therefore inherits fd 9, and flock releases only
  # once EVERY descriptor on that open file description is closed, so one surviving
  # child keeps the lock after the drainer exits. The acquire is non-blocking, so
  # the damage is a SKIPPED sweep on each later tick until that child dies, not a
  # hang; a skipped sweep still delays every queued page.
  #
  # The child used here is the real one, not a contrived stub: the drain's
  # PIPELINE-DEGRADED path fires a durable banner whose alerter watcher is
  # BACKGROUNDED on purpose and documented as free to outlive its caller (alerter
  # blocks for the banner's whole 60-second life). darwin-only, like the lock.
  [[ -x /usr/bin/lockf ]] || skip "no /usr/bin/lockf; the single-instance lock is a darwin-only guarantee"

  # A max-attempts ceiling of zero puts the seeded row past the dead-letter
  # threshold immediately, so the drain dead-letters it BEFORE any POST and fires
  # the one PIPELINE-DEGRADED banner for the pass. No network stubbing needed.
  export OSQUERY_DRAIN_MAX_ATTEMPTS=0
  local url='http://127.0.0.1:8644/webhooks/osquery-priority' body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  _osquery_store_alert_row 1000 osquery-fd-leak "$url" "$body_b64"

  # An alerter stub shaped like the real binary: it stays alive for the banner's
  # lifetime instead of exiting at once, which is what lets it outlive the drainer.
  # It records its pid so the test can prove it is still running (and teardown can
  # reap it) rather than assume it.
  export BANNER_PID_FILE="$HARNESS_HOME/banner.pid"
  : >"$BANNER_PID_FILE"
  cat >"$HARNESS_HOME/bin/alerter" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$$" >>"$BANNER_PID_FILE"
sleep 30
STUB
  chmod +x "$HARNESS_HOME/bin/alerter"

  # Run the drainer DIRECTLY, not through bats `run`: `run` reaps the command's
  # whole process group, which would kill the very surviving child this pin needs
  # (verified: a backgrounded grandchild is dead after `run` and alive after a
  # direct call).
  local drain_status=0
  bash "$DRAINER" >/dev/null 2>&1 || drain_status=$?
  [[ $drain_status -eq 0 ]] # a drain always exits 0

  # The banner child must genuinely still be running, or this pin proves nothing
  # about fd inheritance and would pass for the wrong reason.
  local banner_pid=""
  local attempt
  # The loop breaks the moment the file appears, so a healthy run still costs
  # milliseconds; the ceiling only decides how long a MISSING banner takes to be
  # reported. Two seconds was not enough of it: this waits on a forked stub under
  # `bats --jobs 4` on a shared runner, and a fork that is merely slow reported
  # itself as a drainer that never fired the banner at all.
  for ((attempt = 0; attempt < 200; attempt++)); do
    [[ -s $BANNER_PID_FILE ]] && break
    sleep 0.05
  done
  read -r banner_pid <"$BANNER_PID_FILE" || true
  if [[ -z $banner_pid ]]; then
    printf 'the drain never fired the degraded-pipeline banner, so no surviving child exists to test\n' >&2
    return 1
  fi
  if ! kill -0 "$banner_pid" 2>/dev/null; then
    printf 'the banner child (pid %s) already exited; the test cannot prove fd release\n' "$banner_pid" >&2
    return 1
  fi

  # The bound: with the drainer gone, the lock is free. A surviving child holding
  # an inherited fd 9 would keep it and this acquisition would fail (75).
  local lock_file="${OSQUERY_UNDELIVERED_ALERTS_DB}.drain.lock"
  local probe_status=0
  (exec 7>>"$lock_file" && /usr/bin/lockf -s -t 0 7) || probe_status=$?
  if [[ $probe_status -ne 0 ]]; then
    printf 'the drain lock is STILL held after the drainer exited (rc %s): a surviving child inherited fd 9\n' \
      "$probe_status" >&2
    return 1
  fi
}

@test "T-DRAIN-stderr-audible: a store failure inside the locked sweep reaches stderr" {
  # The lock is taken with `exec 9>>FILE`, and an `exec` carrying NO COMMAND WORD
  # applies its redirections to the SHELL, permanently. A `2>/dev/null` on that line
  # is therefore not scoped to the open: it silences the whole rest of the script, so
  # every diagnostic the sweep would print afterwards is eaten. That is fail-quiet on
  # a delivery-path component, where the drain's only channel for an unreadable store
  # is exactly that stderr line (the drain always exits 0 by design, so the exit
  # status can never carry the news).
  #
  # A lockf stub makes the "lock is REQUIRED" path run on any platform, so the exec
  # is reached here exactly as it is in production.
  printf '#!/usr/bin/env bash\nexit 0\n' >"$HARNESS_HOME/bin/lockf-stub"
  chmod +x "$HARNESS_HOME/bin/lockf-stub"
  export OSQUERY_DRAIN_LOCKF_BIN="$HARNESS_HOME/bin/lockf-stub"

  # A store that EXISTS but is not a database: the sweep gets past its missing-store
  # guard, sqlite3 then fails, and the library reports that on stderr.
  mkdir -p "$(dirname "$OSQUERY_UNDELIVERED_ALERTS_DB")"
  printf 'this file is not a sqlite database\n' >"$OSQUERY_UNDELIVERED_ALERTS_DB"

  local captured="$HARNESS_HOME/drain-stderr.txt" drain_status=0
  bash "$DRAINER" >/dev/null 2>"$captured" || drain_status=$?
  [[ $drain_status -eq 0 ]] # still a best-effort sweep: the news is on stderr, not in the status

  if [[ ! -s $captured ]]; then
    printf 'the drain said NOTHING on stderr about an unreadable store: the lock exec redirected the whole script to /dev/null\n' >&2
    return 1
  fi
  if ! grep -qiE 'not a database' "$captured"; then
    printf 'the drain wrote to stderr but not the store failure; got: %s\n' "$(cat "$captured")" >&2
    return 1
  fi
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
