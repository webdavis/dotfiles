#!/usr/bin/env bats
# Uptime watchdog. An unloaded / crash-looping / undelivering pipeline looks identical to "all
# quiet" (edge-triggered alerter, differential queries), so the watchdog is the liveness backstop.
# R2-7: it must NOT report broken delivery or crashed jobs as healthy. It now checks each job's
# LastExitStatus (a repeatedly-nonzero exit alerts), validates an EXPECTED authenticated route
# response (not "anything but 000"), and alerts on a stale or growing page spool.

load ../fixtures/osquery-alerter-lib

setup() { setup_watchdog_harness; }
teardown() {
  unset WATCHDOG_CRASH_AGENTS WATCHDOG_HTTP_CODE WATCHDOG_CRASH_STATUS 2>/dev/null || true
  teardown_harness
}

@test "T-WATCH-all-up: every probe healthy (route exists, no crash, empty spool) → no dispatch" {
  run_watchdog ""
  assert_no_dispatch
}

@test "T-WATCH-notloaded: an unloaded agent pages CRIT, naming it (unchanged)" {
  run_watchdog "com.webdavis.osquery-tailscale-monitor"
  assert_page_has "LaunchAgent not loaded"
  assert_page_has "com.webdavis.osquery-tailscale-monitor"
}

@test "T-WATCH-crashloop: a REGISTERED but crash-looping agent (nonzero LastExitStatus, 2 consecutive) alerts (R2-7)" {
  # The job stays loaded (launchctl list exits 0) but exits nonzero every run. The old watchdog
  # only checked registration, so a crash-loop read as healthy. Two consecutive nonzero checks
  # (30 min) alert; one transient does not.
  export WATCHDOG_CRASH_AGENTS="com.webdavis.osquery-digest"
  run_watchdog ""
  assert_no_dispatch                     # first observation: a transient, not yet a loop
  run_watchdog ""
  assert_page_has "com.webdavis.osquery-digest"
  assert_page_has "crash"
}

@test "T-WATCH-route-405-ok-but-stale-spool: a 405 GET with a STALE spool (a 502 POST that never drained) is NOT healthy (R2-7)" {
  # The live trap: a priority POST 502s (its page spools and sits), while the watchdog's GET
  # returns 405 (route exists). 405 alone is accepted, but the STALE spooled page proves delivery
  # is broken - the exact case the old "405 is fine" check missed.
  export WATCHDOG_HTTP_CODE=405
  seed_spool_file 45   # a page stuck undelivered for 45 min
  run_watchdog ""
  assert_page_has "spool"
}

@test "T-WATCH-route-404: a 404 (priority route not configured) is NOT healthy (R2-7)" {
  # 404 = the route does not exist. The old "any non-000 = up" wrongly accepted it.
  export WATCHDOG_HTTP_CODE=404
  run_watchdog ""
  assert_page_has "route"
}

@test "T-WATCH-route-502: a 5xx route response is NOT healthy (R2-7)" {
  export WATCHDOG_HTTP_CODE=502
  run_watchdog ""
  assert_page_has "route"
}

@test "T-WATCH-route-000: an unreachable gateway (000) still alerts (kept)" {
  export WATCHDOG_HTTP_CODE=000
  run_watchdog ""
  assert_page_has "route"
}

@test "T-WATCH-spool-stale: a stale undelivered page in the spool alerts (R2-7)" {
  seed_spool_file 60
  run_watchdog ""
  assert_page_has "spool"
}

@test "T-WATCH-spool-growing: a spool that GREW since the last check alerts (R2-7)" {
  # Fresh (not stale) pages, but the count rose between two watchdog runs → pages are not
  # draining. Cross-run growth is a delivery-health signal on its own.
  seed_spool_file 0
  run_watchdog ""     # records the baseline count (1)
  seed_spool_file 0
  seed_spool_file 0   # now 3 fresh files
  run_watchdog ""
  assert_page_has "spool"
}

@test "T-WATCH-heartbeat-down: the unloaded heartbeat agent pages CRIT, naming it (kept)" {
  run_watchdog "com.webdavis.osquery-heartbeat"
  assert_page_has "com.webdavis.osquery-heartbeat"
}
