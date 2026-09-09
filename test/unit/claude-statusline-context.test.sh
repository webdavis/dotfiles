#!/usr/bin/env bash
set -euo pipefail

function statusline_context() {
  local expected="$1" settings="$2" percent="${3-unset}" window="${4-unset}" output="${5-unset}"
  local fixture script actual status
  fixture=$(mktemp -d "${TMPDIR:-/tmp}/statusline-context.XXXXXX")
  script="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/private_dot_claude/executable_statusline-command.sh"
  mkdir -p "$fixture/bin" "$fixture/home/.claude" "$fixture/config" "$fixture/data" "$fixture/state" "$fixture/cache" "$fixture/run" "$fixture/tmp"
  printf '%s\n' "$settings" >"$fixture/home/.claude/settings.json"
  printf '#!/bin/bash\nset -euo pipefail\nexit 1\n' >"$fixture/bin/git"
  printf '#!/bin/bash\nset -euo pipefail\nprintf "fixture-host\\n"\n' >"$fixture/bin/hostname"
  chmod 700 "$fixture/bin/git" "$fixture/bin/hostname"
  ln -s "$(command -v jq)" "$fixture/bin/jq"
  local -a environment=(
    "HOME=$fixture/home" "PATH=$fixture/bin:/usr/bin:/bin" "LC_ALL=C"
    "XDG_CONFIG_HOME=$fixture/config" "XDG_DATA_HOME=$fixture/data"
    "XDG_STATE_HOME=$fixture/state" "XDG_CACHE_HOME=$fixture/cache"
    "XDG_RUNTIME_DIR=$fixture/run" "XDG_CONFIG_DIRS=$fixture/config"
    "XDG_DATA_DIRS=$fixture/data" "CLAUDE_CONFIG_DIR=$fixture/home/.claude"
    "TMPDIR=$fixture/tmp" "TMP=$fixture/tmp" "TEMP=$fixture/tmp"
    "GIT_CONFIG_GLOBAL=/dev/null" "GIT_CONFIG_SYSTEM=/dev/null"
  )
  [[ $percent == unset ]] || environment+=("CLAUDE_AUTOCOMPACT_PCT_OVERRIDE=$percent")
  [[ $window == unset ]] || environment+=("CLAUDE_CODE_AUTO_COMPACT_WINDOW=$window")
  [[ $output == unset ]] || environment+=("CLAUDE_CODE_MAX_OUTPUT_TOKENS=$output")
  if actual=$(printf '%s\n' '{"workspace":{"current_dir":"/fixture/project","project_dir":"/fixture/project"},"context_window":{"used_percentage":60,"context_window_size":1000000}}' |
    env -i "${environment[@]}" /bin/bash "$script" 2>"$fixture/stderr"); then
    status=0
  else
    status=$?
  fi
  actual=$(printf '%s' "$actual" | sed -E $'s/\033\\[[0-9;]*m//g' | sed -n 's/.*ctx:\([^[:space:]]*\).*/\1/p')
  assert_same "$expected" "$actual"
  assert_same "0" "$status"
  assert_same "" "$(cat "$fixture/stderr")"
}

function test_context_parses_leading_zero_percent_as_decimal() {
  statusline_context '~81%/735k' '{"env":{"CLAUDE_AUTOCOMPACT_PCT_OVERRIDE":"075"}}'
}

function test_context_accepts_a_leading_zero_eight_percent() {
  statusline_context '~765%/78k' '{"env":{"CLAUDE_AUTOCOMPACT_PCT_OVERRIDE":"08"}}'
}

function test_context_preserves_a_decimal_percent() {
  statusline_context '~81%/739k' '{"env":{"CLAUDE_AUTOCOMPACT_PCT_OVERRIDE":"75.5"}}'
}

function test_context_ignores_a_zero_percent_without_arithmetic_errors() {
  statusline_context '~62%/967k' '{"env":{"CLAUDE_AUTOCOMPACT_PCT_OVERRIDE":"0"}}'
}

function test_context_ignores_a_percent_above_one_hundred() {
  statusline_context '~62%/967k' '{"env":{"CLAUDE_AUTOCOMPACT_PCT_OVERRIDE":"101"}}'
}

function test_context_uses_the_active_environment_before_user_settings() {
  statusline_context '~122%/490k' '{"env":{"CLAUDE_AUTOCOMPACT_PCT_OVERRIDE":"75"}}' '50'
}

function test_context_measures_against_the_configured_compaction_window() {
  statusline_context '~166%/360k' '{"env":{"CLAUDE_AUTOCOMPACT_PCT_OVERRIDE":"75"}}' unset '500000'
}

function test_context_caps_one_hundred_percent_at_the_default_threshold() {
  statusline_context '~62%/967k' '{"env":{"CLAUDE_AUTOCOMPACT_PCT_OVERRIDE":"100"}}'
}

function test_context_honors_a_smaller_output_token_reserve() {
  statusline_context '~80%/743k' '{"env":{"CLAUDE_AUTOCOMPACT_PCT_OVERRIDE":"75"}}' unset unset '8192'
}

# --- the gauge's color bands -------------------------------------------------

# The band a given context reading paints, asserted on the escape sequence
# itself rather than on the text, because the color IS the signal: the whole
# point of the gauge is that it can be read without reading it.
function statusline_color() {
  local expected="$1" used="$2"
  local fixture script actual
  fixture=$(mktemp -d "${TMPDIR:-/tmp}/statusline-color.XXXXXX")
  script="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/private_dot_claude/executable_statusline-command.sh"
  mkdir -p "$fixture/bin" "$fixture/home/.claude" "$fixture/tmp"
  printf '{}\n' >"$fixture/home/.claude/settings.json"
  printf '#!/bin/bash\nset -euo pipefail\nexit 1\n' >"$fixture/bin/git"
  printf '#!/bin/bash\nset -euo pipefail\nprintf "fixture-host\\n"\n' >"$fixture/bin/hostname"
  chmod 700 "$fixture/bin/git" "$fixture/bin/hostname"
  ln -s "$(command -v jq)" "$fixture/bin/jq"
  actual=$(printf '{"workspace":{"current_dir":"/fixture/project","project_dir":"/fixture/project"},"context_window":{"used_percentage":%s,"context_window_size":200000}}' "$used" |
    env -i "HOME=$fixture/home" "PATH=$fixture/bin:/usr/bin:/bin" LC_ALL=C \
      "TMPDIR=$fixture/tmp" "CLAUDE_CONFIG_DIR=$fixture/home/.claude" \
      GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
      /bin/bash "$script" 2>/dev/null)
  local band=calm
  case "$actual" in
    *'247;118;142'*) band=red ;;
    *'224;175;104'*) band=yellow ;;
  esac
  assert_same "$expected" "$band"
}

# The readings either side of each boundary. They are expressed as the share of
# the model window because that is what Claude Code reports; the gauge divides
# it by the compaction ceiling, so 71 per cent of the window is 85 per cent of
# the way to a compaction.
function test_the_gauge_stays_calm_below_three_fifths() {
  statusline_color calm 50
}

function test_the_gauge_turns_yellow_at_three_fifths() {
  statusline_color yellow 51
}

function test_the_gauge_is_still_yellow_just_below_the_red_band() {
  statusline_color yellow 70
}

function test_the_gauge_turns_red_at_eighty_five() {
  statusline_color red 71
}
