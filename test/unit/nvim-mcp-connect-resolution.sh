#!/usr/bin/env bash
# nvim-mcp-connect.sh, the RESOLVING half: every case that ends with the server
# exec'd against a socket. The refusals and the guards live in
# nvim-mcp-connect-refusals.sh, so that each file stays inside the one-second
# budget.
#
#   a) a live NVIM_MCP_SOCKET pin wins, and the pane socket is never probed
#   b) HERDR_PANE_ID resolves to the live pane socket, colon written as a dot,
#      under XDG_RUNTIME_DIR, with no run-dir query
#   c) without XDG_RUNTIME_DIR the run root is the PARENT of what nvim reports
#      as stdpath("run")
#   d) no pane id and no pin falls back to nvim-mcp's own `--connect auto`
#
set -euo pipefail

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

# --- a) a live pin wins over the pane ----------------------------------------
# Both are live, so the only thing that can pick the pin is the order.
setup_case pin-wins
live "$CASE/run/pinned.sock" "$CASE/run/herdr-pane-w1.p2.sock"
run_case HERDR_PANE_ID=w1:p2 XDG_RUNTIME_DIR="$CASE/run" NVIM_MCP_SOCKET="$CASE/run/pinned.sock"
[[ $RC -eq 0 ]] || fail "pin-wins: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qxF -- "--connect $CASE/run/pinned.sock" "$CASE/exec" ||
  fail "pin-wins: the server was not run against the pin ($(cat "$CASE/exec" 2>/dev/null))"
grep -qF "herdr-pane" "$CASE/probed" && fail 'pin-wins: the pane socket was probed although a pin was set'

# --- b) the pane id names the socket -----------------------------------------
# w1:p2 must reach herdr-pane-w1.p2.sock: serverstart() reads a colon as a TCP
# address, so the Neovim side writes it as a dot and the resolver must agree.
setup_case pane
live "$CASE/run/herdr-pane-w1.p2.sock"
run_case HERDR_PANE_ID=w1:p2 XDG_RUNTIME_DIR="$CASE/run"
[[ $RC -eq 0 ]] || fail "pane: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qxF -- "--connect $CASE/run/herdr-pane-w1.p2.sock" "$CASE/exec" ||
  fail "pane: wrong socket ($(cat "$CASE/exec" 2>/dev/null))"
[[ ! -e $CASE/queried ]] || fail 'pane: nvim was asked for the run dir although XDG_RUNTIME_DIR was set'

# --- c) the run root is asked from nvim and is the parent of its answer ------
# stdpath("run") is per process on 0.12 ($TMPDIR/nvim.<user>/<random>), so the
# shared root is its parent. The stub reports $CASE/run/a1b2c3.
setup_case run-root
live "$CASE/run/herdr-pane-w1.p2.sock"
run_case HERDR_PANE_ID=w1:p2
[[ $RC -eq 0 ]] || fail "run-root: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qxF -- "--connect $CASE/run/herdr-pane-w1.p2.sock" "$CASE/exec" ||
  fail "run-root: wrong socket ($(cat "$CASE/exec" 2>/dev/null))"
[[ -s $CASE/queried ]] || fail 'run-root: nvim was never asked for the run dir'

# --- d) nothing to resolve from falls back to auto ---------------------------
setup_case auto
run_case
[[ $RC -eq 0 ]] || fail "auto: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qxF -- "--connect auto" "$CASE/exec" || fail "auto: wrong argv ($(cat "$CASE/exec" 2>/dev/null))"
[[ ! -e $CASE/probed && ! -e $CASE/queried ]] || fail 'auto: nvim was consulted with nothing to resolve from'

printf 'PASS: %s (4 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
