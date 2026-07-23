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

# --- a crash-looping agent pages, but a transient / frozen exit does not ---------

@test "T-WATCH-crashloop-streak-pages: a loaded agent nonzero on two consecutive RE-RUNS pages, naming it" {
  # First observation is a transient (no page); a second failing re-run (runs
  # advanced) is the loop and pages.
  set_agent com.webdavis.osquery-firewall-gatekeeper-monitor 40 1
  run run_watchdog # observation 1: streak 1, not yet a loop
  [[ $status -eq 0 ]] || {
    echo "run1 status $status: $output"
    false
  }
  assert_no_page

  set_agent com.webdavis.osquery-firewall-gatekeeper-monitor 41 1 # it re-ran and failed again
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "run2 status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'com.webdavis.osquery-firewall-gatekeeper-monitor'
  assert_page_body_has 'crash'
}

@test "T-WATCH-crashloop-transient-silent: a single nonzero exit does not page (one bad run is tolerated)" {
  set_agent com.webdavis.osquery-firewall-gatekeeper-monitor 40 1
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_no_page
}

@test "T-WATCH-crashloop-daily-frozen-silent: a DAILY agent's stale nonzero exit (runs frozen between checks) never pages forever" {
  # The digest and heartbeat run once a day, so their launchctl LastExitStatus is
  # FROZEN between the watchdog's 15-min checks. A crash-loop signal must reflect an
  # actual RE-RUN (runs advanced), not the same frozen exit seen every tick, or a
  # single daily failure would page every 15 min for a day. runs stays 7 across both
  # checks, so the streak never reaches the loop threshold.
  set_agent com.webdavis.osquery-digest 7 1
  run run_watchdog # observation 1: streak 1
  assert_no_page
  set_agent com.webdavis.osquery-digest 7 1 # SAME runs: it did not re-run
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_no_page # the frozen exit is not a fresh failure, so it never streaks to a page
}

# --- notify-before-persist ------------------------------------------------------

@test "T-WATCH-notify-before-persist: a page that cannot be durably queued does not advance the state" {
  # A send_alert store-failure must leave the persisted baseline untouched and
  # surface nonzero, so the next tick re-detects instead of masking the signal.
  seed_watchdog_state '{"agents":{}}'
  snapshot_watchdog_state
  unload_agent com.webdavis.osquery-heartbeat # a problem to page
  export WD_SEND_ALERT_EXIT=1                  # dispatch cannot durably queue the page

  run run_watchdog
  [[ $status -ne 0 ]] || {
    echo "expected nonzero when the page could not be queued, got $status: $output"
    false
  }
  assert_page_count 1             # it DID attempt the page
  assert_watchdog_state_unchanged # but the state did NOT advance
}

@test "T-WATCH-persist-on-success: a page-free healthy tick advances the persisted state" {
  seed_watchdog_state '{"agents":{}}'
  set_agent com.webdavis.osquery-digest 7 1
  run run_watchdog # observation 1 advances digest's streak to 1 in the state
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_state_has 'com.webdavis.osquery-digest' # the state advanced
}

# --- the state file is owner-only (0600), atomic, and fresh on corruption -------

@test "T-WATCH-state-0600: the watchdog persists cross-run state owner-only (0600) with no temp left behind" {
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  [[ -f $OSQUERY_WATCHDOG_STATE ]] || {
    echo "expected the state at $OSQUERY_WATCHDOG_STATE, but it is missing"
    false
  }
  assert_mode 600 "$OSQUERY_WATCHDOG_STATE"
  [[ ! -e $OSQUERY_WATCHDOG_STATE.tmp ]] || {
    echo "expected the state temp file to be gone, but $OSQUERY_WATCHDOG_STATE.tmp remains"
    false
  }
}

@test "T-WATCH-state-corrupt-fresh: a corrupt state file is treated as fresh, never a crash, and is repaired" {
  seed_watchdog_state 'not-json-garbage'
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_no_page # a corrupt state is not itself a page; it starts fresh
  # The garbage is replaced by a valid JSON state (a fresh, healthy baseline).
  run jq -e . "$OSQUERY_WATCHDOG_STATE"
  [[ $status -eq 0 ]] || {
    echo "expected the corrupt state to be repaired to valid JSON"
    false
  }
}

# --- injection defeated by validation -------------------------------------------

@test "T-WATCH-injection-inert: a hostile launchctl LastExitStatus is numeric-sanitized and never reaches the body or executes" {
  # An attacker who could influence launchctl output plants a command-substitution
  # payload, a real newline, and forged markdown in the exit-code line. The watchdog
  # extracts ONLY the leading number and validates it, so the raw payload never
  # reaches a rendered variable and never executes. Escaped backticks keep the TEST
  # itself from running it.
  local payload
  payload="1\`touch ${WD_HOME}/PWNED\`"$'\n'"injected **bold** @everyone"
  # Drive the digest to the crash-loop render (streak 2) so the exit value is used.
  seed_watchdog_state '{"agents":{"com.webdavis.osquery-digest":{"runs":7,"streak":1}}}'
  set_agent_raw_exit com.webdavis.osquery-digest 8 "$payload" # runs advanced (7 -> 8): a fresh failing run

  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'com.webdavis.osquery-digest' # it paged the crash-loop
  # No command execution from the payload.
  assert_file_absent "$WD_HOME/PWNED"
  # The raw hostile string never reaches the rendered body (only the number 1 did).
  refute_file_contains '`touch' "$WD_SEND_ALERT_LOG"
  refute_file_contains 'injected **bold**' "$WD_SEND_ALERT_LOG"
}
