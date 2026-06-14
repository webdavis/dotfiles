#!/usr/bin/env bats
# Agent attack-surface page tier: the AI agents (Claude Code, Paseo, Hermes) are the
# operator's primary remote-access path, so a port newly exposed off-loopback or a
# change to their auth secrets is high-confidence, rare, and actionable → page.

load lib

setup() { setup_harness; }
teardown() { teardown_harness; }

@test "T-PAGE-exposure: an agent/MCP service bound off-loopback pages (names the process)" {
  run_alerter "$(row agent_exposure_changed added 1 '{"address":"0.0.0.0","port":"8000","name":"workspace-mcp"}')"
  assert_page_has 8000
  assert_page_has workspace-mcp
}

@test "T-PAGE-webhooksecret: a change to the alerter HMAC key pages" {
  # The single most critical credential — tampering forges or mutes every alert.
  run_alerter "$(row agent_authfile_changed added 1 '{"path":"/Users/x/.config/osquery/webhook-secret","sha256":"DEADBEEF"}')"
  assert_page_has webhook-secret
}

@test "T-PAGE-paseokey: a change to the paseo daemon keypair pages" {
  run_alerter "$(row agent_authfile_changed added 1 '{"path":"/Users/x/.paseo/daemon-keypair.json","sha256":"DEADBEEF"}')"
  assert_page_has daemon-keypair
}

@test "T-DIG-authfile: a rotation-prone credential change digests, does not page" {
  run_alerter "$(row agent_authfile_changed added 1 '{"path":"/Users/x/.hermes/.env","sha256":"DEADBEEF"}')"
  assert_no_page
  assert_digest_count 1
}

@test "T-LOG-agentbin: an agent binary change is log-only (recorded, never delivered)" {
  # Hash changes cannot tell a frequent legit self-update from a swap, so they are
  # inherently noisy — recorded in results.log for forensics, never paged or digested.
  run_alerter "$(row agent_binary_changed added 1 '{"path":"/Users/x/.local/bin/codex"}')"
  assert_no_dispatch
  assert_digest_count 0
}
