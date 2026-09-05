# shellcheck shell=bash
# Shared fixture for the nvim-mcp-connect.sh tests. SOURCED, never executed:
# test/unit/nvim-mcp-connect-resolution.sh and
# test/unit/nvim-mcp-connect-refusals.sh each source this and then run their own
# cases. It lives here rather than in either file because both need the same
# stub world, and a second copy would be a second thing to drift.
#
# What it provides:
#   fail <message>            print and exit non-zero
#   setup_case <name>         a private sandbox with stub herdr/nvim/nvim-mcp
#   private_path <n>=<t>...   a PATH holding only what is named
#   record <pane> <pid> <so>  one registry file, named for the pid
#   write_layout <tab> <p...> what the herdr stub answers
#   run_case <env...>         run the resolver; sets RC, $CASE/out, $CASE/err
#   make_socket <path>        a real, bound unix socket
#   $dead_pid                 a pid that is certainly not running
#
# The caller sets nothing first and cleans up nothing after: this installs its
# own work directory and EXIT trap in the sourcing shell.
# The sourcing test sets its own errexit; this only needs the paths.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/libexec/nvim-mcp/executable_nvim-mcp-connect.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

if ! command -v jq >/dev/null 2>&1; then
  printf 'SKIP: jq not on PATH; the resolver parses herdr JSON with jq\n'
  exit 0
fi
if [[ ! -x /usr/bin/perl ]]; then
  printf 'SKIP: /usr/bin/perl is absent, and the socket cases need a real unix socket\n'
  exit 0
fi
[[ -f $SCRIPT ]] || fail "missing script: $SCRIPT"
# Read by private_path in the sourcing test, not here.
# shellcheck disable=SC2034
JQ_PATH="$(command -v jq)"

# make_socket <path> -- a real, bound, listening unix socket. System perl,
# because bash cannot make one and the resolver's `-S` test wants the real
# thing rather than a regular file standing in for it.
make_socket() {
  /usr/bin/perl -MIO::Socket::UNIX -e \
    'IO::Socket::UNIX->new(Local => $ARGV[0], Listen => 1) or die $!' "$1"
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# A pid that is certainly not running, so kill -0 on it fails: the dead half of
# case (h). Searched for rather than assumed, because a hardcoded number can be
# alive on somebody's machine.
dead_pid=0
for candidate in 999331 999332 999333 4194301; do
  if ! kill -0 "$candidate" 2>/dev/null; then
    dead_pid="$candidate"
    break
  fi
done
[[ $dead_pid != 0 ]] || fail 'could not find a pid that is not running'

# setup_case <name> -- a private sandbox with stub herdr/nvim/nvim-mcp on PATH.
# Sets CASE (its directory) for the caller to fill in:
#   $CASE/layout.json   what the herdr stub prints for `pane layout`
#   $CASE/identity      "<socket>|<reply>" lines the nvim stub answers with
#   $CASE/hang          sockets the nvim stub never answers on
#   $CASE/registry      the registry DIRECTORY, one file per instance pid
#   $CASE/probed        every socket the nvim stub was asked about
#   $CASE/exec          the argv the nvim-mcp stub was execed with
setup_case() {
  CASE="$work/$1"
  CASE_PATH="$CASE/bin:/usr/bin:/bin:/usr/sbin:/sbin"
  mkdir -p "$CASE/bin" "$CASE/run" "$CASE/registry" "$CASE/tmp"
  # 0700, because the resolver refuses a socket outside a private tree.
  chmod 700 "$CASE/registry" "$CASE/run"
  : >"$CASE/identity"
  : >"$CASE/hang"
  printf '%s' '{"error":{"code":"pane_not_found"},"id":"cli:pane:layout"}' >"$CASE/layout.json"

  cat >"$CASE/bin/herdr" <<STUB
#!/bin/bash
printf '%s\n' "\$*" >>"$CASE/herdr-argv"
[[ "\$1 \$2" == "pane layout" ]] || exit 1
cat "$CASE/layout.json"
STUB

  # nvim --server <socket> --remote-expr <expr>: the identity probe. Every call
  # is logged BEFORE the answer is looked up, which is what case (h) asserts on.
  # %b so an identity reply can carry an escaped newline (case o).
  cat >"$CASE/bin/nvim" <<STUB
#!/bin/bash
sock=""
while [[ \$# -gt 0 ]]; do
  case "\$1" in
    --server)
      sock="\$2"
      shift 2
      ;;
    *) shift ;;
  esac
done
printf '%s\n' "\$sock" >>"$CASE/probed"
if grep -qxF "\$sock" "$CASE/hang"; then
  sleep 3
  exit 0
fi
answer="\$(awk -F'|' -v s="\$sock" '\$1 == s { print \$2; exit }' "$CASE/identity")"
[[ -n \$answer ]] || exit 1
if [[ \$answer == @* ]]; then
  cat "\${answer#@}"
  exit 0
fi
printf '%b' "\$answer"
STUB

  cat >"$CASE/bin/nvim-mcp" <<STUB
#!/bin/bash
printf '%s\n' "\$*" >"$CASE/exec"
STUB

  chmod +x "$CASE/bin/herdr" "$CASE/bin/nvim" "$CASE/bin/nvim-mcp"
}

# private_path <name>=<target>... -- a PATH holding ONLY what is named, plus a
# bash and an env of its own, so an absence fixture cannot be invalidated by
# whatever another host happens to keep in /usr/bin.
private_path() {
  mkdir -p "$CASE/pathbin"
  ln -s /bin/bash "$CASE/pathbin/bash"
  ln -s /usr/bin/env "$CASE/pathbin/env"
  ln -s /usr/bin/mktemp "$CASE/pathbin/mktemp"
  local spec
  for spec in "$@"; do
    ln -s "${spec#*=}" "$CASE/pathbin/${spec%%=*}"
  done
  CASE_PATH="$CASE/pathbin"
}

# record <pane> <pid> <socket> -- one registry file, named for the pid, the way
# the VimEnter autocmd writes it.
record() {
  printf '%s %s %s /repo\n' "$1" "$2" "$3" >"$CASE/registry/$2"
}

# A herdr layout naming <tab> and the panes that follow.
write_layout() { # <tab> <pane...>
  local tab="$1"
  shift
  local panes="" p
  for p in "$@"; do
    panes="$panes{\"pane_id\":\"$p\"},"
  done
  printf '{"id":"cli:pane:layout","result":{"layout":{"panes":[%s],"tab_id":"%s","workspace_id":"w1"},"type":"pane_layout"}}' \
    "${panes%,}" "$tab" >"$CASE/layout.json"
}

# run_case <env assignments...> -- runs the resolver in the current CASE, on
# CASE_PATH. Sets RC; stdout is $CASE/out, stderr is $CASE/err. RC is read by
# the sourcing test, not here.
# shellcheck disable=SC2034
run_case() {
  RC=0
  env -i \
    PATH="$CASE_PATH" \
    HOME="$CASE" \
    NVIM_MCP_REGISTRY="$CASE/registry" \
    NVIM_MCP_BIN="$CASE/bin/nvim-mcp" \
    XDG_RUNTIME_DIR="$CASE/run" \
    TMPDIR="$CASE/tmp" \
    NVIM_MCP_PROBE_DEADLINE=0.1 \
    "$@" \
    bash "$SCRIPT" >"$CASE/out" 2>"$CASE/err" || RC=$?
}
