#!/usr/bin/env bats
# Tailscale funnel poller: pages on the off->on transition of `tailscale funnel`
# (a local port newly exposed to the PUBLIC internet). Funnel traffic tunnels through
# tailscaled, so osquery can't see it as a listener — this poller is the only way.

load lib

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

@test "T-NEG-funnel-firstrun: the first run baselines silently" {
  run_tailscale_monitor "" "https://dresden.tailnet.ts.net/ (Funnel on)|--> http://127.0.0.1:8000"
  assert_no_dispatch
}

# Dead-monitor regression (the GUI-path default silently disabled funnel paging on
# the headless-formula install): a missing binary must be LOUD, never a silent exit.

@test "T-WARN-ts-missing-bin: a missing tailscale binary warns instead of silently exiting" {
  run_tailscale_monitor_missing_bin ""
  assert_warn_has "funnel monitoring"
}

@test "T-WARN-ts-missing-bin-once: the missing-binary warning fires once, not every 60s" {
  run_tailscale_monitor_missing_bin "missing"
  assert_no_dispatch
}

@test "T-PAGE-funnel-after-blind: a funnel found active on recovery from a blind window pages" {
  run_tailscale_monitor "missing" "https://dresden.tailnet.ts.net/ (Funnel on)|--> http://127.0.0.1:8000"
  assert_page_has Funnel
}

@test "T-RESOLVE-ts-path: with no env override the poller finds tailscale via PATH" {
  run_tailscale_monitor_path_resolved inactive "https://dresden.tailnet.ts.net/ (Funnel on)|--> http://127.0.0.1:8000"
  assert_page_has Funnel
}
