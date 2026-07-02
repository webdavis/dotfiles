#!/usr/bin/env bats
# Daily heartbeat: the positive proof-of-life. Sends ONE silent message to #priority at
# 09:00 so the user can trust silence = safe — if the daily ✅ arrives, the pipeline is
# scheduled and alive. The uptime watchdog (every 15 min) is the ALARM that PAGES on
# failure; this is the complementary affirmation, always silent, and honest (it reports
# osqueryd down rather than a blind checkmark).

load lib

setup() { setup_heartbeat_harness; }
teardown() { teardown_harness; }

@test "T-HB-send: a healthy heartbeat sends exactly one silent ✅ message" {
  run_heartbeat
  [ "$(grep -c $'^CRIT\t' "$SEND_ALERT_LOG")" -eq 1 ]  # one #priority message
  grep -qF "pipeline healthy" "$SEND_ALERT_LOG"
  [ -z "$(awk -F'\t' '$1=="CRIT"{print $4}' "$SEND_ALERT_LOG")" ]  # silent (empty sound)
}

@test "T-HB-degraded: if osqueryd is not answering, the heartbeat reports it (still silent)" {
  run_heartbeat 0
  [ "$(grep -c $'^CRIT\t' "$SEND_ALERT_LOG")" -eq 1 ]
  grep -qF "not answering" "$SEND_ALERT_LOG"
  [ -z "$(awk -F'\t' '$1=="CRIT"{print $4}' "$SEND_ALERT_LOG")" ]  # never pings, even degraded
}
