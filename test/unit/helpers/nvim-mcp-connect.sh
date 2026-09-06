# shellcheck shell=bash
# Shared fixture for the nvim-mcp-connect.sh tests. SOURCED, never executed:
# test/unit/nvim-mcp-connect-resolution.sh, nvim-mcp-connect-refusals.sh and
# nvim-mcp-connect-siblings.sh each source this and then run their own cases.
# One fixture rather than three that can drift apart.
#
# What it provides:
#   fail <message>            print and exit non-zero
#   setup_case <name>         a private sandbox with a stub nvim, herdr and nvim-mcp
#   private_path <tool>...    a PATH holding only what is named
#   make_socket <path>...     real, bound unix sockets
#   live <path>...            real sockets the nvim stub ANSWERS on
#   me <terminal>             what herdr answers for the resolver's own pane
#   siblings <tab>|<term>|<pane>...  what herdr answers for the workspace's panes
#   sock <terminal>           the socket path that terminal's Neovim listens on
#   run_case <env...>         run the resolver; sets RC, $CASE/out, $CASE/err
#
# The caller sets nothing first and cleans up nothing after: this installs its
# own work directory and EXIT trap in the sourcing shell.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/libexec/nvim-mcp/executable_nvim-mcp-connect.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

if [[ ! -x /usr/bin/perl ]]; then
  printf 'SKIP: /usr/bin/perl is absent, and the socket cases need a real unix socket\n'
  exit 0
fi
if ! command -v jq >/dev/null 2>&1; then
  printf 'SKIP: jq not on PATH; the resolver needs it\n'
  exit 0
fi
[[ -f $SCRIPT ]] || fail "missing script: $SCRIPT"
# Read by private_path in the sourcing test, not here.
# shellcheck disable=SC2034
JQ_PATH="$(command -v jq)"

# Under /tmp with a SHORT name, not the Darwin per-user temp directory: these
# cases bind real unix sockets, and sun_path is 104 bytes, which
# /var/folders/<...>/T/tmp.XXXXXXXXXX/<case>/... exhausts on its own.
work="$(mktemp -d /tmp/nmc.XXXXXX)"
trap 'rm -rf "$work"' EXIT

# The session half of every socket name: sha256("/s/a.sock") starts with
# 9a663d, run_case exports HERDR_SOCKET_PATH=/s/a.sock, and the nvim stub
# answers that hash the way the real vim.fn.sha256 would. The Lua spec pins the
# same six characters, so both sides are held to one rule.
SESSION=9a663d

# make_socket <path>... -- real, bound, listening unix sockets, all in ONE perl
# process: bash cannot make one, the resolver's `-S` test wants the real thing,
# and a process per socket is the most expensive thing this fixture does.
make_socket() {
  /usr/bin/perl -MIO::Socket::UNIX -e \
    'IO::Socket::UNIX->new(Local => $_, Listen => 1) or die "$_: $!" for @ARGV' "$@"
}

# The stubs are written ONCE and find their case through NMC_CASE at run time.
mkdir -p "$work/bin"

# The two things the resolver asks nvim for. `--server <socket> --remote-expr
# getpid()` is the liveness probe: the socket is logged BEFORE the answer is
# looked up, then it hangs if listed in $NMC_CASE/hang (as ONE process, the
# way a stuck nvim client is: a child holding the pipe would outlive the
# watchdog's kill), answers a pid with no newline (as the real reply) if listed
# in $NMC_CASE/live, and otherwise exits 1 the way a refused connection does.
# Any other invocation is the identity query, logged to $NMC_CASE/queried and
# answered with two lines: the contents of $NMC_CASE/rundir (production:
# stdpath("run")) and the session hash (production: vim.fn.sha256 of
# HERDR_SOCKET_PATH, first six characters; here the fixed SESSION).
cat >"$work/bin/nvim" <<'STUB'
#!/bin/bash
if [[ $1 == --server ]]; then
  printf '%s\n' "$2" >>"$NMC_CASE/probed"
  grep -qxF -- "$2" "$NMC_CASE/hang" && exec sleep 3
  grep -qxF -- "$2" "$NMC_CASE/live" || exit 1
  printf 4242
  exit 0
fi
printf '%s\n' "$*" >>"$NMC_CASE/queried"
cat "$NMC_CASE/rundir"
printf '\n%s' 9a663d
STUB

# herdr 0.8.2 as the resolver sees it. `pane current --current` answers the
# document in $NMC_CASE/me.json, or exits 1 with nothing when there is none;
# `pane list --workspace <the workspace me.json named>` answers
# $NMC_CASE/list.json. Every call is logged; anything else is refused loudly.
# $NMC_CASE/herdr-hang makes every call hang as one process, herdr-fail makes
# every call fail the way a crashed herdr does, herdr-list-fail only the list.
cat >"$work/bin/herdr" <<'STUB'
#!/bin/bash
printf '%s\n' "$*" >>"$NMC_CASE/herdr-argv"
[[ -e $NMC_CASE/herdr-hang ]] && exec sleep 3
[[ -e $NMC_CASE/herdr-fail ]] && exit 1
if [[ "$*" == "pane current --current" ]]; then
  [[ -f $NMC_CASE/me.json ]] || exit 1
  exec cat "$NMC_CASE/me.json"
fi
if [[ -f $NMC_CASE/ws && "$*" == "pane list --workspace $(cat "$NMC_CASE/ws")" ]]; then
  [[ -e $NMC_CASE/herdr-list-fail ]] && exit 1
  exec cat "$NMC_CASE/list.json"
fi
printf 'herdr stub: unexpected argv: %s\n' "$*" >&2
exit 99
STUB

cat >"$work/bin/nvim-mcp" <<'STUB'
#!/bin/bash
printf '%s\n' "$*" >"$NMC_CASE/exec"
STUB

chmod +x "$work/bin/nvim" "$work/bin/herdr" "$work/bin/nvim-mcp"

# setup_case <name> -- a private sandbox for one case. Sets CASE (its
# directory) and RUN (the run root, production's $TMPDIR/nvim.<user>), with:
#   $CASE/rundir       what the nvim stub reports as stdpath("run"): a
#                      per-process directory UNDER the run root, the shape 0.12
#                      gives; the session hash follows it on a second line
#   $CASE/live         sockets the nvim stub answers on
#   $CASE/hang         sockets the nvim stub never answers on
#   $CASE/me.json      herdr's answer for the resolver's own pane (see `me`)
#   $CASE/list.json    herdr's answer for the workspace's panes (see `siblings`)
#   $CASE/probed       every socket the nvim stub was asked about
#   $CASE/queried      every run-dir query the nvim stub received
#   $CASE/herdr-argv   every herdr call
#   $CASE/exec         the argv the nvim-mcp stub was execed with
setup_case() {
  CASE="$work/$1"
  CASE_PATH="$work/bin:/usr/bin:/bin"
  RUN="$CASE/run"
  # 0700 like the real run root: the resolver refuses anything looser.
  mkdir -p "$RUN"
  chmod 700 "$RUN"
  : >"$CASE/live"
  : >"$CASE/hang"
  printf '%s/a1b2c3' "$RUN" >"$CASE/rundir"
}

# live <path>... -- real sockets the probe answers on.
live() {
  make_socket "$@"
  printf '%s\n' "$@" >>"$CASE/live"
}

# me <terminal> -- herdr's answer for the resolver's own pane: pane w1:p1 in
# tab w1:t1 of workspace w1, on the given terminal.
me() {
  printf '{"id":"cli:pane:current","result":{"pane":{"pane_id":"w1:p1","tab_id":"w1:t1","terminal_id":"%s","workspace_id":"w1"}}}' \
    "$1" >"$CASE/me.json"
  printf 'w1' >"$CASE/ws"
}

# siblings <tab>|<terminal>|<pane>... -- herdr's answer for the workspace's
# panes, in the 0.8.2 shape the resolver reads.
siblings() {
  local entries="" spec tab terminal pane
  for spec in "$@"; do
    IFS='|' read -r tab terminal pane <<<"$spec"
    entries="$entries{\"pane_id\":\"$pane\",\"tab_id\":\"$tab\",\"terminal_id\":\"$terminal\",\"workspace_id\":\"w1\"},"
  done
  printf '{"id":"cli:pane:list","result":{"panes":[%s]}}' "${entries%,}" >"$CASE/list.json"
}

# sock <terminal> -- the socket a Neovim on that terminal listens on, under RUN.
sock() {
  printf '%s/herdr-%s-%s.sock' "$RUN" "$SESSION" "$1"
}

# private_path <tool>... -- a PATH holding ONLY the named tools plus what the
# resolver itself needs (bash, dirname, sleep), so an absence
# fixture cannot be invalidated by whatever another host keeps in /usr/bin.
private_path() {
  mkdir -p "$CASE/pathbin"
  ln -s /bin/bash /usr/bin/dirname /bin/sleep "$@" "$CASE/pathbin/"
  CASE_PATH="$CASE/pathbin"
}

# run_case <env assignments...> -- runs the resolver in the current CASE, on
# CASE_PATH, under `env -i` so nothing of this shell's own herdr or pin leaks
# in. The case is inside herdr by default (HERDR_ENV, a fixed
# HERDR_SOCKET_PATH); a case outside herdr passes HERDR_ENV= to unset it.
# XDG_RUNTIME_DIR is the caller's to set: most cases point it at $RUN, and the
# one that leaves it unset is testing the run-dir query. The deadline defaults
# to production's two seconds; only the cases that WANT it to expire shorten
# it, through CASE_DEADLINE.
#
# Sets RC; stdout is $CASE/out, stderr is $CASE/err. RC is read by the sourcing
# test, not here.
# shellcheck disable=SC2034
run_case() {
  RC=0
  env -i \
    PATH="$CASE_PATH" \
    HOME="$CASE" \
    NVIM_MCP_BIN="$work/bin/nvim-mcp" \
    NMC_CASE="$CASE" \
    NVIM_MCP_PROBE_DEADLINE="${CASE_DEADLINE:-2}" \
    HERDR_ENV=1 \
    HERDR_SOCKET_PATH=/s/a.sock \
    "$@" \
    bash "$SCRIPT" >"$CASE/out" 2>"$CASE/err" || RC=$?
}
