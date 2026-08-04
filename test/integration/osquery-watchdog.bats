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

@test "T-WATCH-all-healthy: fresh canary, all agents loaded, route 405, empty queue -> no page (and no blind osqueryi)" {
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_no_page
  assert_osqueryi_not_called # daemon liveness comes from the scheduled canary, R2-8
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

@test "T-WATCH-osqueryd-wedged-stale-canary: osqueryd running but its scheduled canary is STALE pages CRIT (R2-8, the wedge a one-shot would miss)" {
  # osqueryd is alive (pgrep passes) but not producing scheduled results: its
  # heartbeat canary has gone stale. A standalone osqueryi one-shot would answer and
  # hide this, so the watchdog reads the daemon's OWN scheduled canary instead.
  clear_canary
  seed_canary 4000 # last scheduled result ~67 min ago, well past the freshness bound
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'scheduled results'
  assert_osqueryi_not_called # never a blind one-shot checkmark
}

@test "T-WATCH-osqueryd-canary-missing: no scheduled canary at all pages CRIT (daemon never produced a result)" {
  clear_canary
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'scheduled results'
  assert_osqueryi_not_called
}

@test "T-WATCH-osqueryd-canary-implausible-future: a future-dated canary is NOT trusted as healthy (two-sided freshness)" {
  clear_canary
  seed_future_canary 4000 # ~67 min in the future: clock skew or a tampered row
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
}

@test "T-WATCH-osqueryd-canary-huge-epoch-pages: an over-range canary epoch cannot 64-bit-overflow into a false fresh" {
  # sol's overflow: a timestamp of 2^64 + now wraps in bash's signed 64-bit back to
  # ~now (verified: (( now - (2^64+now) )) == 0), so BOTH freshness bounds read fresh
  # and the watchdog stays SILENT. The seam range-bounds it (>10 digits rejected), so
  # it is treated as no valid canary and pages instead.
  clear_canary
  local overflow
  overflow="$(/usr/bin/bc <<<"$(date -u +%s) + 18446744073709551616")"
  seed_raw_canary "$overflow"
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'scheduled results'
}

@test "T-WATCH-osqueryd-canary-leading-zero-pages: a leading-zero canary epoch cannot break arithmetic into a silent fall-through" {
  # A leading-zero value (09999999999) makes bash arithmetic parse it as octal and
  # error, which a naive elif chain swallows into silence. The seam rejects it, so the
  # watchdog pages instead of falling through.
  clear_canary
  seed_raw_canary '09999999999'
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'scheduled results'
}

@test "T-WATCH-clock-unreadable-pages: a failed system-clock read is a CRIT gap, never a silent healthy" {
  export WATCHDOG_CLOCK_OK=0
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'clock'
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

@test "T-WATCH-agent-never-exited-healthy: a loaded agent that has not exited (running or never run) is healthy, not a gap" {
  # launchctl reports "last exit code = (never exited)" for a process that is
  # currently running or has never run: a legitimate not-a-failure state, not an
  # unreadable one, so it must NOT page.
  set_agent_raw_exit com.webdavis.osquery-tailscale-monitor 3 '(never exited)'
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_no_page
}

@test "T-WATCH-agent-never-exited-with-junk-pages: an exit value CONTAINING '(never exited)' plus junk is UNKNOWN and pages (not a substring free pass)" {
  # A substring match would read 'unknown (never exited) trailing-junk' as the healthy
  # never-exited sentinel and reset the streak. Only an EXACT, whitespace-tolerant
  # sentinel is healthy; anything else is an unknown state that fails safe to a page.
  set_agent_raw_exit com.webdavis.osquery-digest 5 'unknown (never exited) trailing-junk'
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'unreadable'
  assert_page_body_has 'com.webdavis.osquery-digest'
}

@test "T-WATCH-agent-exit-garbage-pages: a loaded agent whose exit field is unparseable garbage pages a fail-safe gap (never silent-healthy)" {
  # If the last-exit-code value is neither a number nor the never-exited sentinel,
  # the agent state is UNKNOWN. The watchdog must fail safe to a page, not default
  # the exit code to 0 and read every agent as healthy (the fail-open trap).
  set_agent_raw_exit com.webdavis.osquery-digest 5 'wat-not-a-code'
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'unreadable'
  assert_page_body_has 'com.webdavis.osquery-digest'
}

@test "T-WATCH-agent-exit-field-absent-pages: a loaded agent whose launchctl output lacks the exit field pages a fail-safe gap" {
  # A launchctl output-shape change that drops the last-exit-code field would, under
  # a default-to-0, silently disable crash-loop detection for every agent. Instead
  # the absent field is an unknown state that pages.
  set_agent_no_exit_field com.webdavis.osquery-heartbeat 5
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'unreadable'
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

@test "T-WATCH-state-concatenated-starts-fresh: a state file holding two concatenated documents is not trusted, so a confirmed tamper still pages" {
  # A corruption gate of the shape `jq -e .` takes its exit status from the LAST
  # document of a concatenated stream, so two valid documents back to back read
  # as healthy. Each later read then runs against the stream: a `// -1` fallback
  # emits one line PER document (which every numeric guard rejects), but a `// ""`
  # fallback emits an EMPTY line for a document that lacks the key, and command
  # substitution strips it - so a trailing {} collapses the fan-out back to one
  # clean value that passes its guard. The paged-fingerprint marker is read that
  # way, and a marker resurrected from a corrupt state silences the manifest-audit
  # page for that exact tamper forever (a fingerprint already paged for never
  # pages again). A whole-file "exactly one object" read is what refuses it.
  local fingerprint corrupt_document
  tamper_manifested_file
  run run_watchdog # tick 1: the divergence is seen once (confirming), silent
  [[ $status -eq 0 ]] || {
    echo "tick1 status $status: $output"
    false
  }
  assert_no_page
  fingerprint="$(jq -r '.pipeline_audit.fingerprint' "$OSQUERY_WATCHDOG_STATE")"

  # A state claiming this exact divergence was ALREADY paged for, as two valid
  # concatenated documents. The second document is what collapses the fan-out.
  corrupt_document="$(jq -cn --arg fp "$fingerprint" \
    '{agents: {}, pending: {count: -1, growth_streak: 0},
      pipeline_audit: {fingerprint: $fp, streak: 1, paged_fingerprint: $fp}}')"
  seed_watchdog_state "$corrupt_document
{}"

  run run_watchdog # tick 2: the corrupt state must start fresh, not adopt its marker
  [[ $status -eq 0 ]] || {
    echo "tick2 status $status: $output"
    false
  }
  assert_no_page
  assert_state_has '"paged_fingerprint":""'

  run run_watchdog # tick 3: confirmed against a fresh baseline, so it pages
  [[ $status -eq 0 ]] || {
    echo "tick3 status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'divergence'
}

@test "T-WATCH-state-pretty-printed-round-trips: a valid single-document state is trusted whatever its whitespace" {
  # The corruption gate counts DOCUMENTS, not lines: a hand-inspected (indented,
  # multi-line) state file is still exactly one document and must keep its
  # streak memory, or the crash-loop alarm would silently reset every tick.
  set_agent com.webdavis.osquery-digest 40 1 # observation 1: streak 1
  run run_watchdog
  assert_no_page
  jq . "$OSQUERY_WATCHDOG_STATE" >"$WD_HOME/pretty-state.json"
  cp "$WD_HOME/pretty-state.json" "$OSQUERY_WATCHDOG_STATE"

  set_agent com.webdavis.osquery-digest 41 1 # it re-ran and failed again
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_body_has 'crash'
}

@test "T-WATCH-state-unpersistable-pages: an unwritable state dir pages CRIT (the streak alarms would otherwise silently degrade)" {
  # With the state unwritable, prev_state resets to {} every tick, so a crash-looping
  # agent's streak resets to 1 each run and never reaches the loop threshold: the
  # alarm is silently disabled. So an unpersistable state is itself a paging condition,
  # and the run surfaces nonzero.
  local state_dir
  state_dir="$(dirname "$OSQUERY_WATCHDOG_STATE")"
  chmod 500 "$state_dir"
  run run_watchdog
  chmod 700 "$state_dir" # restore before asserting / teardown
  [[ $status -ne 0 ]] || {
    echo "expected nonzero when the state cannot be persisted, got $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'persist'
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

# --- delivery-backlog health: dead-letters, unreadable counts, sustained growth --

@test "T-WATCH-deadletter-pages: any dead-letter entry pages CRIT (delivery permanently failed)" {
  seed_dead_letter_alerts 2 # two real rows in the real store the counter reads
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_sound_nonempty
  assert_page_body_has 'dead-letter'
}

@test "T-WATCH-count-unreadable-pages: a corrupt alert store is a CRIT gap, never a silent healthy" {
  # Drive the REAL counter's failure path: an on-disk corruption of the store makes
  # the read fail, so the counter returns the present-but-unreadable signal and the
  # watchdog fail-safe pages. A store hiding real dead-letters must never read healthy.
  seed_corrupt_alert_db
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'unreadable'
}

@test "T-WATCH-backlog-growing-pages: a backlog that grows across two consecutive checks pages CRIT" {
  # Seed a prior growth (count 5, growth_streak 1); this tick grows again to 8 real
  # rows, so the streak reaches the sustained-growth threshold and pages.
  seed_watchdog_state '{"agents":{},"pending":{"count":5,"growth_streak":1}}'
  seed_pending_alerts 8
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'backlog'
}

@test "T-WATCH-backlog-steady-silent: a non-growing backlog (even a large one) does not page" {
  # A prior growth streak, but this tick did NOT grow (count flat at 5 real rows): a
  # transient burst the drainer absorbs must not false-page. Only SUSTAINED growth pages.
  seed_watchdog_state '{"agents":{},"pending":{"count":5,"growth_streak":1}}'
  seed_pending_alerts 5
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_no_page
}

# --- the periodic manifest audit: tamper that generates NO file event -----------
#
# osquery watches PATHS. An attacker who hard-links a manifested pipeline script to
# a writable path outside the pipeline home and overwrites that alias mutates the
# SAME INODE: the filesystem event names the attacker's path, nothing fires for the
# watched one, no verdict runs, and the tampered script executes with nothing paged.
# The audit closes that by comparing bytes on a schedule, so it never depends on an
# event having fired. Every case below tampers IN PLACE with no event involved.
#
# A divergence must persist across two consecutive ticks before it pages. A
# legitimate `chezmoi apply` writes the deployed files and then regenerates the
# manifest, so a tick landing inside that window sees a divergence that is gone by
# the next one; requiring the SAME divergence twice spends 15 minutes of detection
# delay on a backstop (the event path already pages instantly for tamper it can see)
# and buys immunity from false-paging every apply.

@test "T-WATCH-audit-tamper-pages: a manifested file whose content diverges pages CRIT, with no file event anywhere" {
  tamper_manifested_file
  run run_watchdog # tick 1: first observation, a transient is tolerated
  [[ $status -eq 0 ]] || {
    echo "tick1 status $status: $output"
    false
  }
  assert_no_page

  run run_watchdog # tick 2: the same divergence is still there
  [[ $status -eq 0 ]] || {
    echo "tick2 status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_sound_nonempty
  assert_page_body_has 'known-good manifest'
  assert_page_body_has 'content changed'
  refute_file_contains 'permissions changed' "$WD_SEND_ALERT_LOG"
}

@test "T-WATCH-audit-tamper-body-inert: the diverging PATH never reaches the page body" {
  # The manifest is attacker-influenceable in the shape this audit exists to catch,
  # so its paths are counted, never rendered. The body carries a validated count and
  # static text only, exactly like every other probe in this watchdog.
  tamper_manifested_file
  run run_watchdog
  run run_watchdog
  assert_page_count 1
  refute_file_contains "$WD_MANIFESTED_SCRIPT" "$WD_SEND_ALERT_LOG"
}

@test "T-WATCH-audit-apply-race-silent: a divergence that resolves before the next tick never pages" {
  # The in-flight apply: the deployed file has changed, the manifest has not been
  # regenerated yet. One tick sees it, the next sees a regenerated manifest. A
  # legitimate apply must not page.
  tamper_manifested_file
  run run_watchdog # tick 1 lands inside the window
  [[ $status -eq 0 ]] || {
    echo "tick1 status $status: $output"
    false
  }
  assert_no_page

  regenerate_pipeline_manifest # the apply finishes: the manifest now covers the new bytes
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "tick2 status $status: $output"
    false
  }
  assert_no_page
}

@test "T-WATCH-audit-symlink-pages: a symlink at a manifested path pages even when its referent matches" {
  # The second shape of the same blind spot: the bytes the manifest vouches for are
  # still readable THROUGH the link, but the file that executes now lives outside
  # the watched tree. An audit that followed links would call this clean.
  symlink_manifested_file
  run run_watchdog
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "tick2 status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'known-good manifest'
}

@test "T-WATCH-audit-attr-hardlink-pages: a chmod made through a hard link outside the pipeline home pages, with no file event and no content change" {
  # The attribute half of the same blind spot, and the one neither layer used to
  # catch. The alias and the manifested script are one inode, so the watched path
  # is now group-writable; the change was made to the attacker's path, so nothing
  # fires for the watched one and the event layer never judges it; and no byte of
  # content moved, so a content-only audit called it clean.
  chmod_manifested_file_through_hard_link
  run run_watchdog # tick 1: first observation, a transient is tolerated
  [[ $status -eq 0 ]] || {
    echo "tick1 status $status: $output"
    false
  }
  assert_no_page

  run run_watchdog # tick 2: the same divergence is still there
  [[ $status -eq 0 ]] || {
    echo "tick2 status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_sound_nonempty
  assert_page_body_has 'known-good manifest'
  # The KIND is named, from the watchdog's own closed vocabulary, because the
  # operator's next move differs: this is the step before a rewrite, and it is
  # reversible. A body that said only "no longer matches" would read the same for a
  # permission change and for a script already executing attacker bytes.
  assert_page_body_has 'permissions changed'
  refute_file_contains 'content changed' "$WD_SEND_ALERT_LOG"
  # The path stays out of the body for an attribute divergence too: same rule, and
  # the manifest is just as attacker-influenceable in this shape as in the others.
  refute_file_contains "$WD_MANIFESTED_SCRIPT" "$WD_SEND_ALERT_LOG"
}

@test "T-WATCH-audit-attr-then-content-repages: a file already reported for its MODE pages again when its content is tampered too" {
  # The escalation case. Page-once dedupes on a fingerprint of the report, so the
  # second, more serious drift is suppressed unless the report itself changes. It
  # changes only because mode and content are DISTINCT kinds, each on its own line:
  # one generic per-path divergence line would be byte-identical before and after,
  # and the content tamper would page nothing.
  chmod_manifested_file_through_hard_link
  run run_watchdog # tick 1: confirming
  run run_watchdog # tick 2: pages for the mode drift
  assert_page_count 1

  tamper_manifested_file # the SAME file, now rewritten as well
  run run_watchdog       # the divergence set changed: confirming again
  assert_page_count 1
  run run_watchdog # confirmed: a second page for the escalation
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 2
}

@test "T-WATCH-audit-manifest-missing-pages: an absent manifest pages instead of reading as all-clear" {
  # A monitor must not go quiet because its own input broke: with no known-good list
  # there is nothing to compare against, so tampering would pass unseen.
  remove_pipeline_manifest
  run run_watchdog
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "tick2 status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_sound_nonempty
  assert_page_body_has 'known-good manifest'
}

@test "T-WATCH-audit-helper-missing-pages: the audit's own dependency going missing pages, instead of killing the tick silently" {
  # The audit reuses the verdict helper for the manifest constant and the ownership
  # check. Under the watchdog's errexit shell, sourcing a deleted helper would abort
  # the whole tick: no probe would report, and nothing would page. A monitor that
  # dies quietly when part of it is removed is worse than no monitor, so the missing
  # dependency has to come out as a page.
  rm -f "$WD_HOME/.local/libexec/osquery/results-alerter/pipeline-verdict.sh"
  run run_watchdog
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "tick2 status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_sound_nonempty
  assert_page_body_has 'not installed'
}

@test "T-WATCH-audit-seam-missing-pages: the audit seam itself going missing pages, instead of killing the tick silently" {
  # One level up from the case above, and the same rule: the watchdog sources the
  # audit seam, so a deleted seam would abort the tick before ANY probe reported.
  # Probes 1 to 4 do not depend on it, so the tick must survive and say so.
  rm -f "$WD_HOME/.local/libexec/osquery/pipeline-audit.sh"
  run run_watchdog
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "tick2 status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_sound_nonempty
  assert_page_body_has 'not installed'
}

@test "T-WATCH-audit-page-once: a persistent divergence pages once, not every 15 minutes forever" {
  tamper_manifested_file
  run run_watchdog # tick 1: confirming
  run run_watchdog # tick 2: pages
  assert_page_count 1
  run run_watchdog # tick 3: the SAME divergence, already reported
  [[ $status -eq 0 ]] || {
    echo "tick3 status $status: $output"
    false
  }
  assert_page_count 1
  run run_watchdog # tick 4: still the same
  assert_page_count 1
}

@test "T-WATCH-audit-repages-on-change: a divergence that CHANGES after being reported pages again" {
  # Page-once must not become page-never: a second file going bad after the first
  # was reported is new information, so it pages on its own confirmation.
  tamper_manifested_file
  run run_watchdog
  run run_watchdog
  assert_page_count 1

  tamper_second_manifested_file
  run run_watchdog # the divergence set changed: confirming
  assert_page_count 1
  run run_watchdog # confirmed: a second page
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 2
}

@test "T-WATCH-audit-clean-silent: an untampered manifested tree is silent across many ticks" {
  # The audit must not itself become a source of noise: nothing diverges, so no
  # amount of ticking produces a page.
  run run_watchdog
  run run_watchdog
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_no_page
}

# --- the same audit, over the managed ~/.local/bin scripts ----------------------
#
# update-skills.sh, homebrew-weekly-upgrade.sh and the claude-* hooks run unattended
# from LaunchAgents and shell hooks. The event path can miss a hard-linked or
# relocated tamper there for exactly the reasons it can in the pipeline home, and
# there is nobody at the keyboard when those fire, so the schedule-driven audit is
# the backstop that matters most for them. All three bound columns are compared, so
# an attribute-only change reports under its own kind rather than passing as clean.

@test "T-WATCH-audit-managed-bin-tamper-pages: a tampered managed ~/.local/bin script pages CRIT, with no file event anywhere" {
  tamper_managed_bin_file
  run run_watchdog # tick 1: first observation, a transient is tolerated
  [[ $status -eq 0 ]] || {
    echo "tick1 status $status: $output"
    false
  }
  assert_no_page

  run run_watchdog # tick 2: the same divergence is still there
  [[ $status -eq 0 ]] || {
    echo "tick2 status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'known-good manifest'
  assert_page_body_has 'content changed'
}

@test "T-WATCH-audit-managed-bin-attr-hardlink-pages: a chmod made through a hard link outside ~/.local/bin pages as a PERMISSION divergence, with no file event and no content change" {
  # The attribute half of the blind spot, on the bin arm. The alias and the managed
  # script are ONE INODE, so the watched path's mode moves while the event names the
  # attacker's path, and not a byte of content changes. A content-only comparison
  # would call this clean; the mode column is what does not.
  chmod_managed_bin_file_through_hard_link
  run run_watchdog
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'permissions changed'
  # ...and it is NOT reported as a content change: no byte moved, and conflating the
  # two would tell the operator the script already executes attacker bytes.
  refute_file_contains 'content changed' "$WD_SEND_ALERT_LOG"
}

@test "T-WATCH-audit-managed-bin-shim-silent: a third-party shim updating itself in ~/.local/bin never pages" {
  # The churn case the manifest-driven coverage exists to avoid. mise, herdr, bob
  # and yt-dlp rewrite themselves on their own schedule; no manifest lists them, so
  # the audit has nothing to compare and must stay quiet. If this paged, the whole
  # probe would become noise the operator learns to ignore.
  update_unmanaged_bin_shim
  run run_watchdog
  run run_watchdog
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_no_page
}

@test "T-WATCH-audit-managed-bin-manifest-missing-pages: losing the managed-bin manifest pages, even while the pipeline manifest is fine" {
  # Half an audit reporting "clean" is a lie about the half it never read, so a
  # refusal on either manifest refuses the tick.
  remove_managed_bin_manifest
  run run_watchdog
  run run_watchdog
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'known-good manifest'
}
