#!/usr/bin/env bash
#
# nvim-mcp-connect.sh -- answer "which Neovim socket should this agent talk to",
# then exec nvim-mcp against it. This is the command both harnesses register as
# the Neovim MCP server, never nvim-mcp itself, whose own `--connect auto` picks
# an instance by current directory or git root and so cannot tell two Neovim
# panes in one herdr workspace apart (docs/research/2026-09-nvim-mcp-evaluation.md,
# criterion 3).
#
# Discovery is by construction, not by lookup (spec 7.3). A Neovim started in a
# herdr pane listens on <run root>/herdr-pane-<pane id>.sock
# (dot_config/nvim/lua/custom_api/pane_socket.lua), so the Neovim an agent
# means is the one whose pane id the agent's own environment carries, and the
# path is DERIVED rather than recorded. Nothing is written down, so nothing can
# go stale: Neovim removes its socket on exit, and a socket a crash left behind
# is one nobody answers on, which the probe below refuses.
#
#   1. NVIM_MCP_SOCKET set: that socket, if a Neovim answers on it. A pin is the
#      operator's explicit choice, so a dead pin is refused rather than quietly
#      replaced by discovery.
#   2. HERDR_PANE_ID set: the pane socket, if a Neovim answers on it. The id
#      must be a name that fits in a socket path, and the colon herdr puts in
#      every id (`wW:p3K`) is written as a dot, because serverstart() reads any
#      address holding a colon as TCP (:help serverstart()). The Neovim side
#      spells the same rule.
#   3. Neither: nvim-mcp's own `--connect auto`.
#
# The run root is the directory both sides derive the same way: XDG_RUNTIME_DIR
# when set, else the PARENT of stdpath("run"). That parent, not stdpath("run")
# itself, because on 0.12 with XDG_RUNTIME_DIR unset the run dir is per process
# ($TMPDIR/nvim.<user>/<random>) and no other process can compute it; the parent
# is the 0700 directory Neovim creates and checks for itself. It is asked from
# Neovim rather than recomputed, so TMPDIR handling and the user name stay
# Neovim's.
#
# Exit codes: 3 a refusal (nothing answers where the pin or the pane points, or
# a pane id that cannot name a socket), 2 an environmental fault (nvim missing,
# or unable to say where its run dir is). On success this process is REPLACED
# by nvim-mcp. The pin lasts the server process: nvim-mcp connects once and
# keeps that client, so a Neovim that exits leaves later tool calls on a stale
# client until the harness starts a new session (spec 7.3, sticky selection).
set -euo pipefail

# die <exit code> <message...>
die() {
  local code="$1"
  shift
  printf 'nvim-mcp-connect: %s\n' "$*" >&2
  exit "$code"
}

command -v nvim >/dev/null 2>&1 || die 2 'nvim is not on PATH, and the resolver needs it'

server="${NVIM_MCP_BIN:-$HOME/.local/libexec/nvim-mcp/nvim-mcp}"
# Seconds one nvim call may take. A knob only so the test can bound itself well
# inside its own one-second budget.
deadline="${NVIM_MCP_PROBE_DEADLINE:-2}"
[[ $deadline =~ ^[0-9]+(\.[0-9]+)?$ ]] || die 2 'NVIM_MCP_PROBE_DEADLINE must be seconds'

# bounded <command...> -- the command's stdout, cut off at the deadline. Stock
# macOS ships no timeout(1) and bash has no wait-with-deadline, so a child that
# TERMs the job is the portable stand-in; a prompt answer pays nothing for it.
bounded() {
  local job watchdog
  "$@" 2>/dev/null &
  job=$!
  { sleep "$deadline" && kill -TERM "$job"; } </dev/null >/dev/null 2>&1 &
  watchdog=$!
  wait "$job" 2>/dev/null || true
  kill -TERM "$watchdog" 2>/dev/null || true
}

# answers <socket> -- true when a Neovim answers on it. The reply is taken
# WHOLE (the x sentinel keeps trailing newlines command substitution would
# drop), so only the exact answer to the expression `1` passes.
answers() {
  local reply
  [[ -S $1 ]] || return 1
  reply="$(
    bounded nvim --server "$1" --remote-expr 1
    printf x
  )"
  [[ ${reply%x} == 1 ]]
}

if [[ -n ${NVIM_MCP_SOCKET:-} ]]; then
  answers "$NVIM_MCP_SOCKET" ||
    die 3 "NVIM_MCP_SOCKET names $NVIM_MCP_SOCKET, and no Neovim answers there; unset it, or pin a running Neovim"
  exec "$server" --connect "$NVIM_MCP_SOCKET"
fi

if [[ -n ${HERDR_PANE_ID:-} ]]; then
  [[ $HERDR_PANE_ID =~ ^[A-Za-z0-9:-]{1,64}$ ]] ||
    die 3 "HERDR_PANE_ID is '$HERDR_PANE_ID', which cannot name a socket; export NVIM_MCP_SOCKET instead"
  root="${XDG_RUNTIME_DIR:-}"
  if [[ -z $root ]]; then
    run_dir="$(bounded nvim --headless --clean -c 'lua io.write(vim.fn.stdpath("run"))' -c 'qa!')"
    [[ $run_dir == /* ]] || die 2 'nvim did not report its run dir (stdpath("run")), so there is no root to look in'
    root="$(dirname "$run_dir")"
  fi
  sock="$root/herdr-pane-${HERDR_PANE_ID//:/.}.sock"
  answers "$sock" ||
    die 3 "no Neovim answers for pane $HERDR_PANE_ID at $sock; start Neovim in this pane, launch the agent from Neovim (<leader>Cc), or export NVIM_MCP_SOCKET"
  exec "$server" --connect "$sock"
fi

exec "$server" --connect auto
