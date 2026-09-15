#!/usr/bin/env bash
# What the resolver does when the workspace offers no single answer: two live
# siblings, none at all, one in another tab, and a root whose name holds a
# space. Every case ends in a refusal or a picker, so the question each asks is
# the same one: what did it refuse, and did it connect to anything anyway.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit. The fixture installs its own work directory and EXIT trap.

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

function test_two_live_siblings_are_a_picker_not_a_guess() {
  setup_case two-siblings
  me term_a
  siblings 'w1:t1|term_a|w1:p1' 'w1:t1|term_b|w1:p2' 'w1:t1|term_c|w1:p3'
  live "$(sock term_b)" "$(sock term_c)"
  run_case XDG_RUNTIME_DIR="$RUN"
  assert_same 4 "$RC"
  assert_file_contains "$CASE/err" "$(sock term_b)  pane w1:p2  pid 4242"
  assert_file_contains "$CASE/err" "$(sock term_c)  pane w1:p3  pid 4242"
  assert_file_contains "$CASE/err" NVIM_MCP_SOCKET
  assert_empty "$(connected_socket)"
}

function test_no_live_sibling_refuses_after_probing() {
  setup_case no-sibling
  me term_a
  siblings 'w1:t1|term_a|w1:p1' 'w1:t1|term_b|w1:p2'
  make_socket "$(sock term_b)"
  run_case XDG_RUNTIME_DIR="$RUN"
  assert_same 3 "$RC"
  assert_contains "$(sock term_b)" "$(probes)"
  assert_empty "$(connected_socket)"
}

function test_another_tab_of_the_workspace_is_not_a_candidate() {
  setup_case other-tab
  me term_a
  siblings 'w1:t1|term_a|w1:p1' 'w1:t2|term_c|w1:p5'
  live "$(sock term_c)"
  run_case XDG_RUNTIME_DIR="$RUN"
  assert_same 3 "$RC"
  assert_not_contains "$(sock term_c)" "$(probes)"
  assert_empty "$(connected_socket)"
}

# Candidates are carried in arrays, not one space-delimited string: a socket
# under "run root" must reach the picker whole, never cut at the space.
function test_a_run_root_with_a_space_reaches_the_picker_whole() {
  setup_case spaced-root
  RUN="$CASE/run root"
  mkdir "$RUN"
  chmod 700 "$RUN"
  me term_a
  siblings 'w1:t1|term_a|w1:p1' 'w1:t1|term_b|w1:p2' 'w1:t1|term_c|w1:p3'
  live "$(sock term_b)" "$(sock term_c)"
  run_case XDG_RUNTIME_DIR="$RUN"
  assert_same 4 "$RC"
  assert_file_contains "$CASE/err" "  $(sock term_b)  pane w1:p2  pid 4242"
  assert_file_contains "$CASE/err" "  $(sock term_c)  pane w1:p3  pid 4242"
  assert_empty "$(connected_socket)"
}
