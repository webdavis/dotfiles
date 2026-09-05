#!/usr/bin/env bash
#
# nvim-mcp-connect.sh -- answer "which Neovim socket should this agent talk to",
# then exec nvim-mcp against it. This is the command both harnesses register as
# the Neovim MCP server, never nvim-mcp itself, which picks an instance by
# current directory or git root and so cannot tell two Neovim panes in one herdr
# workspace apart (docs/research/2026-09-nvim-mcp-evaluation.md, criterion 3).
#
# The five steps are spec 7.3's and are not restated here. Three things this
# file decides that the spec does not:
#
#   - The pane id is passed to `herdr pane layout` EXPLICITLY. `pane current`
#     answers the CALLER's pane, so a resolver that asks it matches nothing.
#   - NVIM_MCP_SOCKET SELECTS from the verified set rather than bypassing it.
#     The spec would use an injected socket as-is, which lets a stale or hostile
#     value reach any Neovim the caller can open. Both injection directions
#     split the pane inside the caller's own tab, so nothing legitimate is lost.
#   - Identity is a BOUNDED probe with a strict reply grammar, because a socket
#     that accepts and never answers would otherwise hang the harness forever.
#
# Exit codes: 3 a refusal, 4 the picker, 2 an environmental fault (a missing
# tool, unsafe registry state, a broken herdr answer). On success this process
# is REPLACED by nvim-mcp, so there is no fourth outcome. No memo file: the
# harness starts this once per session and the exec holds that instance for the
# session, which is the sticky selection spec 7.3 asks for.
#
# TWO STATED LIMITS, left open for the operator rather than worked around.
# (a) The probe and the connection are separate processes, so a same-UID process
# can rebind the path between them. Every same-UID process here already holds
# the operator's whole authority, so that is inside the trust boundary, not
# across it; closing it needs a server that verifies pane and pid over the
# connection it keeps, which is the custom-crate row.
# (b) The picker is an exit code and stderr, not the tool result spec 7.3 step 4
# asks for, because a wrapper that execs the server cannot return one.
set -euo pipefail

# die <exit code> <message...>
die() {
  local code="$1"
  shift
  printf 'nvim-mcp-connect: %s\n' "$*" >&2
  exit "$code"
}

# Both hard dependencies, checked FIRST, before herdr, the registry or any
# socket: otherwise the missing tool surfaces as whatever fails next, which
# reads as a herdr fault and sends the operator to debug the wrong thing.
for required in nvim jq; do
  command -v "$required" >/dev/null 2>&1 ||
    die 2 "$required is not on PATH, and the resolver needs it"
done

# Written by dot_config/nvim/lua/config/autocmds.lua: a 0700 DIRECTORY holding
# one file per instance, named for its pid, holding "<pane> <pid> <socket>
# <cwd>" (cwd last, the only field that can hold spaces). One file per pid, not
# one shared file: two instances starting at once would lose each other's line,
# and a nested Neovim inheriting its parent's pane id would replace it instead
# of becoming the second candidate a picker names.
registry="${NVIM_MCP_REGISTRY:-${XDG_STATE_HOME:-$HOME/.local/state}/nvim-mcp/registry}"
server="${NVIM_MCP_BIN:-$HOME/.local/libexec/nvim-mcp/nvim-mcp}"
# The root :help serverstart() documents. $TMPDIR alone is the macOS case and
# misses every Linux socket; $TMPDIR already ends in a slash there.
runtime_root="${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp/}nvim.${USER:-$(id -un)}}"
# Seconds a single identity probe may take. A knob only so the test can bound
# itself well inside its own one-second budget.
deadline="${NVIM_MCP_PROBE_DEADLINE:-2}"
[[ $deadline =~ ^[0-9]+(\.[0-9]+)?$ ]] || die 2 'NVIM_MCP_PROBE_DEADLINE must be seconds'

# An EXPLICIT template, because a bare `mktemp` on macOS ignores TMPDIR and uses
# the per-user Darwin temp directory instead, which puts the probe file
# somewhere neither this script nor its test chose.
probe_dir="${TMPDIR:-/tmp}"
probe_out="$(mktemp "${probe_dir%/}/nvim-mcp-connect.XXXXXX")"
# `|| true` so a failing cleanup cannot overwrite the exit status: bash reports
# the trap's own status, and a refusal that surfaces as 127 reads to the harness
# as a crash rather than as the reason it printed.
trap 'rm -f "$probe_out" 2>/dev/null || true' EXIT

# dir_mode <path> -- the permission bits. BSD stat first, GNU second; neither
# answering leaves this empty, which every caller reads as a refusal.
dir_mode() {
  stat -f '%Lp' "$1" 2>/dev/null || stat -c '%a' "$1" 2>/dev/null || true
}

# socket_fault <path> -- why this endpoint is not a private unix socket, empty
# when it is one. `nvim --listen` takes a TCP address or any path, and Neovim
# trusts every RPC peer it accepts, so an endpoint another account can reach is
# an endpoint another account can rebind after its owner dies and then answer
# the identity probe with whatever it likes. Only a socket inside a directory
# this user owns at 0700 is out of that reach.
socket_fault() {
  local dir mode
  [[ -S $1 ]] || {
    printf 'is not a unix socket'
    return 0
  }
  dir="${1%/*}"
  [[ -O $dir ]] || {
    printf 'sits in %s, which this user does not own' "$dir"
    return 0
  }
  mode="$(dir_mode "$dir")"
  [[ $mode == 700 ]] ||
    printf 'sits in %s, which is mode %s rather than 0700' "$dir" "${mode:-unreadable}"
}

# identity <socket> -- "<pane id> <pid>" as the instance reports itself, empty
# if it does not. Deadline-bounded (stock macOS ships no timeout(1)), and the
# reply is rejected WHOLE rather than trimmed to fit: a cap that truncates turns
# malformed output into a valid identity, which a 1 MiB answer opening with the
# right pane and pid and padded with newlines demonstrated.
identity() {
  local job watchdog reply size
  : >"$probe_out"
  nvim --server "$1" --remote-expr \
    'join([getenv("HERDR_PANE_ID"), getpid()], " ")' >"$probe_out" 2>/dev/null &
  job=$!
  # Stock macOS ships no timeout(1) and bash has no wait-with-deadline, so a
  # child that TERMs the probe is the portable stand-in. It costs a healthy
  # probe NOTHING, which a polling loop cannot say: at any poll interval, every
  # probe pays one interval it did not need.
  { sleep "$deadline" && kill -TERM "$job"; } </dev/null >/dev/null 2>&1 &
  watchdog=$!
  wait "$job" 2>/dev/null || true
  kill -TERM "$watchdog" 2>/dev/null || true
  # Size FIRST, from the file, so an over-long reply is refused rather than cut
  # down to something well-formed.
  size="$(wc -c <"$probe_out" | tr -d '[:space:]')"
  [[ $size =~ ^[0-9]+$ ]] && ((size <= 128)) || return 0
  # The `x` sentinel preserves trailing newlines, which command substitution
  # strips; stripping them is what let padding hide a garbage suffix. A real
  # reply carries none (measured: `--remote-expr` emits no trailing newline), so
  # the grammar sees the COMPLETE reply and any newline at all fails it.
  reply="$(
    cat "$probe_out"
    printf x
  )"
  reply="${reply%x}"
  [[ $reply =~ ^[A-Za-z0-9:_-]+\ [0-9]+$ ]] || return 0
  printf '%s' "$reply"
}

# Read AND pruned, so the state is checked before either. A symlink would aim a
# delete elsewhere; anything looser than a 0700 directory this user owns lets
# another account plant a record. Absent is fine, the fallback covers it.
if [[ -L $registry ]]; then
  die 2 "the registry $registry is a symlink; refusing to read or prune through it"
elif [[ -e $registry ]]; then
  [[ -d $registry ]] || die 2 "the registry $registry is not a directory"
  [[ -O $registry ]] || die 2 "the registry $registry is not owned by this user"
  mode="$(dir_mode "$registry")"
  [[ $mode == 700 ]] || die 2 "the registry $registry is not mode 0700"
fi

[[ -n ${HERDR_PANE_ID:-} ]] ||
  die 3 'HERDR_PANE_ID is not set, so there is no pane to resolve from; start the agent in a herdr pane'

# herdr answers a bad pane id with {"error":...} and exit 0, so the shape is
# checked rather than the status.
layout="$(herdr pane layout --pane "$HERDR_PANE_ID" 2>/dev/null || true)"
tab="$(printf '%s' "$layout" | jq -r '.result.layout.tab_id // empty' 2>/dev/null || true)"
[[ -n $tab ]] || die 2 "herdr did not report a layout for pane $HERDR_PANE_ID"
# Space-delimited on both ends, because membership is tested as *" $pane "*.
siblings=" $(printf '%s' "$layout" | jq -r '.result.layout.panes[].pane_id' | tr '\n' ' ')"

# Candidates accumulate as "<pane id> <pid> <socket>" lines.
candidates=""
for record in "$registry"/*; do
  [[ -f $record && ! -L $record ]] || continue
  read -r pane pid sock _ <"$record" || true
  # A record that disagrees with its own filename is not one this design wrote.
  [[ -n ${pane:-} && -n ${sock:-} && ${pid:-} == "${record##*/}" ]] || continue
  [[ $siblings == *" $pane "* ]] || continue
  # A record naming something other than an absolute path is a TCP or named
  # endpoint, and one naming a path that exists but is not a private socket is
  # reachable by another account. Both are refused, before any connection,
  # because a record is OUR data and one of these is an anomaly rather than
  # noise. A path that simply does not exist is a dead instance and falls
  # through to the identity check below, which prunes it.
  if [[ $sock != /* ]]; then
    die 3 "the record for pane $pane names $sock, which is not an absolute path, so it is a TCP or named endpoint that another account can observe and rebind"
  fi
  if [[ -e $sock ]]; then
    fault="$(socket_fault "$sock")"
    [[ -z $fault ]] || die 3 "the record for pane $pane names $sock, which $fault"
  fi
  if [[ "$(identity "$sock")" != "$pane $pid" ]]; then
    rm -f "$record"
    continue
  fi
  candidates="$candidates$pane $pid $sock"$'\n'
done

# The fallback, for an instance that never wrote a record. The pid is in the
# filename, so kill -0 filters the graveyard (383 dead sockets, no live one, per
# the evaluation) with no connection; identity still runs on the survivors.
if [[ -z $candidates ]]; then
  for sock in "$runtime_root"/*/nvim.*.0; do
    [[ -e $sock ]] || continue
    pid="${sock##*/nvim.}"
    pid="${pid%.0}"
    [[ $pid =~ ^[0-9]+$ ]] || continue
    kill -0 "$pid" 2>/dev/null || continue
    # Skipped rather than refused: the runtime root is a directory anything may
    # drop a file into, so one odd entry must not block every resolution.
    [[ -z "$(socket_fault "$sock")" ]] || continue
    reported="$(identity "$sock")"
    pane="${reported% *}"
    [[ -n $reported && $reported == "$pane $pid" && $siblings == *" $pane "* ]] || continue
    candidates="$candidates$pane $pid $sock"$'\n'
  done
fi

# Injection SELECTS, it does not bypass.
if [[ -n ${NVIM_MCP_SOCKET:-} ]]; then
  pinned=""
  while read -r pane pid sock; do
    [[ ${sock:-} == "$NVIM_MCP_SOCKET" ]] && pinned="$pane $pid $sock"$'\n'
  done <<<"$candidates"
  [[ -n $pinned ]] ||
    die 3 "NVIM_MCP_SOCKET names $NVIM_MCP_SOCKET, which is not a verified Neovim in tab $tab of pane $HERDR_PANE_ID; unset it, or pin a Neovim in this tab"
  candidates="$pinned"
fi

count="$(printf '%s' "$candidates" | grep -c . || true)"
case "$count" in
  0)
    die 3 "no live Neovim in tab $tab (the tab of pane $HERDR_PANE_ID); launch the agent from Neovim (<leader>Cc), or export NVIM_MCP_SOCKET"
    ;;
  1)
    read -r pane pid sock <<<"$candidates"
    # exec REPLACES this process, so the EXIT trap never runs. Clean up first.
    rm -f "$probe_out" 2>/dev/null || true
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
