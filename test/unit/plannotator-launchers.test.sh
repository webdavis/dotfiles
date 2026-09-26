#!/usr/bin/env bash
set -euo pipefail

function set_up() {
  PLANNOTATOR_FIXTURE="$(mktemp -d)"
  PLANNOTATOR_REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  mkdir -p "$PLANNOTATOR_FIXTURE/bin" "$PLANNOTATOR_FIXTURE/annotate plugin/bin"
  cat >"$PLANNOTATOR_FIXTURE/annotate plugin/bin/plannotator-tui.exe" <<'STUB'
#!/bin/bash
set -euo pipefail
printf '<%s>\n' "$@" >"$PLANNOTATOR_FIXTURE/argv"
STUB
  cat >"$PLANNOTATOR_FIXTURE/bin/herdr" <<'STUB'
#!/bin/bash
set -euo pipefail
cat "$PLANNOTATOR_FIXTURE/plugins.json"
STUB
  chmod +x "$PLANNOTATOR_FIXTURE/bin/herdr" \
    "$PLANNOTATOR_FIXTURE/annotate plugin/bin/plannotator-tui.exe"
}

function invoke_plannotator_glue() {
  local subject="$1"
  shift
  env PLANNOTATOR_FIXTURE="$PLANNOTATOR_FIXTURE" PATH="$PLANNOTATOR_FIXTURE/bin:$PATH" \
    /bin/bash "$PLANNOTATOR_REPO/$subject" "$@"
}

function test_plannotator_launcher_selects_enabled_annotate_and_preserves_spaced_arguments() {
  local status=0
  jq -n --arg root "$PLANNOTATOR_FIXTURE/annotate plugin" \
    '{result:{plugins:[
      {plugin_id:"other",enabled:true,plugin_root:"/wrong/plugin"},
      {plugin_id:"annotate",enabled:false,plugin_root:"/disabled/plugin"},
      {plugin_id:"annotate",enabled:true,plugin_root:$root}
    ]}}' >"$PLANNOTATOR_FIXTURE/plugins.json"
  invoke_plannotator_glue dot_local/bin/executable_plannotator-tui \
    'plan with spaces.md' '--editor=some command' '' </dev/null || status=$?
  assert_same 0 "$status"
  assert_same $'<plan with spaces.md>\n<--editor=some command>\n<>' "$(cat "$PLANNOTATOR_FIXTURE/argv")"
}

function test_plannotator_launcher_fails_when_annotate_is_absent_or_disabled() {
  local plugin_id status
  for plugin_id in other annotate; do
    jq -n --arg id "$plugin_id" --arg root "$PLANNOTATOR_FIXTURE/annotate plugin" \
      '{result:{plugins:[{plugin_id:$id,enabled:($id=="other"),plugin_root:$root}]}}' \
      >"$PLANNOTATOR_FIXTURE/plugins.json"
    status=0
    invoke_plannotator_glue dot_local/bin/executable_plannotator-tui </dev/null 2>/dev/null || status=$?
    assert_not_same 0 "$status"
    assert_file_not_exists "$PLANNOTATOR_FIXTURE/argv"
  done
}
