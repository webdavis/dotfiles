#!/usr/bin/env bash
#
# nvim-mcp-connect.sh -- answer "which Neovim socket should this agent talk to",
# then exec nvim-mcp against it. This is the command both harnesses register as
# the Neovim MCP server, never nvim-mcp itself, which picks an instance by
# current directory or git root and so cannot tell two Neovim panes in one herdr
# workspace apart (docs/research/2026-09-nvim-mcp-evaluation.md, criterion 3).
#
# The five steps of spec 7.3, in order:
#
#   1. NVIM_MCP_SOCKET wins. Whichever side spawned the other wrote the address
#      down, so the common case is created rather than inferred.
#   2. Topology. `herdr pane layout --pane <id>` gives the caller's tab and its
#      sibling panes. The pane id is passed EXPLICITLY: `herdr pane current`
#      answers the CALLER's pane, so a resolver that asks it matches nothing.
#   3. Identity, not presence. A socket that answers proves only that SOMETHING
#      is there, and Neovim 0.12.5 binds a dead instance's path without
#      complaint (measured), so each candidate is asked over RPC for its own
#      pane id and pid and both are compared. A mismatch prunes the entry.
#   4. Two verified candidates is a PICKER, never a guess: a guess edits the
#      wrong buffer. The enumeration goes to stderr, where the harness shows it.
#   5. Zero is a refusal naming its reason.
#
# Exit codes: 3 a refusal, 4 the picker, 2 an environmental fault (a missing
# tool, a broken herdr answer). On success this process is REPLACED by
# nvim-mcp, so there is no fourth outcome. There is no memo file: the harness
# starts this once per session and the exec keeps that instance for the
# session's life, which is the sticky selection spec 7.3 asks for.
set -euo pipefail

# Both hard dependencies, checked FIRST, before herdr, the registry or any
# socket: otherwise the missing tool surfaces as whatever fails next, which
# reads as a herdr fault and sends the operator to debug the wrong thing.
for required in nvim jq; do
  if ! command -v "$required" >/dev/null 2>&1; then
    printf 'nvim-mcp-connect: %s is not on PATH, and the resolver needs it\n' "$required" >&2
    exit 2
  fi
done

# The registry Neovim writes on VimEnter and removes on VimLeavePre
# (dot_config/nvim/lua/config/autocmds.lua): "<pane id> <pid> <socket> <cwd>",
# cwd last because it is the field that can hold spaces.
registry="${NVIM_MCP_REGISTRY:-${XDG_STATE_HOME:-$HOME/.local/state}/nvim-mcp/registry}"
server="${NVIM_MCP_BIN:-$HOME/.local/libexec/nvim-mcp/nvim-mcp}"
# The root :help serverstart() documents. $TMPDIR alone is the macOS case and
# misses every Linux socket; $TMPDIR already ends in a slash there.
runtime_root="${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp/}nvim.${USER:-$(id -un)}}"

refuse() {
  printf 'nvim-mcp-connect: %s\n' "$*" >&2
  exit 3
}

# identity <socket> -- what the instance behind <socket> says it is, as
# "<pane id> <pid>". Empty output (or a non-zero nvim) means it did not answer.
identity() {
  nvim --server "$1" --remote-expr \
    'join([getenv("HERDR_PANE_ID"), getpid()], " ")' 2>/dev/null || true
}

# --- step 1: injection -------------------------------------------------------
if [[ -n ${NVIM_MCP_SOCKET:-} ]]; then
  [[ -n "$(identity "$NVIM_MCP_SOCKET")" ]] ||
    refuse "NVIM_MCP_SOCKET names $NVIM_MCP_SOCKET, which no Neovim answers on; unset it or point it at a live instance"
  exec "$server" --connect "$NVIM_MCP_SOCKET"
fi

[[ -n ${HERDR_PANE_ID:-} ]] ||
  refuse 'HERDR_PANE_ID is not set, so there is no pane to resolve from; export NVIM_MCP_SOCKET, or start the agent in a herdr pane'

# --- step 2: topology --------------------------------------------------------
# herdr answers a bad pane id with {"error":...} and exit 0, so the shape is
# checked rather than the status.
layout="$(herdr pane layout --pane "$HERDR_PANE_ID" 2>/dev/null || true)"
tab="$(printf '%s' "$layout" | jq -r '.result.layout.tab_id // empty' 2>/dev/null || true)"
if [[ -z $tab ]]; then
  printf 'nvim-mcp-connect: herdr did not report a layout for pane %s\n' "$HERDR_PANE_ID" >&2
  exit 2
fi
siblings=" $(printf '%s' "$layout" | jq -r '.result.layout.panes[].pane_id' | tr '\n' ' ')"

# --- step 3: identity over the registry --------------------------------------
# Candidates accumulate as "<pane id> <pid> <socket>" lines; pruned registry
# entries are dropped by rewriting the file from the lines that survived.
candidates=""
kept=""
pruned=0
if [[ -r $registry ]]; then
  while read -r pane pid sock cwd; do
    [[ -n ${pane:-} && -n ${pid:-} && -n ${sock:-} ]] || continue
    # A pane outside this tab is not ours to judge: keep the entry, skip it.
    if [[ $siblings != *" $pane "* ]]; then
      kept="$kept$pane $pid $sock $cwd"$'\n'
      continue
    fi
    # Identity, not presence: the entry names a pid, and only the instance
    # that reports BOTH that pid and that pane id is the one it registered.
    if [[ "$(identity "$sock")" != "$pane $pid" ]]; then
      pruned=1
      continue
    fi
    kept="$kept$pane $pid $sock $cwd"$'\n'
    candidates="$candidates$pane $pid $sock"$'\n'
  done <"$registry"
  if [[ $pruned -eq 1 ]]; then
    printf '%s' "$kept" >"$registry"
  fi
fi

# The runtime-root fallback, for an instance that never wrote a registry line.
# The pid comes out of the filename, so kill -0 filters the graveyard without a
# single connection; identity still runs on whatever survives, because a pid is
# reusable too.
if [[ -z $candidates ]]; then
  for sock in "$runtime_root"/*/nvim.*.0; do
    [[ -S $sock || -e $sock ]] || continue
    pid="${sock##*/nvim.}"
    pid="${pid%.0}"
    [[ $pid =~ ^[0-9]+$ ]] || continue
    kill -0 "$pid" 2>/dev/null || continue
    reported="$(identity "$sock")"
    pane="${reported% *}"
    [[ -n $reported && $reported == "$pane $pid" && $siblings == *" $pane "* ]] || continue
    candidates="$candidates$pane $pid $sock"$'\n'
  done
fi

# --- steps 4 and 5: pick, do not stall; refuse only when nothing is alive -----
count="$(printf '%s' "$candidates" | grep -c . || true)"
case "$count" in
  0)
    refuse "no live Neovim in tab $tab (the tab of pane $HERDR_PANE_ID); launch the agent from Neovim (<leader>Cc), or export NVIM_MCP_SOCKET"
    ;;
  1)
    read -r pane pid sock <<<"$candidates"
    exec "$server" --connect "$sock"
    ;;
esac

{
  printf 'nvim-mcp-connect: %s Neovim instances in tab %s answer for this pane, so it will not guess.\n' \
    "$count" "$tab"
  printf 'Re-run with NVIM_MCP_SOCKET set to one of:\n'
  while read -r pane pid sock; do
    [[ -n ${sock:-} ]] || continue
    printf '  %s  pane %s  pid %s\n' "$sock" "$pane" "$pid"
  done <<<"$candidates"
} >&2
exit 4
