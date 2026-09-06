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
# herdr pane listens on <run root>/herdr-<session>-<terminal>.sock
# (dot_config/nvim/lua/custom_api/pane_socket.lua): the session is the first six
# hex digits of the sha256 of HERDR_SOCKET_PATH, the terminal is herdr's own id
# for the pane's terminal. Both are asked from herdr or the environment on both
# sides, so a name is DERIVED and nothing is recorded anywhere. Nothing can go
# stale: Neovim removes its socket on exit, and a socket a crash left behind is
# one nobody answers on, which the probe below refuses.
#
#   1. NVIM_MCP_SOCKET set: that socket, if a Neovim answers on it. A pin is the
#      operator's explicit choice, so a dead pin is refused rather than quietly
#      replaced by discovery.
#   2. The pane this process runs in, from `herdr pane current --current`, which
#      answers for the CALLER's terminal by process rather than by environment
#      (measured on 0.8.2 from a cleared, piped environment, the shape an MCP
#      child has). Its terminal id names this pane's socket, used if a Neovim
#      answers on it. Never HERDR_PANE_ID: that is the launch-time id, and a
#      pane moved to another workspace keeps it while herdr renames the pane.
#   3. Else the panes sharing this pane's TAB, from `herdr pane list --workspace`
#      filtered to the tab herdr reported for this pane, each named the same way
#      and kept when a Neovim answers. One is connected to. Several are a
#      PICKER, never a guess: exit 4 and one line per candidate on stderr, which
#      both harnesses surface as server-startup text (spec 7.3, resolver row).
#      None is a refusal naming both remedies.
#   4. herdr answering nothing: inside herdr (HERDR_ENV set) that is a refusal,
#      since the pane cannot be named; outside it, nvim-mcp's own `--connect
#      auto`.
#
# The run root is the directory both sides derive the same way: XDG_RUNTIME_DIR
# when set, else the PARENT of stdpath("run"). That parent, not stdpath("run")
# itself, because on 0.12 with XDG_RUNTIME_DIR unset the run dir is per process
# ($TMPDIR/nvim.<user>/<random>) and no other process can compute it; the parent
# is the 0700 directory Neovim creates and checks for itself. It is asked from
# Neovim rather than recomputed, so TMPDIR handling and the user name stay
# Neovim's.
#
# Exit codes: 3 a refusal (nothing answers where the pin, the pane or its tab
# points, or herdr cannot name the pane), 4 the picker, 2 an environmental
# fault (a tool missing, nvim unable to say where its run dir is, or a run root
# that is not this user's private directory). On success
# this process is REPLACED by nvim-mcp. The pin lasts the server process:
# nvim-mcp connects once and keeps that client, so a Neovim that exits leaves
# later tool calls on a stale client until the harness starts a new session
# (spec 7.3, sticky selection).
set -euo pipefail

# die <exit code> <message...>
die() {
  local code="$1"
  shift
  printf 'nvim-mcp-connect: %s\n' "$*" >&2
  exit "$code"
}

# All three hard dependencies, checked FIRST: otherwise a missing one surfaces
# as whatever fails next, which reads as a herdr fault and sends the operator to
# debug the wrong thing. herdr itself is optional and is handled below.
for required in nvim jq shasum; do
  command -v "$required" >/dev/null 2>&1 || die 2 "$required is not on PATH, and the resolver needs it"
done

server="${NVIM_MCP_BIN:-$HOME/.local/libexec/nvim-mcp/nvim-mcp}"
# Seconds one nvim or herdr call may take. A knob only so the test can bound
# itself well inside its own one-second budget.
deadline="${NVIM_MCP_PROBE_DEADLINE:-2}"
[[ $deadline =~ ^[0-9]+(\.[0-9]+)?$ ]] || die 2 'NVIM_MCP_PROBE_DEADLINE must be seconds'

# bounded <command...> -- the command's stdout, cut off at the deadline. Stock
# macOS ships no timeout(1) and bash has no wait-with-deadline, so a child that
# TERMs the job is the portable stand-in; a prompt answer pays nothing for it. A
# command that is not installed contributes nothing, the same as one that fails.
bounded() {
  local job watchdog
  "$@" 2>/dev/null &
  job=$!
  { sleep "$deadline" && kill -TERM "$job"; } </dev/null >/dev/null 2>&1 &
  watchdog=$!
  wait "$job" 2>/dev/null || true
  kill -TERM "$watchdog" 2>/dev/null || true
}

# answers <socket> -- the pid of the Neovim answering on it, nothing otherwise.
# The reply is taken WHOLE (the x sentinel keeps trailing newlines command
# substitution would drop), so only a bare decimal pid passes.
answers() {
  local reply
  [[ -S $1 ]] || return 0
  reply="$(
    bounded nvim --server "$1" --remote-expr 'getpid()'
    printf x
  )"
  reply="${reply%x}"
  [[ $reply =~ ^[0-9]+$ ]] && printf '%s' "$reply" || true
}

# fits <terminal id> -- true when the id can name a socket. An id carrying a
# slash could derive a path OUTSIDE the run root.
fits() {
  [[ $1 =~ ^[A-Za-z0-9_-]{1,64}$ ]]
}

# root_fault <dir> -- why <dir> is not a directory this user owns at mode 0700,
# empty when it is. Neovim itself falls back to `<temp>/nvim.<random>` when
# `nvim.<user>` is mis-owned or too open (measured on 0.12.5), and that
# directory's parent is `<temp>` itself, which can be a shared /tmp where any
# account may pre-create a pane socket. A supplied XDG_RUNTIME_DIR gets the same
# check. BSD stat first, GNU second.
root_fault() {
  local meta
  [[ -d $1 ]] || {
    printf 'is not a directory'
    return
  }
  meta="$(stat -f '%u %Lp' "$1" 2>/dev/null || stat -c '%u %a' "$1" 2>/dev/null || true)"
  [[ $meta == "$(id -u) 700" ]] ||
    printf 'is owned by uid %s at mode %s, not by this user at 0700' "${meta% *}" "${meta#* }"
}

# pane_socket <terminal id> -- its socket path under $root for this session.
pane_socket() {
  printf '%s/herdr-%s-%s.sock' "$root" "$session" "$1"
}

if [[ -n ${NVIM_MCP_SOCKET:-} ]]; then
  [[ -n "$(answers "$NVIM_MCP_SOCKET")" ]] ||
    die 3 "NVIM_MCP_SOCKET names $NVIM_MCP_SOCKET, and no Neovim answers there; unset it, or pin a running Neovim"
  exec "$server" --connect "$NVIM_MCP_SOCKET"
fi

me="$(bounded herdr pane current --current)"
terminal="$(jq -r '.result.pane.terminal_id // empty' <<<"$me" 2>/dev/null || true)"
tab="$(jq -r '.result.pane.tab_id // empty' <<<"$me" 2>/dev/null || true)"
workspace="$(jq -r '.result.pane.workspace_id // empty' <<<"$me" 2>/dev/null || true)"
if [[ -z $terminal ]]; then
  [[ -z ${HERDR_ENV:-} ]] || die 3 'herdr did not report which pane this is, so no Neovim can be named for it; export NVIM_MCP_SOCKET to pin one'
  exec "$server" --connect auto
fi
fits "$terminal" || die 3 "herdr reports terminal '$terminal', which cannot name a socket; export NVIM_MCP_SOCKET instead"

root="${XDG_RUNTIME_DIR:-}"
if [[ -z $root ]]; then
  run_dir="$(bounded nvim --headless --clean -c 'lua io.write(vim.fn.stdpath("run"))' -c 'qa!')"
  [[ $run_dir == /* ]] || die 2 'nvim did not report its run dir (stdpath("run")), so there is no root to look in'
  root="$(dirname "$run_dir")"
fi
fault="$(root_fault "$root")"
[[ -z $fault ]] || die 2 "the run root $root $fault, so no socket there can be trusted"
session="$(printf '%s' "${HERDR_SOCKET_PATH:-}" | shasum -a 256 | cut -c1-6)"

own="$(pane_socket "$terminal")"
if [[ -n "$(answers "$own")" ]]; then
  exec "$server" --connect "$own"
fi

# Candidates in three ARRAYS, never one delimited string: a run root may carry
# a space. A herdr that fails or answers something else contributes nothing.
sockets=()
panes=()
pids=()
while read -r sibling pane; do
  fits "$sibling" || continue
  candidate="$(pane_socket "$sibling")"
  pid="$(answers "$candidate")"
  [[ -n $pid ]] || continue
  sockets+=("$candidate")
  panes+=("$pane")
  pids+=("$pid")
done < <(bounded herdr pane list --workspace "$workspace" |
  jq -r --arg tab "$tab" --arg me "$terminal" \
    '.result.panes[]? | select(.tab_id == $tab and .terminal_id != $me) | "\(.terminal_id) \(.pane_id)"' 2>/dev/null || true)

case "${#sockets[@]}" in
  0)
    die 3 "no Neovim answers for this pane at $own, nor for any pane sharing tab $tab; start Neovim in this tab, launch the agent from Neovim (<leader>Cc), or export NVIM_MCP_SOCKET"
    ;;
  1)
    exec "$server" --connect "${sockets[0]}"
    ;;
esac
{
  printf 'nvim-mcp-connect: %s Neovims share tab %s with this pane, so it will not guess.\n' "${#sockets[@]}" "$tab"
  printf 'Re-run with NVIM_MCP_SOCKET set to one of:\n'
  for index in "${!sockets[@]}"; do
    printf '  %s  pane %s  pid %s\n' "${sockets[index]}" "${panes[index]}" "${pids[index]}"
  done
} >&2
exit 4
