#!/usr/bin/env bats
# Harness smoke test: prove lib.bash can drive the real alerter and observe that
# nothing paged. Green against today's alerter (a counter==0 baseline produces no
# CRIT dispatch).

load lib

setup() { setup_harness; }
teardown() { teardown_harness; }

@test "harness drives the alerter and a counter==0 baseline does not page" {
  run_alerter "$(row new_admin_user added 0 '{}')"
  assert_no_page
}
