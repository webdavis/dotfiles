#!/usr/bin/env bash
set -euo pipefail

function render_plannotator_hooks() {
  local repo fixture
  repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  fixture="$(mktemp -d)"
  printf '%s' "$2" |
    HOME="$fixture" CI=1 chezmoi --config /dev/null --config-format toml --source "$fixture" \
      --override-data '{"chezmoi":{"homeDir":"/Users/fixture"}}' execute-template \
      --no-tty --with-stdin --file "$repo/$1"
}

function test_plannotator_codex_preserves_existing_hooks_and_metadata() {
  local input actual
  input='{"description":"operator hooks","hooks":{"state":{"opaque":{"trusted":false}},"SessionStart":[{"hooks":[{"type":"command","command":"herdr session"}]}],"Stop":[{"hooks":[{"type":"command","command":"pns hook stop","timeout":12}]}],"PermissionRequest":[{"hooks":[{"type":"command","command":"pns hook blocked"}]}]}}'
  actual="$(render_plannotator_hooks private_dot_codex/modify_hooks.json "$input")"
  assert_same "$(printf '%s' "$input" | jq -Sc .)" \
    "$(printf '%s' "$actual" | jq -Sc 'del(.hooks.Stop[-1])')"
  assert_same '{"hooks":[{"command":"/Users/fixture/.local/bin/plannotator","timeout":345600,"type":"command"}]}' \
    "$(printf '%s' "$actual" | jq -Sc '.hooks.Stop[-1]')"
}

function test_plannotator_gemini_preserves_settings_and_other_matchers() {
  local input actual
  input='{"experimental":{"plan":false,"other":true},"hooks":{"BeforeTool":[{"matcher":"read_file","hooks":[{"type":"command","command":"plannotator","timeout":2}]},{"matcher":"exit_plan_mode","sequential":true,"hooks":[{"type":"command","command":"audit plan"}]}],"AfterTool":[]},"security":{"auth":{"selectedType":"oauth-personal"}}}'
  actual="$(render_plannotator_hooks private_dot_gemini/modify_settings.json "$input")"
  assert_same "$(printf '%s' "$input" | jq -Sc .)" \
    "$(printf '%s' "$actual" | jq -Sc 'del(.hooks.BeforeTool[-1])')"
  assert_same '{"hooks":[{"command":"/Users/fixture/.local/bin/plannotator","timeout":345600,"type":"command"}],"matcher":"exit_plan_mode"}' \
    "$(printf '%s' "$actual" | jq -Sc '.hooks.BeforeTool[-1]')"
}

function test_plannotator_empty_settings_bootstrap_and_repeat_without_duplicates() {
  local template event input actual repeated
  for template in private_dot_codex/modify_hooks.json private_dot_gemini/modify_settings.json; do
    event=Stop
    [[ $template != *gemini* ]] || event=BeforeTool
    for input in '' $' \n\t'; do
      actual="$(render_plannotator_hooks "$template" "$input")"
      assert_same 1 "$(printf '%s' "$actual" | jq --arg event "$event" '.hooks[$event] | length')"
      if [[ $event == BeforeTool ]]; then
        assert_same true "$(printf '%s' "$actual" | jq '.experimental.plan')"
      fi
      repeated="$(render_plannotator_hooks "$template" "$actual")"
      assert_same "$actual" "$repeated"
    done
  done
}

function test_plannotator_existing_handler_is_updated_without_replacing_its_group() {
  local template event input actual expected
  for template in private_dot_codex/modify_hooks.json private_dot_gemini/modify_settings.json; do
    event=Stop
    [[ $template != *gemini* ]] || event=BeforeTool
    input="$(jq -nc --arg event "$event" '{hooks:{($event):[{matcher:"exit_plan_mode",sequential:true,hooks:[{type:"command",command:"audit plan"},{type:"command",command:"plannotator",timeout:3,name:"review"}]}]}}')"
    actual="$(render_plannotator_hooks "$template" "$input")"
    expected="$(printf '%s' "$input" | jq -Sc --arg event "$event" '.hooks[$event][0].hooks[1] |= (.command="/Users/fixture/.local/bin/plannotator" | .timeout=345600)')"
    assert_same "$expected" "$(printf '%s' "$actual" | jq -Sc .)"
    assert_same "$actual" "$(render_plannotator_hooks "$template" "$actual")"
  done
}

function test_plannotator_noop_preserves_raw_formatting() {
  local template event input
  for template in private_dot_codex/modify_hooks.json private_dot_gemini/modify_settings.json; do
    event=Stop
    [[ $template != *gemini* ]] || event=BeforeTool
    input="$(jq -nc --arg event "$event" '{hooks:{($event):[{matcher:"exit_plan_mode",hooks:[{type:"command",command:"/Users/fixture/.local/bin/plannotator",timeout:345600}]}]},custom:"keep"}')"
    input="  $input"$'\n\n'
    assert_same "${input}END" "$(
      render_plannotator_hooks "$template" "$input"
      printf END
    )"
  done
}

function test_plannotator_rejects_malformed_json_without_emitting_a_replacement() {
  local template input output status
  for template in private_dot_codex/modify_hooks.json private_dot_gemini/modify_settings.json; do
    for input in '{"hooks":' '{}{}' 'null' '[]' '{"hooks":null}' '{"hooks":[]}'; do
      status=0
      output="$(render_plannotator_hooks "$template" "$input" 2>/dev/null)" || status=$?
      assert_same 1 "$status"
      assert_empty "$output"
    done
  done
}

function test_plannotator_rejects_malformed_event_containers() {
  local template event input output status
  for template in private_dot_codex/modify_hooks.json private_dot_gemini/modify_settings.json; do
    event=Stop
    [[ $template != *gemini* ]] || event=BeforeTool
    for input in 'null' '{}' '[{}]' '[{"hooks":null}]' '[{"hooks":[null]}]'; do
      status=0
      output="$(render_plannotator_hooks "$template" "{\"hooks\":{\"$event\":$input}}" 2>/dev/null)" || status=$?
      assert_same 1 "$status"
      assert_empty "$output"
    done
  done
}
