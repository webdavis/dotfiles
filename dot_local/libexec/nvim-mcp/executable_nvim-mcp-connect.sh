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
# (dot_config/nvim/lua/custom_api/pane_socket.lua), so a pane id IS a socket
# path and nothing is recorded anywhere. Nothing can go stale: Neovim removes
# its socket on exit, and a socket a crash left behind is one nobody answers
# on, which the probe below refuses.
#
#   1. NVIM_MCP_SOCKET set: that socket, if a Neovim answers on it. A pin is the
#      operator's explicit choice, so a dead pin is refused rather than quietly
#      replaced by discovery.
#   2. HERDR_PANE_ID set: this pane's socket, if a Neovim answers on it. The id
#      must be a name that fits in a socket path, and the colon herdr puts in
#      every id (`wW:p3K`) is written as a dot, because serverstart() reads any
#      address holding a colon as TCP (:help serverstart()). The Neovim side
#      spells the same rule.
#   3. Else the panes sharing this pane's TAB, from `herdr pane layout --pane`
#      with the id passed EXPLICITLY (`pane current` answers the CALLER's pane,
#      which is this one), each mapped to its pane socket the same way and kept
#      when a Neovim answers. One is connected to. Several are a PICKER, never a
#      guess: exit 4 and one line per candidate on stderr, which both harnesses
#      surface as server-startup text (spec 7.3, resolver row). None is a
#      refusal naming both remedies. herdr missing, failing, silent or hanging
#      means no siblings, never a crash; jq is not needed, the two fields read
#      here are lifted with grep.
#   4. Neither variable: nvim-mcp's own `--connect auto`.
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
# points, or a pane id that cannot name a socket), 4 the picker, 2 an
# environmental fault (nvim missing, or unable to say where its run dir is). On
# success this process is REPLACED by nvim-mcp. The pin lasts the server
# process: nvim-mcp connects once and keeps that client, so a Neovim that exits
# leaves later tool calls on a stale client until the harness starts a new
# session (spec 7.3, sticky selection).
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
# Seconds one nvim or herdr call may take. A knob only so the test can bound
# itself well inside its own one-second budget.
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

# fits <pane id> -- true when the id can name a socket.
fits() {
  [[ $1 =~ ^[A-Za-z0-9:-]{1,64}$ ]]
}

# pane_socket <pane id> -- its socket path under $root.
pane_socket() {
  printf '%s/herdr-pane-%s.sock' "$root" "${1//:/.}"
}

if [[ -n ${NVIM_MCP_SOCKET:-} ]]; then
  [[ -n "$(answers "$NVIM_MCP_SOCKET")" ]] ||
    die 3 "NVIM_MCP_SOCKET names $NVIM_MCP_SOCKET, and no Neovim answers there; unset it, or pin a running Neovim"
  exec "$server" --connect "$NVIM_MCP_SOCKET"
fi

if [[ -n ${HERDR_PANE_ID:-} ]]; then
  fits "$HERDR_PANE_ID" ||
    die 3 "HERDR_PANE_ID is '$HERDR_PANE_ID', which cannot name a socket; export NVIM_MCP_SOCKET instead"
  root="${XDG_RUNTIME_DIR:-}"
  if [[ -z $root ]]; then
    run_dir="$(bounded nvim --headless --clean -c 'lua io.write(vim.fn.stdpath("run"))' -c 'qa!')"
    [[ $run_dir == /* ]] || die 2 'nvim did not report its run dir (stdpath("run")), so there is no root to look in'
    root="$(dirname "$run_dir")"
  fi
  sock="$(pane_socket "$HERDR_PANE_ID")"
  if [[ -n "$(answers "$sock")" ]]; then
    exec "$server" --connect "$sock"
  fi

  # Candidates accumulate as "<socket> <pane id> <pid>" lines. Only the pane
  # ids herdr 0.8.2 puts under .result.layout.panes are read, by name, so a
  # herdr that is missing, failing or silent contributes nothing and needs no
  # guard of its own: `bounded` swallows its exit status and its stderr. An id
  # that cannot name a socket is skipped, because one carrying a slash would
  # derive a path OUTSIDE the run root.
  candidates=""
  layout="$(bounded herdr pane layout --pane "$HERDR_PANE_ID")"
  while IFS= read -r pane; do
    if [[ $pane == "$HERDR_PANE_ID" ]] || ! fits "$pane"; then
      continue
    fi
    sibling="$(pane_socket "$pane")"
    pid="$(answers "$sibling")"
    [[ -n $pid ]] && candidates+="$sibling $pane $pid"$'\n'
  done < <(printf '%s' "$layout" | grep -o '"pane_id":"[^"]*"' | cut -d'"' -f4)

  count="$(printf '%s' "$candidates" | grep -c . || true)"
  case "$count" in
    0)
      die 3 "no Neovim answers for pane $HERDR_PANE_ID at $sock, nor for any pane sharing its tab; start Neovim in this tab, launch the agent from Neovim (<leader>Cc), or export NVIM_MCP_SOCKET"
      ;;
    1)
      read -r sibling _ <<<"$candidates"
      exec "$server" --connect "$sibling"
      ;;
  esac
  {
    printf 'nvim-mcp-connect: %s Neovims share the tab of pane %s, so it will not guess.\n' "$count" "$HERDR_PANE_ID"
    printf 'Re-run with NVIM_MCP_SOCKET set to one of:\n'
    while read -r sibling pane pid; do
      [[ -n ${sibling:-} ]] && printf '  %s  pane %s  pid %s\n' "$sibling" "$pane" "$pid"
    done <<<"$candidates"
  } >&2
  exit 4
fi

exec "$server" --connect auto
