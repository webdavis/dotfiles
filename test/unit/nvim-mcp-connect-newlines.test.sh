#!/usr/bin/env bash
# A newline in a path splits the report the resolver reads and the lists it
# keeps, so every place a path arrives refuses one rather than working on half
# of it: the runtime root, the pin, and the run dir nvim reports.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit.

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

function test_a_multiline_runtime_root_is_refused() {
  setup_case newline-root
  RUN="$CASE/line"$'\n'"break"
  mkdir "$RUN"
  chmod 700 "$RUN"
  me term_a
  live "$(sock term_a)"
  run_case XDG_RUNTIME_DIR="$RUN"
  assert_same 2 "$RC"
  assert_empty "$(probes)"
  assert_empty "$(connected_socket)"
}

# The pin is refused outright rather than falling through to discovery, which
# would connect somewhere the operator did not name.
function test_a_multiline_pin_is_refused_without_discovery() {
  setup_case newline-pin
  local pin="$RUN/pin"$'\n'"socket"
  live "$pin"
  run_case NVIM_MCP_SOCKET="$pin"
  assert_same 3 "$RC"
  assert_empty "$(probes)"
  assert_empty "$(herdr_calls)"
  assert_empty "$(connected_socket)"
}

function test_a_multiline_run_report_is_refused() {
  setup_case newline-fallback
  me term_a
  printf '%s/line\nbreak/instance' "$CASE" >"$CASE/rundir"
  run_case
  assert_same 2 "$RC"
  assert_empty "$(probes)"
  assert_empty "$(connected_socket)"
}
