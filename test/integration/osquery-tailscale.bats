#!/usr/bin/env bats
# The public-exposure monitor (executable_tailscale-monitor.sh) runs as a user
# LaunchAgent every 60s. It reads `tailscale funnel status --json`, classifies
# whether a Funnel exposes a local service to the PUBLIC internet (an AllowFunnel
# entry set true, distinct from a tailnet-only serve), compares against the prior
# baseline, and pages CRIT only on an off->on transition, a first-observation
# active funnel, or a monitoring gap. It is silent in steady state and when a
# funnel is closed. Every page precedes the state advance (notify-before-persist).
#
# R2-5: a MONITORING GAP of a public-exposure detector is itself CRIT. A missing
# binary, a failed status command, empty output, or malformed JSON is NOT swallowed
# into a false "inactive"; it pages once, preserves the prior valid baseline, and
# never advances state on a blind read. The funnel status is network-influenceable,
# so the exposure page renders it as inert data (sanitized + inline-code-wrapped),
# and gap pages render only the local binary path, a numeric rc, and static text.

load ../fixtures/osquery-tailscale-lib

setup() { setup_tailscale_harness; }
teardown() { teardown_tailscale_harness; }

# A realistic active-funnel ServeConfig: AllowFunnel true for one SNI:port, plus
# the Web handler that proxies it. Shape verified against tailscale v1.98.8
# ipn/serve.go (AllowFunnel map[HostPort]bool).
FUNNEL_ON='{"AllowFunnel":{"dresden.tailnet.ts.net:443":true},"Web":{"dresden.tailnet.ts.net:443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:8000"}}}}}'
# A tailnet-only serve: Web/TCP populated, but NO AllowFunnel (not public).
SERVE_ONLY='{"Web":{"dresden.tailnet.ts.net:443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:8000"}}}}}'

# --- B1: binary resolution ------------------------------------------------------

@test "T-TS-resolve-path: with no env override the monitor finds tailscale via PATH" {
  seed_funnel_state inactive
  set_funnel "$FUNNEL_ON"
  run run_tailscale_monitor_path_resolved
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1 # resolved the stub on PATH and paged the funnel
  assert_tailscale_called_with '--json'
}

# --- B2/B3: an idle funnel is silent; first run seeds inactive silently ----------

@test "T-TS-idle-silent: an idle funnel status with an inactive baseline pages nothing" {
  seed_funnel_state inactive
  set_funnel '{}'
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_no_page
  assert_baseline_funnel inactive
  assert_tailscale_called_with 'funnel status --json' # read via the verified JSON path
}

@test "T-TS-firstrun-idle-seeds-silent: a first run with no baseline and an idle funnel seeds inactive silently" {
  seed_funnel_state "" # no prior baseline
  set_funnel '{}'
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_no_page
  assert_baseline_funnel inactive # seeded, so the next tick is quiet
}

# --- B4: an off->on transition pages CRIT (public exposure) ----------------------

@test "T-TS-funnel-on-pages: a funnel turning on pages one CRIT naming the public exposure" {
  seed_funnel_state inactive
  set_funnel "$FUNNEL_ON"
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT   # only a CRIT reaches the #priority webhook
  assert_page_sound_nonempty     # the page tier, never muted
  assert_page_body_has 'Funnel'
  assert_page_body_has 'PUBLIC'
  # Notify-before-persist: at page time the baseline had NOT advanced to active.
  assert_page_saw_prior_not_active
  # Once the page is durably queued, the baseline advances to active.
  assert_baseline_funnel active
}

# --- B5: an already-active funnel is silent (steady state) -----------------------

@test "T-TS-funnel-steady-silent: an already-active funnel does not re-page" {
  seed_funnel_state active
  set_funnel "$FUNNEL_ON"
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_no_page
  assert_baseline_funnel active
}

# --- B6: a first-observation active funnel (no baseline) pages -------------------

@test "T-TS-firstrun-active-pages: a first run with the funnel already active pages CRIT" {
  seed_funnel_state "" # no prior baseline
  set_funnel "$FUNNEL_ON"
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_sound_nonempty
  assert_page_body_has 'Funnel'
  assert_baseline_funnel active # seeded only after the page succeeds
}

# --- on->off: closing a funnel removes the exposure and is SILENT ----------------

@test "T-TS-funnel-off-silent: a funnel turning OFF (closed) is silent and updates the baseline to inactive" {
  seed_funnel_state active
  set_funnel '{}' # the funnel was reset/closed
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_no_page # closing an exposure is good news, not actionable
  assert_baseline_funnel inactive
}

# --- B22: a tailnet-only serve is NOT a public exposure and does not page --------

@test "T-TS-serve-only-silent: a tailnet-only serve (Web set, no AllowFunnel) does not page" {
  # DIVERGENCE from c69baab: its coarse text-grep pages on ANY non-empty status,
  # so a private `tailscale serve` would false-page as a public exposure. Reading
  # AllowFunnel from --json pages only a real public funnel.
  seed_funnel_state inactive
  set_funnel "$SERVE_ONLY"
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_no_page
  assert_baseline_funnel inactive
}

@test "T-TS-allowfunnel-false-silent: an AllowFunnel entry set FALSE is not an active funnel" {
  seed_funnel_state inactive
  set_funnel '{"AllowFunnel":{"dresden.tailnet.ts.net:443":false}}'
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_no_page
  assert_baseline_funnel inactive
}

@test "T-TS-allowfunnel-nonboolean-gaps: a non-boolean AllowFunnel value is an unexpected shape and pages a gap, not a silent inactive" {
  # AllowFunnel is map[HostPort]bool: tailscale always serializes booleans. A
  # non-boolean value (a serialization change or a tampered read) is an
  # unclassifiable funnel state, so fail-safe to a CRIT gap, never a silent
  # inactive that could miss a real public exposure (R2-5).
  seed_funnel_state inactive
  set_funnel '{"AllowFunnel":{"dresden.tailnet.ts.net:443":"true"}}'
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'BLIND'
  assert_gap_marker
  assert_baseline_funnel inactive # a gap never advances the baseline
}

@test "T-TS-foreground-funnel-pages: a funnel in a Foreground session is detected and pages" {
  # `tailscale funnel <port>` (no --bg) nests the config under Foreground.<session>,
  # each a ServeConfig that itself carries AllowFunnel. A funnel there is still a
  # public exposure, so the recursive classifier must catch it.
  seed_funnel_state inactive
  set_funnel '{"Foreground":{"sess-abc":{"AllowFunnel":{"dresden.tailnet.ts.net:443":true}}}}'
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
}

# --- B7: notify-before-persist; a store failure never loses the page -------------

@test "T-TS-page-failure-keeps-baseline: a send_alert failure does not advance the baseline, so the next tick re-detects and re-pages" {
  seed_funnel_state inactive
  snapshot_baseline
  set_funnel "$FUNNEL_ON"
  export TS_SEND_ALERT_EXIT=1 # dispatch cannot durably queue the page

  run run_tailscale_monitor
  [[ $status -ne 0 ]] || {
    echo "expected the monitor to surface the send_alert failure (nonzero), got $status: $output"
    false
  }
  assert_page_count 1       # it DID attempt the page
  assert_baseline_unchanged # but the baseline did NOT advance to active

  # Next tick: dispatch now succeeds. The still-inactive baseline re-detects the
  # transition and re-pages (at-least-once), then advances. Nothing was lost.
  export TS_SEND_ALERT_EXIT=0
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "retry status $status: $output"
    false
  }
  assert_page_count 2 # re-detected and re-paged, never silently lost
  assert_baseline_funnel active
}

@test "T-TS-firstrun-active-page-failure-no-seed: a first-observation page whose send_alert fails seeds no baseline, exits nonzero, and re-pages next tick" {
  seed_funnel_state "" # no prior baseline
  set_funnel "$FUNNEL_ON"
  export TS_SEND_ALERT_EXIT=1

  run run_tailscale_monitor
  [[ $status -ne 0 ]] || {
    echo "expected nonzero when the first-observation page could not be queued, got $status: $output"
    false
  }
  assert_no_state     # no baseline seeded on failure, so the exposure is re-detected
  assert_page_count 1 # it attempted the page

  export TS_SEND_ALERT_EXIT=0
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "retry status $status: $output"
    false
  }
  assert_page_count 2            # re-detected the still-unbaselined exposure and re-paged
  assert_baseline_funnel active  # now seeded
}

# --- B8: the baseline is owner-only (0600) and atomic ---------------------------

@test "T-TS-persist-0600: the monitor persists the baseline owner-only (0600) with no temp left behind" {
  seed_funnel_state inactive
  set_funnel "$FUNNEL_ON"
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  [[ -f $OSQUERY_TAILSCALE_STATE ]] || {
    echo "expected the baseline at $OSQUERY_TAILSCALE_STATE, but it is missing"
    false
  }
  assert_mode 600 "$OSQUERY_TAILSCALE_STATE"
  [[ ! -e $OSQUERY_TAILSCALE_STATE.tmp ]] || {
    echo "expected the write_state temp file to be gone, but $OSQUERY_TAILSCALE_STATE.tmp remains"
    false
  }
}

# --- B9: a missing binary is a CRIT gap, page-once (R2-5) ------------------------

@test "T-TS-missing-bin-pages: a missing tailscale binary pages a CRIT BLIND gap (blindness reaches the remote)" {
  # The dead-monitor regression: a GUI-path default silently disabled funnel paging
  # on the headless-formula install. Now it is a CRIT page (was a WARN the dispatcher dropped).
  seed_funnel_state inactive
  run run_tailscale_monitor_missing_bin
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_sound_nonempty
  assert_page_body_has 'BLIND'
  assert_gap_marker
}

@test "T-TS-missing-bin-once: the missing-binary gap pages once, not every 60s (R2-5)" {
  seed_funnel_state gap # a prior blind window: the gap marker is already set
  run run_tailscale_monitor_missing_bin
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_no_page # already blind: the marker suppresses a re-page
}

# --- B10/B11: a failing status command is a CRIT gap and preserves the baseline --

@test "T-TS-status-fail-pages-gap: a failing funnel status (rc=1) is a CRIT gap, not a silent inactive (R2-5)" {
  seed_funnel_state active
  set_funnel '' # no output
  set_funnel_rc 1
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'BLIND'
  assert_gap_marker
}

@test "T-TS-status-hang-pages-gap: a funnel status that outlasts the bound is killed and pages a gap (R2-5)" {
  # A wedged tailscaled (the CLI blocks on the local API socket) must become a
  # monitoring gap, not silent blindness: without a bound, launchd skips ticks
  # while the process lives and the monitor never pages. The bound kills the read
  # and the gap gate pages it.
  seed_funnel_state active
  snapshot_baseline
  export OSQUERY_TAILSCALE_TIMEOUT=1 # bound the read at 1s
  export TAILSCALE_FUNNEL_SLEEP=30   # tailscaled wedges far past the bound
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'BLIND'
  assert_gap_marker
  assert_baseline_unchanged # a wedged read is a gap: it never persists
}

@test "T-TS-status-fail-preserves-baseline: a status failure preserves the prior valid funnel baseline (R2-5)" {
  # rc=1 must not overwrite a known-active baseline with a false "inactive": the
  # prior state is preserved so a real transition is still detectable on recovery.
  seed_funnel_state active
  snapshot_baseline
  set_funnel ''
  set_funnel_rc 1
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_baseline_funnel active
  assert_baseline_unchanged
}

# --- B12: empty output and malformed JSON are CRIT gaps --------------------------

@test "T-TS-empty-output-pages-gap: an empty (rc=0) status output is a CRIT gap, not a silent inactive (R2-5)" {
  seed_funnel_state active
  set_funnel '' # succeeds but prints nothing
  set_funnel_rc 0
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'BLIND'
  assert_gap_marker
  assert_baseline_funnel active # a gap never overwrites the baseline
}

@test "T-TS-malformed-json-pages-gap: a malformed (non-JSON) status output is a CRIT gap, not a silent inactive" {
  seed_funnel_state active
  set_funnel '{not valid json'
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'BLIND'
  assert_gap_marker
  assert_baseline_funnel active
}

# --- B13: a corrupt prior state is NOT a baseline -------------------------------

@test "T-TS-corrupt-state-active-pages: a corrupt prior state is not a baseline, so an active funnel pages (R2-5)" {
  # A garbage state file must not be trusted as an "active" baseline that would
  # suppress the page.
  seed_funnel_state corrupt
  set_funnel "$FUNNEL_ON"
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'Funnel'
  assert_baseline_funnel active
}

# --- B14: a funnel found active on recovery from a blind window pages ------------

@test "T-TS-funnel-after-blind-pages: a funnel found active on recovery from a blind window pages" {
  seed_funnel_state gap # prior blind window (inactive baseline + gap marker)
  set_funnel "$FUNNEL_ON"
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'Funnel'
  assert_no_gap_marker # a valid read cleared the blind-window marker
  assert_baseline_funnel active
}

# --- B15: a valid read after a gap clears the marker, so a later gap re-pages -----

@test "T-TS-gap-recovery-clears-marker: a valid read after a gap clears the marker, so a later gap pages again" {
  seed_funnel_state inactive

  set_funnel '' # gap 1: empty output pages and writes the marker
  set_funnel_rc 0
  run run_tailscale_monitor
  assert_page_count 1
  assert_gap_marker

  set_funnel '{}' # recovery: a valid idle read clears the marker, steady inactive (no page)
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "recovery status $status: $output"
    false
  }
  assert_no_gap_marker

  set_funnel '' # gap 2: the marker was cleared, so it pages again
  run run_tailscale_monitor
  assert_page_count 2 # the second gap is not suppressed by a stale marker
  assert_gap_marker
}

@test "T-TS-gap-page-failure-no-marker: a gap whose send_alert fails writes no marker, exits nonzero, and re-pages next tick" {
  seed_funnel_state inactive
  set_funnel ''
  export TS_SEND_ALERT_EXIT=1 # dispatch cannot queue the gap page

  run run_tailscale_monitor
  [[ $status -ne 0 ]] || {
    echo "expected nonzero when the gap page could not be queued, got $status: $output"
    false
  }
  assert_no_gap_marker # no marker on failure, so the next tick retries
  assert_page_count 1  # it attempted the page

  export TS_SEND_ALERT_EXIT=0
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "retry status $status: $output"
    false
  }
  assert_page_count 2 # re-detected the still-unmarked gap and re-paged (at-least-once)
  assert_gap_marker
}

# --- B16: the funnel status is network-influenceable and rendered INERT ----------

@test "T-TS-injection-inert: a hostile funnel host:port is sanitized, cannot break the code span, and never executes" {
  seed_funnel_state inactive
  # A hostile AllowFunnel key: a backtick + command-substitution payload, a real
  # newline, and forged Discord markdown. An attacker who opened the funnel controls
  # this SNI:port string. Escaped backticks keep the TEST itself from executing it.
  local payload
  payload="evil\`touch ${TS_HOME}/PWNED\`"$'\n'"Injected **bold** @everyone:443"
  set_funnel "$(jq -cn --arg k "$payload" '{AllowFunnel: {($k): true}}')"

  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  # No command execution from the payload (the body is passed as data, never eval'd).
  assert_file_absent "$TS_HOME/PWNED"
  # Backticks are stripped: the raw `touch sequence cannot survive to break out of
  # the inline-code span. (The legitimate wrapping backticks remain, so we refute
  # the payload's specific backtick-touch, not all backticks.)
  refute_file_contains '`touch' "$TS_SEND_ALERT_LOG"
  # The newline is squashed to a space: the payload cannot inject a standalone
  # markdown line, so PWNED and Injected land on ONE line, not two.
  if ! grep -F 'PWNED' "$TS_SEND_ALERT_LOG" | grep -qF 'Injected'; then
    echo "expected the newline-squashed payload on one line; send_alert log:"
    cat "$TS_SEND_ALERT_LOG"
    false
  fi
}

@test "T-TS-gap-body-no-raw-output: a malformed status does not leak raw CLI output into the gap page" {
  seed_funnel_state inactive
  set_funnel "GARBAGE\`touch ${TS_HOME}/PWNED\`{not json"
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'BLIND'
  # A gap page renders only static text + the local path + a numeric rc: the raw
  # (attacker-influenceable) output must never reach it.
  refute_file_contains 'GARBAGE' "$TS_SEND_ALERT_LOG"
  refute_file_contains '`touch' "$TS_SEND_ALERT_LOG"
  assert_file_absent "$TS_HOME/PWNED"
}

# --- B17: every dispatch is the page tier (CRIT + non-empty sound), never muted --

@test "T-TS-tier-page-not-muted: a funnel-on page is CRIT with a non-empty sound (page tier)" {
  # GATE: an empty sound threads tier=muted into the webhook and skips the phone
  # ping. A public-exposure page must ping. CRIT and a non-empty sound are BOTH
  # load-bearing (mutation-verified: CRIT->INFO and sound->"" each red a test).
  seed_funnel_state inactive
  set_funnel "$FUNNEL_ON"
  run run_tailscale_monitor
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_severity_is CRIT
  assert_page_sound_nonempty
}

@test "T-TS-gap-tier-page-not-muted: a monitoring-gap page is also CRIT with a non-empty sound" {
  seed_funnel_state inactive
  run run_tailscale_monitor_missing_bin
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_severity_is CRIT
  assert_page_sound_nonempty
}

# --- B23: a baseline-write failure is a loud degraded-monitor gap ----------------

@test "T-TS-persist-failure-pages-degraded: a baseline-write failure pages a degraded-monitor gap, never silently trusts a stale baseline" {
  # A funnel closing (active->inactive) is a SILENT path that still must persist the
  # new baseline. Make the state dir unwritable so write_state fails on this tick.
  # Without a loud failure, the stale prev=active would mask a later re-exposure
  # (cur=active vs stale active reads as steady, silent, forever).
  seed_funnel_state active
  set_funnel '{}' # funnel closed: a silent path that must persist inactive
  chmod 500 "$(dirname "$OSQUERY_TAILSCALE_STATE")"
  run run_tailscale_monitor
  chmod 700 "$(dirname "$OSQUERY_TAILSCALE_STATE")" # restore before asserting / teardown

  [[ $status -ne 0 ]] || {
    echo "expected nonzero when the baseline could not be persisted, got $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'degraded'
}
