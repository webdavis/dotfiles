#!/usr/bin/env bash
# realpath alone hides an intermediate link target, so both the spelling the
# caller gave and every target it resolves through have to be checked. One case
# puts the loose directory in the spelling, the other puts it in the target.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit.

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

function test_a_link_under_a_loose_parent_is_refused() {
  setup_case alias-parent
  mkdir -p "$CASE/open" "$CASE/target/run"
  chmod 777 "$CASE/open"
  chmod 700 "$CASE/target/run"
  ln -s "$CASE/target" "$CASE/open/alias"
  RUN="$CASE/open/alias/run"
  me term_a
  live "$(sock term_a)"
  run_case XDG_RUNTIME_DIR="$RUN"
  assert_same 2 "$RC"
  assert_empty "$(probes)"
  assert_empty "$(connected_socket)"
}

function test_a_link_onto_a_loose_target_is_refused() {
  setup_case alias-target
  mkdir -p "$CASE/open/run" "$CASE/aliases"
  chmod 777 "$CASE/open"
  chmod 700 "$CASE/open/run"
  ln -s "$CASE/open" "$CASE/aliases/alias"
  RUN="$CASE/aliases/alias/run"
  me term_a
  live "$(sock term_a)"
  run_case XDG_RUNTIME_DIR="$RUN"
  assert_same 2 "$RC"
  assert_empty "$(probes)"
  assert_empty "$(connected_socket)"
}
