#!/usr/bin/env bash
set -euo pipefail

function render_statusline_collector() {
  local repo fixture
  repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  fixture="$(mktemp -d)"
  printf '%s\n' "$1" |
    HOME="$fixture" CI=1 chezmoi --config /dev/null --config-format toml --source "$repo" execute-template \
      --no-tty --with-stdin --file "$repo/private_dot_claude/modify_settings.json" |
    jq -c '.statusLine'
}

function test_statusline_preserves_the_quota_collector_and_its_refresh_interval() {
  local collector actual
  collector='{"type":"command","command":"HERDR_PLUGIN_STATE_DIR=\u0027/fixture/state\u0027 \u0027/fixture/herdr-agent-quota\u0027 claude-statusline","refreshInterval":120}'
  actual="$(render_statusline_collector "{\"statusLine\":$collector}")"
  assert_same "$(printf '%s' "$collector" | jq -Sc .)" "$(printf '%s' "$actual" | jq -Sc .)"
}

function test_statusline_replaces_unrelated_live_commands() {
  local actual
  actual="$(render_statusline_collector '{"statusLine":{"type":"command","command":"echo unrelated","refreshInterval":9}}')"
  assert_same true "$(printf '%s' "$actual" | jq '.command | endswith("/.claude/statusline-command.sh")')"
  assert_same null "$(printf '%s' "$actual" | jq '.refreshInterval')"
}

function test_statusline_handles_absent_and_malformed_collectors() {
  local input actual
  for input in '{}' '{"statusLine":null}' '{"statusLine":"old command"}' '{"statusLine":{"command":42}}'; do
    actual="$(render_statusline_collector "$input")"
    assert_same true "$(printf '%s' "$actual" | jq '.command | endswith("/.claude/statusline-command.sh")')"
  done
}
