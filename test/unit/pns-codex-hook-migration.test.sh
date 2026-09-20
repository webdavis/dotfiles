#!/usr/bin/env bash
set -euo pipefail

function pns_hook_migration_fixture() {
  local fixture
  fixture="$(mktemp -d)"
  mkdir -p "$fixture/.codex" "$fixture/.cargo/bin"
  printf '#!/usr/bin/env bash\nexit 99\n' >"$fixture/.cargo/bin/pns"
  chmod +x "$fixture/.cargo/bin/pns"
  printf '[hooks.state]\noperator_trust = false\n' >"$fixture/.codex/config.toml"
  printf '%s' "$fixture"
}

# The two rows every run now adds, as a jq expression over the object under
# test, so each expectation below states only what its own fixture carries.
function pns_hook_migration_answered_rows() {
  # SC2016 is the point: `$root` is a jq variable the caller binds with --arg.
  # shellcheck disable=SC2016
  printf '%s' ' |
    ("PNS_PRODUCER=codex " + $root + "/.cargo/bin/pns hook resolved") as $r |
    .hooks.PostToolUse = ((.hooks.PostToolUse // []) + [{hooks:[{type:"command",command:$r}]}]) |
    .hooks.Interrupt = ((.hooks.Interrupt // []) + [{hooks:[{type:"command",command:$r}]}])'
}

function pns_hook_migration_run() {
  local repo
  repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  HOME="$1" bash "$repo/dot_local/libexec/pns/hooks/codex/executable_install-hooks.sh"
}

function test_pns_hook_migration_collapses_owned_duplicates_and_preserves_other_entries() {
  local fixture before expected actual warning
  fixture="$(pns_hook_migration_fixture)"
  before="$(jq -nc --arg root "$fixture" '
    ("PNS_AGENT=codex " + $root + "/.local/libexec/pns/pns hook stop") as $old |
    ("PNS_PRODUCER=codex " + $root + "/.cargo/bin/pns hook stop") as $new |
    {custom:{preserve:true},hooks:{SessionStart:[{hooks:[{type:"command",command:"herdr session"}]}],
      Stop:[{matcher:"*",custom:"group",hooks:[{type:"command",command:$old,timeout:12,custom:"handler"},
        {type:"command",command:("echo " + $old)},
        {type:"command",command:"RELAY_AGENT=other /usr/local/bin/unrelated stop"},
        {type:"command",command:("PNS_AGENT=codex " + $root + "/.local/libexec/pns/hooks/relay-agent.sh done --custom")},
        {type:"command",command:"PNS_AGENT=codex /Users/another/.local/libexec/pns/pns hook stop"},
        {type:"prompt",command:$old},
        {type:"command",command:[$old]},
        {type:"command",command:("PNS_AGENT=codex " + $root + "/.local/libexec/pns/pns hook blocked")}]},
        {matcher:"*",custom:"group",hooks:[{type:"command",command:$new,timeout:12,custom:"handler"}]},
        {matcher:"unused",custom:"empty group",hooks:[]}],
      PermissionRequest:[{hooks:[{type:"command",command:($old|sub(" stop$";" blocked"))}]},
        {hooks:[{type:"command",command:($new|sub(" stop$";" blocked"))}]}],
      CustomEvent:[{hooks:[{type:"command",command:$old}]}]}}')"
  printf '%s\n' "$before" >"$fixture/.codex/hooks.json"
  expected="$(jq -Sc --arg root "$fixture" '
    .hooks.Stop[0].hooks[0].command = ("PNS_PRODUCER=codex " + $root + "/.cargo/bin/pns hook stop") |
    .hooks.PermissionRequest[0].hooks[0].command = ("PNS_PRODUCER=codex " + $root + "/.cargo/bin/pns hook blocked --remind") |
    del(.hooks.Stop[1], .hooks.PermissionRequest[1])'"$(pns_hook_migration_answered_rows)" <<<"$before")"
  warning="$(pns_hook_migration_run "$fixture" 2>&1)"
  actual="$(jq -Sc . "$fixture/.codex/hooks.json")"
  assert_same "$expected" "$actual"
  assert_contains '/hooks' "$warning"
  assert_empty "$(pns_hook_migration_run "$fixture" 2>&1)"
  assert_same "$actual" "$(jq -Sc . "$fixture/.codex/hooks.json")"
  assert_same $'[hooks.state]\noperator_trust = false' "$(cat "$fixture/.codex/config.toml")"
}

function test_pns_hook_migration_retains_legacy_handler_metadata_without_a_current_duplicate() {
  local fixture expected
  fixture="$(pns_hook_migration_fixture)"
  jq -n --arg root "$fixture" '{description:"operator hooks",hooks:{
    Stop:[{matcher:"*",custom:{group:true},hooks:[{type:"command",timeout:15,custom:{handler:true},
      command:("PNS_AGENT=codex " + $root + "/.local/libexec/pns/pns hook stop")}]}],
    PermissionRequest:[{custom:"blocked",hooks:[{type:"command",async:true,
      command:("PNS_AGENT=codex " + $root + "/.local/libexec/pns/pns hook blocked")}]}]}}' \
    >"$fixture/.codex/hooks.json"
  expected="$(jq -Sc --arg root "$fixture" '
    .hooks.Stop[0].hooks[0].command = ("PNS_PRODUCER=codex " + $root + "/.cargo/bin/pns hook stop") |
    .hooks.PermissionRequest[0].hooks[0].command = ("PNS_PRODUCER=codex " + $root + "/.cargo/bin/pns hook blocked --remind")'"$(pns_hook_migration_answered_rows)" "$fixture/.codex/hooks.json")"
  pns_hook_migration_run "$fixture" 2>/dev/null
  assert_same "$expected" "$(jq -Sc . "$fixture/.codex/hooks.json")"
}

function assert_pns_hook_generation() {
  local fixture
  fixture="$(pns_hook_migration_fixture)"
  jq -n --arg root "$fixture" --arg prefix "$1" --arg executable "$2" \
    --arg stop "$3" --arg blocked "$4" '
    {hooks:{Stop:[{hooks:[{type:"command",command:($prefix + "=codex " + $root + "/" + $executable + " " + $stop)}]}],
      PermissionRequest:[{hooks:[{type:"command",command:($prefix + "=codex " + $root + "/" + $executable + " " + $blocked)}]}]}}' \
    >"$fixture/.codex/hooks.json"
  pns_hook_migration_run "$fixture" 2>/dev/null
  assert_same "PNS_PRODUCER=codex $fixture/.cargo/bin/pns hook stop" \
    "$(jq -r '.hooks.Stop[].hooks[].command' "$fixture/.codex/hooks.json")"
  assert_same "PNS_PRODUCER=codex $fixture/.cargo/bin/pns hook blocked --remind" \
    "$(jq -r '.hooks.PermissionRequest[].hooks[].command' "$fixture/.codex/hooks.json")"
}

function test_pns_hook_migration_recognizes_the_relay_bin_script() {
  assert_pns_hook_generation RELAY_AGENT .local/bin/relay-agent.sh 'done' blocked
}

function test_pns_hook_migration_recognizes_the_relay_codex_hooks_script() {
  assert_pns_hook_generation RELAY_AGENT .local/libexec/pns/codex-hooks/relay-agent.sh 'done' blocked
}

function test_pns_hook_migration_recognizes_the_relay_owned_hooks_script() {
  assert_pns_hook_generation RELAY_AGENT .local/libexec/pns/hooks/relay-agent.sh 'done' blocked
}

function test_pns_hook_migration_recognizes_the_pns_owned_hooks_script() {
  assert_pns_hook_generation PNS_AGENT .local/libexec/pns/hooks/relay-agent.sh 'done' blocked
}

function test_pns_hook_migration_recognizes_the_relay_engine_command() {
  assert_pns_hook_generation RELAY_AGENT .local/libexec/pns/pns 'hook stop' 'hook blocked'
}

function test_pns_hook_migration_recognizes_the_legacy_pns_engine_command() {
  assert_pns_hook_generation PNS_AGENT .local/libexec/pns/pns 'hook stop' 'hook blocked'
}

function test_pns_hook_migration_rewrites_the_deployed_agent_variable() {
  # THE GUARD ON THE RENAME: every Codex hook already on this machine carries
  # PNS_AGENT at the current path, so an installer that only wrote new rows
  # would leave Codex events with no producer name until they were edited by
  # hand.
  assert_pns_hook_generation PNS_AGENT .cargo/bin/pns 'hook stop' 'hook blocked'
}

function test_pns_hook_migration_retains_the_current_cargo_command() {
  assert_pns_hook_generation PNS_PRODUCER .cargo/bin/pns 'hook stop' 'hook blocked'
}

function test_pns_hook_migration_preserves_conflicting_duplicate_metadata_for_review() {
  local fixture before warning conflict
  fixture="$(pns_hook_migration_fixture)"
  for conflict in handler group; do
    jq -n --arg root "$fixture" --arg conflict "$conflict" '{hooks:{Stop:[
      {hooks:[{type:"command",timeout:12,command:("PNS_AGENT=codex " + $root + "/.local/libexec/pns/pns hook stop")}]},
      {hooks:[{type:"command",timeout:12,command:("PNS_PRODUCER=codex " + $root + "/.cargo/bin/pns hook stop")}]}]}} |
      if $conflict == "handler" then .hooks.Stop[1].hooks[0].timeout = 30
      else .hooks.Stop[1].custom = "second group" end' >"$fixture/.codex/hooks.json"
    before="$(cat "$fixture/.codex/hooks.json")"
    warning="$(pns_hook_migration_run "$fixture" 2>&1)"
    assert_same "$before" "$(cat "$fixture/.codex/hooks.json")"
    assert_contains 'metadata' "$warning"
  done
}

function test_pns_hook_migration_wires_both_answered_events_beside_a_foreign_row() {
  local fixture resolved
  fixture="$(pns_hook_migration_fixture)"
  resolved="PNS_PRODUCER=codex $fixture/.cargo/bin/pns hook resolved"
  jq -n '{hooks:{PostToolUse:[{hooks:[{type:"command",command:"herdr tool"}]}]}}' \
    >"$fixture/.codex/hooks.json"
  pns_hook_migration_run "$fixture" 2>/dev/null
  assert_same "herdr tool | $resolved" \
    "$(jq -r '[.hooks.PostToolUse[].hooks[].command] | join(" | ")' "$fixture/.codex/hooks.json")"
  assert_same "$resolved" \
    "$(jq -r '[.hooks.Interrupt[].hooks[].command] | join(" | ")' "$fixture/.codex/hooks.json")"
}

function test_pns_hook_migration_writes_each_answered_row_once_across_two_runs() {
  local fixture resolved
  fixture="$(pns_hook_migration_fixture)"
  resolved="PNS_PRODUCER=codex $fixture/.cargo/bin/pns hook resolved"
  pns_hook_migration_run "$fixture" 2>/dev/null
  pns_hook_migration_run "$fixture" 2>/dev/null
  assert_same "$resolved" \
    "$(jq -r '[.hooks.PostToolUse[].hooks[].command] | join(" | ")' "$fixture/.codex/hooks.json")"
  assert_same "$resolved" \
    "$(jq -r '[.hooks.Interrupt[].hooks[].command] | join(" | ")' "$fixture/.codex/hooks.json")"
}
