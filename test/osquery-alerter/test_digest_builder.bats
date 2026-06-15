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

@test "T-DIGM-overflow: more than twelve detector groups roll up with a marker, not a silent cut" {
  # The only global bound was head -c 1800, which drops trailing groups and cuts the
  # last line mid-string with no indicator. Cap the group count and emit a footer so
  # the loss is signalled (the dropped content still survives in results.log).
  local lines=() i
  for i in $(seq 1 15); do
    lines+=("$(digest_record "detector_$i" "id_$i" "detector_$i id_$i")")
  done
  seed_digest "${lines[@]}"
  run_digest
  assert_digest_sent
  assert_digest_body_has 'more detector group(s)'
}

@test "T-DIGM-cadence: the builder sends on any invocation — cadence is launchd's, not an internal gate" {
  # A hidden wall-clock gate sneaking into the builder would silently stop the daily
  # digest; assert it sends whenever invoked against a seeded store.
  seed_digest "$(digest_record sudoers /etc/sudoers.d/foo 'sudoers /etc/sudoers.d/foo')"
  run_digest
  assert_digest_sent
}

@test "T-DIGM-heartbeat-sep: the digest body carries no heartbeat/healthy content" {
  # The digest summarizes findings only; uptime/heartbeat is a separate signal. Guard
  # against a future merge that leaks a "healthy"/✅ line into the calm daily summary.
  seed_digest "$(digest_record system_extensions_new io.example 'system_extensions_new io.example')"
  run_digest
  assert_digest_sent
  run grep -iE 'healthy|✅|all clear' "$SEND_ALERT_LOG"
  [ "$status" -ne 0 ]
}

@test "T-DIGM-all-torn: a spool of only torn lines sends nothing, not a blank N-item message" {
  # Every line unparseable (an interrupted _digest_append with zero clean appends):
  # the rendered body is empty, yet Guard 2 (non-whitespace bytes) passes and
  # item_count counts the torn lines. The builder must NOT POST a misleading silent
  # "2 item(s)" with an empty body — that inverts the empty-suppression invariant.
  mkdir -p "$(dirname "$OSQUERY_DIGEST_STORE")"
  printf '%s\n%s\n' '{"detector":"x' '{"oops' >"$OSQUERY_DIGEST_STORE"
  run run_digest
  [ "$status" -eq 0 ]
  assert_no_dispatch
  assert_store_rotated
}

@test "T-DIGM-torn-line: a malformed spool line is skipped; the day's digest still sends" {
  # A SIGKILLed / ENOSPC-interrupted _digest_append can leave one torn (non-JSON)
  # line in the spool. The builder must skip it, NOT abort the whole run under
  # set -e + pipefail and silently lose the day's findings — empty-suppression
  # would make that total loss indistinguishable from "nothing happened".
  mkdir -p "$(dirname "$OSQUERY_DIGEST_STORE")"
  {
    printf '%s\n' "$(digest_record persistence_launchd com.good.one 'persistence_launchd com.good.one')"
    printf '%s\n' '{"detector":"persistence_launchd","identity":"com.tor'
    printf '%s\n' "$(digest_record persistence_launchd com.good.two 'persistence_launchd com.good.two')"
  } >"$OSQUERY_DIGEST_STORE"
  run run_digest
  [ "$status" -eq 0 ]
  assert_digest_sent
  assert_digest_body_has 'com.good.one'
  assert_digest_body_has 'com.good.two'
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
