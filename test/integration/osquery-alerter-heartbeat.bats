#!/usr/bin/env bats
# Daily heartbeat: the positive proof-of-life. Sends ONE silent message to #priority at 09:00 so
# the user can trust silence = safe. R2-8: it must verify the ROOT DAEMON — a standalone osqueryi
# one-shot succeeds even while osqueryd is stopped/wedged, so instead the heartbeat checks that
# the daemon's scheduled heartbeat_canary snapshot is FRESH. Always silent (never pings), honest
# (reports a stale canary rather than a blind checkmark); the uptime watchdog is what PAGES.

load ../fixtures/osquery-alerter-lib

setup() { setup_heartbeat_harness; }
teardown() { teardown_harness; }

@test "T-HB-send: a fresh canary sends exactly one silent healthy message" {
  seed_canary 30   # the daemon wrote the canary 30s ago → alive and scheduling
  run_heartbeat
  [ "$(grep -c $'^CRIT\t' "$SEND_ALERT_LOG")" -eq 1 ]
  grep -qF "healthy" "$SEND_ALERT_LOG"
  [ -z "$(awk -F'\t' '$1=="CRIT"{print $4}' "$SEND_ALERT_LOG")" ]  # silent (empty sound)
}

@test "T-HB-honest-claim: the healthy message claims the DAEMON is alive and points at the watchdog (R2-8)" {
  seed_canary 30
  run_heartbeat
  local body
  body=$(grep $'^CRIT\t' "$SEND_ALERT_LOG" | cut -f3)
  ! grep -qiF "all monitors scheduled" <<<"$body"   # no overclaim
  grep -qiF "watchdog" <<<"$body"                    # points at who owns agent liveness
  grep -qiE "daemon|schedule|canary" <<<"$body"      # it verified the daemon, not a fresh osqueryi
}

@test "T-HB-daemon-stopped: a STALE canary (osqueryd stopped/wedged) reports UNHEALTHY, still silent (R2-8)" {
  # The core R2-8 fix: osqueryd stopped an hour ago, so the newest canary is stale. The old
  # heartbeat launched a fresh osqueryi (which still answers) and reported a blind ✅. Now the
  # stale canary is caught and reported unhealthy.
  seed_canary 3600   # last canary an hour ago → the daemon is not producing scheduled results
  run_heartbeat
  [ "$(grep -c $'^CRIT\t' "$SEND_ALERT_LOG")" -eq 1 ]
  grep -qiE "stale|not producing|not answering" "$SEND_ALERT_LOG"
  ! grep -qF "healthy" "$SEND_ALERT_LOG"
  [ -z "$(awk -F'\t' '$1=="CRIT"{print $4}' "$SEND_ALERT_LOG")" ]  # never pings, even degraded
}

@test "T-HB-no-canary: no canary row at all (fresh deploy / daemon never ran) reports UNHEALTHY (R2-8)" {
  # An absent snapshots.log / no canary row is also not-fresh → unhealthy (the safe direction).
  run_heartbeat
  grep -qiE "stale|not producing|no canary|not answering" "$SEND_ALERT_LOG"
  ! grep -qF "healthy" "$SEND_ALERT_LOG"
}
