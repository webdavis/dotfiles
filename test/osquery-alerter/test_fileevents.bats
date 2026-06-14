#!/usr/bin/env bats
# file_events page tier: a written SSH key or sshd_config is remote-auth tampering
# and pages. Only CREATED/UPDATED is actionable — a DELETE is your own revert noise.

load lib

setup() { setup_harness; }
teardown() { teardown_harness; }

@test "T-PAGE-authkeys: an authorized_keys file CREATED pages" {
  run_alerter "$(file_event_row authorized_keys /Users/x/.ssh/authorized_keys CREATED)"
  assert_page_has authorized_keys
}

@test "T-NEG-authkeys-delete: an authorized_keys DELETE does not page" {
  run_alerter "$(file_event_row authorized_keys /Users/x/.ssh/authorized_keys DELETED)"
  assert_no_page
}

@test "T-PAGE-sshd: an sshd_config UPDATED pages" {
  run_alerter "$(file_event_row sshd_config /etc/ssh/sshd_config UPDATED)"
  assert_page_has sshd_config
}
