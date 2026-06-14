#!/usr/bin/env bats
# Digest builder (osquery-digest.sh): turns the spool into ONE grouped, silent,
# empty-suppressed #priority message, then rotates the live store to .last.

load lib

setup() { setup_harness; }
teardown() { teardown_harness; }

@test "T-DIGM-empty: an absent store produces silence and exit 0" {
  run run_digest
  [ "$status" -eq 0 ]
  assert_no_dispatch
}

@test "T-DIGM-empty2: a whitespace-only store produces silence" {
  mkdir -p "$(dirname "$OSQUERY_DIGEST_STORE")"
  printf '   \n' >"$OSQUERY_DIGEST_STORE"
  run run_digest
  [ "$status" -eq 0 ]
  assert_no_dispatch
}

@test "T-DIGM-send: a non-empty store sends exactly one grouped digest" {
  seed_digest \
    "$(digest_record persistence_launchd com.foo.agent 'persistence_launchd com.foo.agent')" \
    "$(digest_record persistence_launchd com.bar.agent 'persistence_launchd com.bar.agent')"
  run_digest
  assert_digest_sent
}

@test "T-DIGM-route: the digest send is silent (empty sound, not a page ping)" {
  seed_digest "$(digest_record system_extensions_new io.tailscale 'system_extensions_new io.tailscale')"
  run_digest
  assert_digest_silent
}

@test "T-DIGM-group: two events for one detector group under one header, not two messages" {
  seed_digest \
    "$(digest_record persistence_launchd com.foo.agent 'persistence_launchd com.foo.agent')" \
    "$(digest_record persistence_launchd com.bar.agent 'persistence_launchd com.bar.agent')"
  run_digest
  assert_digest_sent
  assert_digest_body_has '**persistence_launchd** (2)'
  assert_digest_body_has 'com.foo.agent'
  assert_digest_body_has 'com.bar.agent'
}

@test "T-DIGM-rollup: more than ten entries roll up with a +K more footer" {
  local lines=() i
  for i in $(seq 1 20); do
    lines+=("$(digest_record persistence_launchd "com.foo.$i" "persistence_launchd com.foo.$i")")
  done
  seed_digest "${lines[@]}"
  run_digest
  assert_digest_sent
  assert_digest_body_has '+10 more'
}

@test "T-DIGM-clear: after a send the store rotates to .last and a re-run is silent" {
  seed_digest "$(digest_record sudoers /etc/sudoers.d/foo 'sudoers /etc/sudoers.d/foo')"
  run_digest
  assert_store_rotated
  : >"$SEND_ALERT_LOG"
  run run_digest
  [ "$status" -eq 0 ]
  assert_no_dispatch
}

@test "T-DIGM-e2e: a digested finding flows alerter -> spool -> builder" {
  # The alerter digests a sysext (writes one spool line, dispatches nothing); the
  # builder then renders it. Proves _digest_append and the builder agree on field
  # names (.identity), end to end.
  run_alerter "$(row pack_intrusion-detection_system_extensions_new added 1 '{"identifier":"io.example.ext","team":"TEAMID"}')"
  assert_digest_count 1
  assert_no_dispatch
  run_digest
  assert_digest_sent
  assert_digest_body_has 'io.example.ext'
}
