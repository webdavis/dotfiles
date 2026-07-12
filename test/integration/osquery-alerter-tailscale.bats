#!/usr/bin/env bats
# Tailscale funnel poller: pages on the off->on transition of `tailscale funnel` (a local port
# newly exposed to the PUBLIC internet). Funnel traffic tunnels through tailscaled, so osquery
# can't see it as a listener — this poller is the only way. R2-5: a monitoring GAP (missing binary,
# failed/empty status, corrupt state) is CRIT (it reaches the remote channel), the prior valid
# state is preserved (not swallowed into a false "inactive"), and state is written atomically.

load ../fixtures/osquery-alerter-lib

setup() { setup_tailscale_harness; }
teardown() { teardown_harness; }

@test "T-PAGE-funnel: a funnel turning on pages (public exposure)" {
  run_tailscale_monitor inactive "https://dresden.tailnet.ts.net/ (Funnel on)|--> http://127.0.0.1:8000"
  assert_page_has Funnel
}

@test "T-NEG-funnel-steady: an already-active funnel does not re-page" {
  run_tailscale_monitor active "https://dresden.tailnet.ts.net/ (Funnel on)|--> http://127.0.0.1:8000"
  assert_no_dispatch
}

@test "T-NEG-funnel-none: no funnel config is silent" {
  run_tailscale_monitor inactive "No serve config"
  assert_no_dispatch
}

@test "T-NEG-funnel-firstrun-inactive: a first run with NO funnel baselines silently" {
  run_tailscale_monitor "" "No serve config"
  assert_no_dispatch
}

@test "T-PAGE-funnel-firstrun-active: a first run with the funnel ALREADY active pages (FX7)" {
  run_tailscale_monitor "" "https://dresden.tailnet.ts.net/ (Funnel on)|--> http://127.0.0.1:8000"
  assert_page_has Funnel
}

# R2-5: a monitoring gap is CRIT (reaches the remote channel — a WARN was dropped before the POST).

@test "T-CRIT-ts-missing-bin: a missing tailscale binary pages CRIT (blindness reaches remote) (R2-5)" {
  # The dead-monitor regression: the GUI-path default silently disabled funnel paging on the
  # headless-formula install. Now it is a CRIT page (was a WARN the dispatcher dropped).
  run_tailscale_monitor_missing_bin ""
  assert_page_has "BLIND"
}

@test "T-NEG-ts-missing-bin-once: the missing-binary gap pages once, not every 60s (R2-5)" {
  run_tailscale_monitor_missing_bin "missing"
  assert_no_dispatch
}

@test "T-PAGE-funnel-after-blind: a funnel found active on recovery from a blind window pages" {
  run_tailscale_monitor "missing" "https://dresden.tailnet.ts.net/ (Funnel on)|--> http://127.0.0.1:8000"
  assert_page_has Funnel
}

@test "T-CRIT-ts-status-fail: a failing \`funnel status\` (rc=1) is a CRIT gap, not a silent inactive (R2-5)" {
  run_tailscale_monitor active "" 1
  assert_page_has "BLIND"
}

@test "T-NEG-ts-status-fail-preserves: a status failure PRESERVES the prior valid funnel state (R2-5)" {
  # rc=1 must not overwrite a known-active baseline with a false "inactive" — the prior state is
  # preserved so a real transition is still detectable when the command recovers.
  run_tailscale_monitor active "" 1
  [ "$(tailscale_state_funnel)" = "active" ]
}

@test "T-CRIT-ts-empty-output: an empty (rc=0) status output is a CRIT gap, not a silent inactive (R2-5)" {
  run_tailscale_monitor active "" 0
  assert_page_has "BLIND"
}

@test "T-CRIT-ts-corrupt-state-active: a corrupt prior state is NOT a baseline, so an active funnel pages (R2-5)" {
  # A garbage state file must not be trusted as an "active" baseline that suppresses the page.
  run_tailscale_monitor corrupt "https://dresden.tailnet.ts.net/ (Funnel on)|--> http://127.0.0.1:8000"
  assert_page_has Funnel
}

@test "T-RESOLVE-ts-path: with no env override the poller finds tailscale via PATH" {
  run_tailscale_monitor_path_resolved inactive "https://dresden.tailnet.ts.net/ (Funnel on)|--> http://127.0.0.1:8000"
  assert_page_has Funnel
}
