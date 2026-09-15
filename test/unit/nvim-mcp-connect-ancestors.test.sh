#!/usr/bin/env bash
# A run root is only as private as the directories above it: an ancestor any
# other account can replace lets that account swap the root itself and pre-place
# a socket inside it. These cases walk the two shapes that matter, a loose
# ancestor and a protected one, so the check cannot be satisfied by looking at
# the leaf alone.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit.

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

# open_ancestor <mode> -- a private root under an ancestor at <mode>.
open_ancestor() {
  setup_case "ancestor-$1"
  RUN="$CASE/open/inner/run"
  mkdir -p "$RUN"
  chmod 700 "$RUN" "$CASE/open/inner"
  chmod "$1" "$CASE/open"
  me term_a
  live "$(sock term_a)"
  run_case XDG_RUNTIME_DIR="$RUN"
}

function test_a_world_writable_ancestor_is_refused() {
  open_ancestor 777
  assert_same 2 "$RC"
  assert_empty "$(probes)"
  assert_empty "$(connected_socket)"
}

function test_a_group_writable_ancestor_is_refused() {
  open_ancestor 770
  assert_same 2 "$RC"
  assert_empty "$(probes)"
  assert_empty "$(connected_socket)"
}

# A sticky directory only lets a child be replaced by its own owner or root,
# which is what makes a shared /tmp usable as an ancestor.
function test_a_sticky_ancestor_is_protection_enough() {
  setup_case sticky-parent
  mkdir -p "$CASE/sticky/run"
  chmod 1777 "$CASE/sticky"
  chmod 700 "$CASE/sticky/run"
  RUN="$CASE/sticky/run"
  me term_a
  live "$(sock term_a)"
  run_case XDG_RUNTIME_DIR="$RUN"
  assert_same 0 "$RC"
  assert_same "$(sock term_a)" "$(connected_socket)"
}

# Expanding a safe alias such as /var can push the name past sun_path, so the
# spelling the caller gave is the one that must survive.
function test_a_protected_alias_keeps_the_socket_spelling() {
  setup_case safe-alias
  mkdir "$CASE/aliases"
  ln -s "$CASE" "$CASE/aliases/alias"
  RUN="$CASE/aliases/alias/run"
  me term_a
  live "$(sock term_a)"
  run_case XDG_RUNTIME_DIR="$RUN"
  assert_same 0 "$RC"
  assert_same "$(sock term_a)" "$(connected_socket)"
}
