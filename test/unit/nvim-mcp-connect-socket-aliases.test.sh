#!/usr/bin/env bash
# A socket that is a symlink is a socket some other account may be pointing
# somewhere else, so the resolver refuses it before the probe rather than after.
# One case per place a candidate comes from: the pin, the caller's own pane, and
# a sibling. The last two must lose only the alias and still find the safe one.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit.

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

function test_a_pinned_socket_symlink_is_refused_unprobed() {
  setup_case pinned-alias
  live "$CASE/public.sock"
  ln -s "$CASE/public.sock" "$RUN/pin.sock"
  printf '%s\n' "$RUN/pin.sock" >>"$CASE/live"
  run_case NVIM_MCP_SOCKET="$RUN/pin.sock"
  assert_same 3 "$RC"
  assert_empty "$(probes)"
  assert_empty "$(herdr_calls)"
  assert_empty "$(connected_socket)"
}

function test_an_own_pane_socket_symlink_loses_to_the_safe_sibling() {
  setup_case pane-alias
  me term_a
  siblings 'w1:t1|term_b|w1:p2'
  live "$CASE/public.sock" "$(sock term_b)"
  ln -s "$CASE/public.sock" "$(sock term_a)"
  sock term_a >>"$CASE/live"
  run_case XDG_RUNTIME_DIR="$RUN"
  assert_same 0 "$RC"
  assert_same "$(sock term_b)" "$(connected_socket)"
  assert_not_contains "$(sock term_a)" "$(probes)"
}

function test_a_sibling_socket_symlink_is_not_a_candidate() {
  setup_case sibling-alias
  me term_a
  siblings 'w1:t1|term_b|w1:p2' 'w1:t1|term_c|w1:p3'
  live "$CASE/public.sock" "$(sock term_c)"
  ln -s "$CASE/public.sock" "$(sock term_b)"
  sock term_b >>"$CASE/live"
  run_case XDG_RUNTIME_DIR="$RUN"
  assert_same 0 "$RC"
  assert_same "$(sock term_c)" "$(connected_socket)"
  assert_not_contains "$(sock term_b)" "$(probes)"
}
