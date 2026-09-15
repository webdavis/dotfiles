#!/usr/bin/env bash
# Everything the resolver refuses about the run root itself, before any socket
# under it is worth looking at: no root reported, a root this user does not hold
# privately, and a root deep enough that the socket name no longer fits
# sun_path. Each case also asserts that nothing was probed and nothing was run,
# because a refusal that still touched the filesystem is not a refusal.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit.

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

# An empty XDG_RUNTIME_DIR is one way to get there: Neovim then reports an
# empty stdpath("run") and starts no server at all.
function test_an_unusable_run_dir_report_is_refused() {
  setup_case no-run-dir
  me term_a
  : >"$CASE/rundir"
  run_case
  assert_same 2 "$RC"
  assert_file_contains "$CASE/err" 'run dir'
  assert_empty "$(probes)"
  assert_empty "$(connected_socket)"
}

# Neovim falls back to <temp>/nvim.<random> when nvim.<user> is mis-owned, and
# <temp> can be a shared /tmp: a socket there is one any account can pre-create.
function test_a_root_that_is_not_private_is_refused() {
  setup_case loose-root
  me term_a
  live "$(sock term_a)"
  chmod 755 "$RUN"
  run_case XDG_RUNTIME_DIR="$RUN"
  assert_same 2 "$RC"
  assert_file_contains "$CASE/err" 0700
  assert_file_contains "$CASE/err" "$RUN"
  assert_empty "$(probes)"
  assert_empty "$(connected_socket)"
}

# sun_path is 104 bytes on macOS (108 on Linux), NUL included. A root deep
# enough pushes the name past it; the bind would fail with a bare "invalid
# argument" on the editor's side and a probe here would find nothing, so the
# resolver says what happened instead of refusing as if no Neovim existed.
function test_a_socket_name_over_sun_path_is_refused_with_its_length() {
  setup_case long-root
  RUN="$CASE/run/$(printf 'x%.0s' {1..60})"
  mkdir -p "$RUN"
  chmod 700 "$RUN"
  me term_a
  run_case XDG_RUNTIME_DIR="$RUN"
  assert_same 5 "$RC"
  assert_file_contains "$CASE/err" "$(printf '%s' "$(sock term_a)" | wc -c | tr -d ' ') bytes"
  assert_file_contains "$CASE/err" 'allow 103'
  assert_empty "$(probes)"
  assert_empty "$(connected_socket)"
}

# UTF-8 characters consume more bytes than Bash's character count. The path
# fits by characters but exceeds sun_path in bytes, and must be refused first.
function test_the_length_is_counted_in_bytes_not_characters() {
  setup_case utf8-root
  RUN="$CASE/run/$(printf 'é%.0s' {1..35})"
  mkdir -p "$RUN"
  chmod 700 "$RUN"
  me term_a
  run_case LC_ALL=en_US.UTF-8 XDG_RUNTIME_DIR="$RUN"
  assert_same 5 "$RC"
  assert_file_contains "$CASE/err" "$(printf '%s' "$(sock term_a)" | wc -c | tr -d ' ') bytes"
  assert_empty "$(probes)"
  assert_empty "$(connected_socket)"
}
