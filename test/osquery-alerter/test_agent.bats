#!/usr/bin/env bats
# Agent attack-surface page tier: the AI agents (Claude Code, Paseo, Hermes) are the
# operator's primary remote-access path, so a port newly exposed off-loopback or a
# change to their auth secrets is high-confidence, rare, and actionable → page.

load lib

setup() { setup_harness; }
teardown() { teardown_harness; }

@test "T-PAGE-exposure: an agent port bound off-loopback pages" {
  run_alerter "$(row agent_exposure_changed added 1 '{"address":"0.0.0.0","port":"8644"}')"
  assert_page_has 8644
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

@test "T-DIG-agentbin: an agent binary change digests, does not page" {
  run_alerter "$(row agent_binary_changed added 1 '{"path":"/Users/x/.local/bin/codex"}')"
  assert_no_page
  assert_digest_count 1
}
