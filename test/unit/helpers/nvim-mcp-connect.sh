# shellcheck shell=bash
# Shared fixture for the nvim-mcp-connect.sh tests. SOURCED, never executed:
# test/unit/nvim-mcp-connect-resolution.sh and
# test/unit/nvim-mcp-connect-refusals.sh each source this and then run their own
# cases. One fixture rather than two that can drift apart.
#
# What it provides:
#   fail <message>          print and exit non-zero
#   setup_case <name>       a private sandbox with a stub nvim and nvim-mcp
#   private_path <tool>...  a PATH holding only what is named
#   make_socket <path>...   real, bound unix sockets
#   live <path>...          real sockets the nvim stub ANSWERS on
#   run_case <env...>       run the resolver; sets RC, $CASE/out, $CASE/err
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
[[ -f $SCRIPT ]] || fail "missing script: $SCRIPT"

# Under /tmp with a SHORT name, not the Darwin per-user temp directory: these
# cases bind real unix sockets, and sun_path is 104 bytes, which
# /var/folders/<...>/T/tmp.XXXXXXXXXX/<case>/... exhausts on its own.
work="$(mktemp -d /tmp/nmc.XXXXXX)"
trap 'rm -rf "$work"' EXIT

# make_socket <path>... -- real, bound, listening unix sockets, all in ONE perl
# process: bash cannot make one, the resolver's `-S` test wants the real thing,
# and a process per socket is the most expensive thing this fixture does.
make_socket() {
  /usr/bin/perl -MIO::Socket::UNIX -e \
    'IO::Socket::UNIX->new(Local => $_, Listen => 1) or die "$_: $!" for @ARGV' "$@"
}

# The stubs are written ONCE and find their case through NMC_CASE at run time.
mkdir -p "$work/bin"

# The two things the resolver asks nvim for. `--server <socket> --remote-expr 1`
# is the liveness probe: the socket is logged BEFORE the answer is looked up,
# then it hangs if listed in $NMC_CASE/hang (as ONE process, the way a stuck
# nvim client is: a child holding the pipe would outlive the watchdog's kill),
# answers exactly `1` (no newline, as the real reply) if listed in
# $NMC_CASE/live, and otherwise exits 1 the way a refused connection does. Any
# other invocation is the run-dir query, logged to $NMC_CASE/queried and
# answered with the contents of $NMC_CASE/rundir.
cat >"$work/bin/nvim" <<'STUB'
#!/bin/bash
if [[ $1 == --server ]]; then
  printf '%s\n' "$2" >>"$NMC_CASE/probed"
  grep -qxF -- "$2" "$NMC_CASE/hang" && exec sleep 3
  grep -qxF -- "$2" "$NMC_CASE/live" || exit 1
  printf 1
  exit 0
fi
printf '%s\n' "$*" >>"$NMC_CASE/queried"
cat "$NMC_CASE/rundir"
STUB

cat >"$work/bin/nvim-mcp" <<'STUB'
#!/bin/bash
printf '%s\n' "$*" >"$NMC_CASE/exec"
STUB

chmod +x "$work/bin/nvim" "$work/bin/nvim-mcp"

# setup_case <name> -- a private sandbox for one case. Sets CASE (its
# directory), with:
#   $CASE/run       the run root (production: $TMPDIR/nvim.<user>); pane
#                   sockets are expected directly inside it
#   $CASE/rundir    what the nvim stub reports as stdpath("run"): a per-process
#                   directory UNDER the run root, the shape 0.12 gives
#   $CASE/live      sockets the nvim stub answers on
#   $CASE/hang      sockets the nvim stub never answers on
#   $CASE/probed    every socket the nvim stub was asked about
#   $CASE/queried   every run-dir query the nvim stub received
#   $CASE/exec      the argv the nvim-mcp stub was execed with
setup_case() {
  CASE="$work/$1"
  CASE_PATH="$work/bin:/usr/bin:/bin"
  mkdir -p "$CASE/run"
  : >"$CASE/live"
  : >"$CASE/hang"
  printf '%s/a1b2c3' "$CASE/run" >"$CASE/rundir"
}

# live <path>... -- real sockets the probe answers on.
live() {
  make_socket "$@"
  printf '%s\n' "$@" >>"$CASE/live"
}

# private_path <tool>... -- a PATH holding ONLY the named tools plus what the
# resolver itself needs (bash, dirname, sleep), so an absence fixture cannot be
# invalidated by whatever another host keeps in /usr/bin.
private_path() {
  mkdir -p "$CASE/pathbin"
  ln -s /bin/bash /usr/bin/dirname /bin/sleep "$@" "$CASE/pathbin/"
  CASE_PATH="$CASE/pathbin"
}

# run_case <env assignments...> -- runs the resolver in the current CASE, on
# CASE_PATH, under `env -i` so nothing of this shell's own herdr or pin leaks
# in. XDG_RUNTIME_DIR is the caller's to set: most cases point it at $CASE/run,
# and the one that leaves it unset is testing the run-dir query. The probe
# deadline defaults to production's two seconds; only the case that WANTS it to
# expire shortens it, through CASE_DEADLINE.
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
    "$@" \
    bash "$SCRIPT" >"$CASE/out" 2>"$CASE/err" || RC=$?
}
