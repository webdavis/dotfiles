#!/usr/bin/env bats
# Uptime watchdog: an unloaded pipeline component looks identical to "all quiet"
# (edge-triggered alerter, differential queries), so the watchdog is the liveness
# backstop. No osquery plist sets KeepAlive, so launchd will not reload an unloaded
# agent — the watchdog MUST cover every deployed osquery LaunchAgent, especially the
# page-tier tailscale exposure poller and the digest agent.

load lib

setup() { setup_watchdog_harness; }
teardown() { teardown_harness; }

@test "T-WATCH-all-up: every probe healthy produces no dispatch" {
  run_watchdog ""
  assert_no_dispatch
}

@test "T-WATCH-tailscale-down: the unloaded tailscale exposure poller pages CRIT, naming it" {
  run_watchdog "com.webdavis.osquery-tailscale-monitor"
  assert_page_has "LaunchAgent not loaded"
  assert_page_has "com.webdavis.osquery-tailscale-monitor"
}

@test "T-WATCH-digest-down: the unloaded digest agent pages CRIT, naming it" {
  run_watchdog "com.webdavis.osquery-digest"
  assert_page_has "com.webdavis.osquery-digest"
}
