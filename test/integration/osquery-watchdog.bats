#!/usr/bin/env bats
# The uptime watchdog (executable_uptime-watchdog.sh) runs as a user LaunchAgent
# every 15 min. A dead pipeline looks identical to "all quiet" (the alerter is
# edge-triggered and the queries are differential), so the watchdog is the sole
# liveness backstop: it verifies osqueryd is answering, every OTHER osquery agent
# is loaded, and the hermes #priority route is reachable, then pages ONE CRIT if
# anything is down.
#
# Cardinal invariant: FAIL-SAFE toward paging. Any ambiguous or failed check (an
# unloaded agent, a wedged osqueryd, an unhealthy route) resolves to a CRIT, never
# a silent all-healthy. Every page is CRIT with a non-empty sound (it must reach
# #priority and ping).

load ../fixtures/osquery-watchdog-lib

setup() { setup_watchdog_harness; }
teardown() { teardown_watchdog_harness; }

# --- a healthy pipeline is silent -----------------------------------------------

@test "T-WATCH-all-healthy: osqueryd answering, all agents loaded, route 405, empty queue -> no page" {
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_no_page
}

# --- an unloaded agent pages CRIT, naming it; the full six-agent set -------------

@test "T-WATCH-agent-not-loaded: an unloaded agent pages one CRIT naming it" {
  unload_agent com.webdavis.osquery-results-alerter
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_sound_nonempty
  assert_page_body_has 'not loaded'
  assert_page_body_has 'com.webdavis.osquery-results-alerter'
}

@test "T-WATCH-watches-new-labels: each agent the basic version did not watch (digest, heartbeat, tailscale, drainer) pages when unloaded" {
  # The re-land expands the watched set from the pre-S9 two agents to the full six.
  local label
  for label in com.webdavis.osquery-alert-drainer \
    com.webdavis.osquery-digest \
    com.webdavis.osquery-heartbeat \
    com.webdavis.osquery-tailscale-monitor; do
    setup_watchdog_harness # a clean, all-healthy baseline per label
    unload_agent "$label"
    run run_watchdog
    [[ $status -eq 0 ]] || {
      echo "status $status for $label: $output"
      false
    }
    assert_page_count 1
    assert_page_body_has "$label"
  done
}

@test "T-WATCH-excludes-self: a full outage names the six watched agents but NOT the watchdog itself" {
  # The watchdog is loaded by definition if it is running, so it must not probe its
  # own label (that would be a guaranteed self-page).
  local label
  for label in "${WD_WATCHED_AGENTS[@]}"; do unload_agent "$label"; done
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  refute_file_contains 'com.webdavis.osquery-uptime-watchdog' "$WD_SEND_ALERT_LOG"
}

# --- osqueryd down and wedged ---------------------------------------------------

@test "T-WATCH-osqueryd-down: osqueryd not running pages CRIT" {
  export WATCHDOG_OSQUERYD_RUNNING=0
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'osqueryd'
}

@test "T-WATCH-osqueryd-wedged: osqueryd running but not answering a one-shot query pages CRIT" {
  export WATCHDOG_OSQUERYI_OK=0
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'wedged'
}

# --- route health (the R2-7 strictening) ----------------------------------------

@test "T-WATCH-route-404-pages: a 404 (priority route not configured) is NOT healthy" {
  export WATCHDOG_HTTP_CODE=404
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'route'
}

@test "T-WATCH-route-502-pages: a 5xx route response is NOT healthy" {
  export WATCHDOG_HTTP_CODE=502
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_body_has 'route'
}

@test "T-WATCH-route-000-pages: an unreachable gateway (000) is NOT healthy" {
  export WATCHDOG_HTTP_CODE=000
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_body_has 'route'
}

@test "T-WATCH-route-405-healthy: a 405 (POST-only route present, rejects GET) is healthy and silent" {
  export WATCHDOG_HTTP_CODE=405
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_no_page
}

@test "T-WATCH-route-probe-unsigned: the route probe is a bare GET carrying NO signing header or secret" {
  # The reachability probe must never put the HMAC key on the wire.
  export WATCHDOG_HTTP_CODE=405
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_curl_probe_unsigned
}
