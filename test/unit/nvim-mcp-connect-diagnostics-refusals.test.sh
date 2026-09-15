#!/usr/bin/env bash
# --diagnose prints the target it WOULD use and starts nothing, so every
# refusal it reports has to reach stderr with the same reason the connecting
# path gives, leave stdout empty, and exec nothing. One case per refusal the
# operator is most likely to be staring at.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit.

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

function test_diagnose_reports_an_ambiguous_workspace_as_a_picker() {
  setup_case diagnose-ambiguous
  CASE_OPTION=--diagnose
  me term_a
  siblings 'w1:t1|term_b|w1:p2' 'w1:t1|term_c|w1:p3'
  live "$(sock term_b)" "$(sock term_c)"
  run_case XDG_RUNTIME_DIR="$RUN"
  assert_same 4 "$RC"
  assert_is_file_empty "$CASE/out"
  assert_empty "$(connected_socket)"
  assert_file_contains "$CASE/err" "$(sock term_b)  pane w1:p2  pid 4242"
  assert_file_contains "$CASE/err" "$(sock term_c)  pane w1:p3  pid 4242"
  assert_file_contains "$CASE/err" NVIM_MCP_SOCKET
}

function test_diagnose_reports_a_pin_nothing_answers_on() {
  setup_case diagnose-dead-pin
  CASE_OPTION=--diagnose
  make_socket "$RUN/dead.sock"
  run_case NVIM_MCP_SOCKET="$RUN/dead.sock"
  assert_same 3 "$RC"
  assert_is_file_empty "$CASE/out"
  assert_empty "$(connected_socket)"
  assert_file_contains "$CASE/err" 'no Neovim answers'
}

function test_diagnose_reports_a_root_that_is_not_private() {
  setup_case diagnose-root
  CASE_OPTION=--diagnose
  me term_a
  chmod 755 "$RUN"
  run_case XDG_RUNTIME_DIR="$RUN"
  assert_same 2 "$RC"
  assert_is_file_empty "$CASE/out"
  assert_empty "$(connected_socket)"
  assert_file_contains "$CASE/err" 0700
}

function test_diagnose_reports_a_socket_name_over_sun_path() {
  setup_case diagnose-length
  CASE_OPTION=--diagnose
  me term_a
  RUN="$RUN/$(printf '%090d' 0)"
  mkdir "$RUN"
  chmod 700 "$RUN"
  run_case XDG_RUNTIME_DIR="$RUN"
  assert_same 5 "$RC"
  assert_is_file_empty "$CASE/out"
  assert_empty "$(connected_socket)"
  assert_file_contains "$CASE/err" 'unix sockets allow 103'
}
