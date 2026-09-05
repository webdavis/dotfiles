#!/usr/bin/env bash
#
# nvim-mcp-connect.sh -- answer "which Neovim socket should this agent talk to",
# then exec nvim-mcp against it. This is the command both harnesses register as
# the Neovim MCP server, never nvim-mcp itself, which picks an instance by
# current directory or git root and so cannot tell two Neovim panes in one herdr
# workspace apart (docs/research/2026-09-nvim-mcp-evaluation.md, criterion 3).
#
# The five steps are spec 7.3's and are not restated here. What this file adds:
#
#   - The pane id is passed to `herdr pane layout` EXPLICITLY. `pane current`
#     answers the CALLER's pane, so a resolver that asks it matches nothing.
#   - Identity is a BOUNDED probe whose WHOLE reply must match the grammar. A
#     socket that accepts and never answers would hang the harness forever, and
#     a reply merely trimmed to fit can be padded into a valid-looking one.
#   - Only a unix socket inside a directory this user owns at 0700 is connected
#     to. `nvim --listen` takes a TCP address or any path and Neovim trusts
#     every RPC peer, so a reachable endpoint is a rebindable one.
#
# NVIM_MCP_SOCKET selecting from the verified set, and the exit-4 picker, are
# both REQUIRED by 7.3 as amended on 2026-09-05. Neither is a deviation.
#
# Exit codes: 3 a refusal, 4 the picker, 2 an environmental fault (a missing
# tool, unsafe registry state, a broken herdr answer). On success this process
# is REPLACED by nvim-mcp, so there is no fourth outcome. No memo file and no
# per-call recheck: nvim-mcp connects once at startup and keeps that client, so
# the pin lasts the server process and a Neovim that exits leaves a stale client
# until the harness starts a new session. Spec 7.3's sticky selection is amended
# to say so; automatic recovery is server lifecycle work, on the crate row.
#
# TWO STATED LIMITS, left open for the operator rather than worked around.
# (a) The probe and the connection are separate processes, so a same-UID process
# can rebind the path between them. Every same-UID process here already holds
# the operator's whole authority, so that is inside the trust boundary, not
# across it; closing it needs a server that verifies pane and pid over the
# connection it keeps, which is the custom-crate row.
# (b) The picker is an exit code and an enumeration on stderr rather than a tool
# result, which amended 7.3 accepts on this row: a wrapper that execs the server
# cannot return one, and the tool result lives on the crate row.
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
for required in nvim jq realpath; do
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
# The root :help serverstart() documents. XDG_RUNTIME_DIR is the Linux case and
# $TMPDIR/nvim.<user> the macOS one; neither alone covers both. The separator is
# NORMALIZED rather than assumed: macOS sets TMPDIR with a trailing slash and
# most other systems do not, so concatenating the two directly searches
# "<dir>nvim.<user>" wherever it does not, and finds nothing.
tmp_root="${TMPDIR:-/tmp}"
runtime_root="${XDG_RUNTIME_DIR:-${tmp_root%/}/nvim.${USER:-$(id -un)}}"
# Seconds a single identity probe may take. A knob only so the test can bound
# itself well inside its own one-second budget.
caller_uid="$(id -u)"
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

# node_meta <path...> -- "<octal mode> <uid>" per path, one line each, from
# LSTAT, so a symlink reports ITSELF rather than what it points at. %Mp%Lp, not
# %Lp: the short form drops the sticky bit, which would read /tmp as a directory
# other accounts can write freely. BSD first, GNU second; neither answering
# leaves this empty, which every caller reads as a refusal. Takes the WHOLE
# chain at once, because one stat over six paths is one process and six stats
# are six.
node_meta() {
  stat -f '%Mp%Lp %u' "$@" 2>/dev/null || stat -c '%a %u' "$@" 2>/dev/null || true
}

# dir_fault <canonical directory> -- why that directory is not private to this
# user, all the way up, empty when it is.
#
# THE ARGUMENT MUST BE CANONICAL, from realpath, and the caller must go on to
# use that same canonical path. Checking the name as written and then operating
# on the name as written are two DIFFERENT paths as soon as any component is a
# symlink, and that gap is the whole bug class this closes: a parent link whose
# own mode was 0700 over a 0777 target satisfied both the ownership test, which
# follows the link, and the mode test, which does not.
#
# The directory itself must be owned by this user at 0700. Every ancestor must
# be owned by this user or by root, and must not be writable by other accounts
# without the sticky bit that stops them removing what is not theirs, because
# such a directory is one where they can swap the subtree between any two
# operations here.
#
# `nvim --listen` takes a TCP address or any path, and Neovim trusts every RPC
# peer it accepts, so an endpoint another account can reach is one they can
# rebind after its owner dies and then answer the identity probe from.
dir_fault() {
  local dir="$1" chain=() node="$1" why="" line mode uid seen=0
  while :; do
    chain+=("$node")
    [[ $node != / ]] || break
    node="${node%/*}"
    [[ -n $node ]] || node=/
  done
  while IFS= read -r line; do
    read -r mode uid <<<"$line"
    node="${chain[seen]}"
    seen=$((seen + 1))
    [[ -z $why ]] || continue
    if [[ $uid != "$caller_uid" && ($seen == 1 || $uid != 0) ]]; then
      why="sits under $node, which this user does not own"
    elif [[ $seen == 1 ]] && ((8#$mode != 448)); then
      why="sits in $node, which is mode $mode rather than 0700"
    elif [[ $seen != 1 ]] && (((8#$mode & 18) != 0 && (8#$mode & 512) == 0)); then
      why="has an ancestor at $node other accounts can write and that is not sticky"
    fi
  done < <(node_meta "${chain[@]}")
  # A short reading means stat could not answer for some link in the chain, and
  # an unanswered ancestor is not one this can vouch for.
  [[ $seen == "${#chain[@]}" ]] || why="${why:-cannot be read all the way up from $dir}"
  printf '%s' "$why"
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

# Read AND pruned, so its state is checked before either, and the CANONICAL
# form is what gets kept: everything below reads and deletes through that, not
# through the configured string. Absent is fine, the fallback covers it.
registry_real="$(realpath "$registry" 2>/dev/null || true)"
if [[ -n $registry_real ]]; then
  [[ -d $registry_real ]] || die 2 "the registry $registry is not a directory"
  fault="$(dir_fault "$registry_real")"
  [[ -z $fault ]] || die 2 "the registry $registry $fault"
  registry="$registry_real"
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
  # endpoint. Refused rather than skipped, because a record is OUR data and this
  # is an anomaly rather than noise.
  if [[ $sock != /* ]]; then
    die 3 "the record for pane $pane names $sock, which is not an absolute path, so it is a TCP or named endpoint that another account can observe and rebind"
  fi
  # NOTHING absent is probed. A dead instance is pruned here instead, because
  # the gap between finding a pathname missing and connecting to it is a gap in
  # which another account can create it, wherever the path is one they can
  # write, and then answer the identity probe as an instance that is gone. The
  # -L arm keeps a dangling symlink out of this branch, so it reaches the
  # endpoint check below and is refused by name rather than quietly pruned.
  if [[ ! -e $sock && ! -L $sock ]]; then
    rm -f "$record"
    continue
  fi
  # Resolved ONCE here, validated as the resolved form, and used as the resolved
  # form from here on: probing or connecting to the recorded string after
  # validating something else is the gap this closes. Every surviving endpoint
  # is validated BEFORE the probe, so nothing unchecked reaches --remote-expr.
  sock_real="$(realpath "$sock" 2>/dev/null || true)"
  [[ -n $sock_real ]] ||
    die 3 "the record for pane $pane names $sock, which cannot be resolved to a real path"
  [[ -S $sock_real ]] || die 3 "the record for pane $pane names $sock, which is not a unix socket"
  fault="$(dir_fault "${sock_real%/*}")"
  [[ -z $fault ]] || die 3 "the record for pane $pane names $sock, which $fault"
  sock="$sock_real"
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
    # drop a file into, so one odd entry must not block every resolution. Same
    # rule as above: the resolved form is what is judged and what is used.
    sock_real="$(realpath "$sock" 2>/dev/null || true)"
    [[ -n $sock_real && -S $sock_real ]] || continue
    [[ -z "$(dir_fault "${sock_real%/*}")" ]] || continue
    sock="$sock_real"
    reported="$(identity "$sock")"
    pane="${reported% *}"
    [[ -n $reported && $reported == "$pane $pid" && $siblings == *" $pane "* ]] || continue
    candidates="$candidates$pane $pid $sock"$'\n'
  done
fi

# Injection SELECTS, it does not bypass.
if [[ -n ${NVIM_MCP_SOCKET:-} ]]; then
  # Canonical on both sides, because the candidates are canonical: comparing a
  # pin as typed against a resolved candidate never matches for any path with a
  # symlink in it.
  pin_real="$(realpath "$NVIM_MCP_SOCKET" 2>/dev/null || true)"
  pinned=""
  while read -r pane pid sock; do
    [[ -n ${sock:-} && ${sock:-} == "$pin_real" ]] && pinned="$pane $pid $sock"$'\n'
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
