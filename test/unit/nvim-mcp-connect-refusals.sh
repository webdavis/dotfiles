#!/usr/bin/env bash
# nvim-mcp-connect.sh, the REFUSING half: every case that ends with the resolver
# declining to connect, and the guards that run before it looks at anything.
# The resolutions live in nvim-mcp-connect-resolution.sh, so that each file
# stays inside the one-second budget.
#
#   e) a pin nobody answers on   -> exit 3 naming it, never falls through to the pane
#   f) a pin that does not exist -> exit 3, no exec
#   g) a pane id that cannot name a socket -> exit 3, nothing probed or queried
#   h) a pane whose socket is absent       -> exit 3 naming the pane and both remedies
#   i) a pane socket nobody answers on (listener gone) -> exit 3, no exec
#   j) a probe that never answers -> bounded by the deadline, exit 3
#   k) nvim missing from PATH     -> exit 2, the whole diagnostic, nothing touched
#   l) nvim cannot say where its run dir is -> exit 2
#
set -euo pipefail

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

# --- e) a dead pin is refused, not replaced by discovery ---------------------
# The pane socket is live, so falling through would have resolved.
setup_case dead-pin
make_socket "$CASE/run/dead.sock"
live "$CASE/run/herdr-pane-w1.p2.sock"
run_case HERDR_PANE_ID=w1:p2 XDG_RUNTIME_DIR="$CASE/run" NVIM_MCP_SOCKET="$CASE/run/dead.sock"
[[ $RC -eq 3 ]] || fail "dead-pin: expected exit 3, got $RC ($(cat "$CASE/err"))"
grep -qF "$CASE/run/dead.sock" "$CASE/err" || fail "dead-pin: the refusal does not name the pin ($(cat "$CASE/err"))"
grep -qxF "$CASE/run/dead.sock" "$CASE/probed" || fail 'dead-pin: the pin was never probed'
[[ ! -f $CASE/exec ]] || fail "dead-pin: it connected anyway ($(cat "$CASE/exec"))"

# --- f) a pin that is not there at all ---------------------------------------
setup_case absent-pin
run_case XDG_RUNTIME_DIR="$CASE/run" NVIM_MCP_SOCKET="$CASE/run/nowhere.sock"
[[ $RC -eq 3 ]] || fail "absent-pin: expected exit 3, got $RC ($(cat "$CASE/err"))"
[[ ! -f $CASE/exec ]] || fail 'absent-pin: it connected anyway'

# --- g) a pane id that cannot sit in a socket path ---------------------------
# Refused before nvim is asked anything: nothing derived from it may reach the
# filesystem.
for unsafe in '../w1:p2' 'w1:p2/x' 'w1 p2' "$(printf 'a%.0s' {1..65})"; do
  setup_case unsafe-id
  run_case HERDR_PANE_ID="$unsafe" XDG_RUNTIME_DIR="$CASE/run"
  [[ $RC -eq 3 ]] || fail "unsafe-id '$unsafe': expected exit 3, got $RC ($(cat "$CASE/err"))"
  grep -q 'HERDR_PANE_ID' "$CASE/err" || fail "unsafe-id '$unsafe': the refusal does not name the variable"
  [[ ! -e $CASE/probed && ! -e $CASE/queried ]] || fail "unsafe-id '$unsafe': nvim was consulted"
  [[ ! -f $CASE/exec ]] || fail "unsafe-id '$unsafe': it connected anyway"
done

# --- h) no socket for this pane ----------------------------------------------
setup_case absent-pane
run_case HERDR_PANE_ID=w1:p2 XDG_RUNTIME_DIR="$CASE/run"
[[ $RC -eq 3 ]] || fail "absent-pane: expected exit 3, got $RC ($(cat "$CASE/err"))"
grep -qF 'w1:p2' "$CASE/err" || fail "absent-pane: the refusal does not name the pane ($(cat "$CASE/err"))"
grep -qF '<leader>Cc' "$CASE/err" || fail 'absent-pane: the refusal does not say to launch from Neovim'
grep -qF 'NVIM_MCP_SOCKET' "$CASE/err" || fail 'absent-pane: the refusal does not say to pin'
[[ ! -f $CASE/exec ]] || fail 'absent-pane: it connected anyway'

# --- i) the pane socket is there but its Neovim is gone ----------------------
# A crash leaves the socket file behind; nothing accepts on it.
setup_case dead-pane
make_socket "$CASE/run/herdr-pane-w1.p2.sock"
run_case HERDR_PANE_ID=w1:p2 XDG_RUNTIME_DIR="$CASE/run"
[[ $RC -eq 3 ]] || fail "dead-pane: expected exit 3, got $RC ($(cat "$CASE/err"))"
grep -qxF "$CASE/run/herdr-pane-w1.p2.sock" "$CASE/probed" || fail 'dead-pane: the pane socket was never probed'
[[ ! -f $CASE/exec ]] || fail 'dead-pane: it connected to a socket nobody answers on'

# --- j) a probe that never answers is bounded --------------------------------
setup_case probe-hangs
make_socket "$CASE/run/herdr-pane-w1.p2.sock"
printf '%s\n' "$CASE/run/herdr-pane-w1.p2.sock" >"$CASE/hang"
start="$SECONDS"
# The only case that shortens the deadline, because it is the one waiting for
# the deadline to expire.
CASE_DEADLINE=0.2 run_case HERDR_PANE_ID=w1:p2 XDG_RUNTIME_DIR="$CASE/run"
[[ $RC -eq 3 ]] || fail "probe-hangs: expected exit 3, got $RC ($(cat "$CASE/err"))"
(($((SECONDS - start)) < 2)) || fail 'probe-hangs: the probe was not bounded'
[[ ! -f $CASE/exec ]] || fail 'probe-hangs: it connected to a socket that never answered'

# --- k) nvim missing: exit 2 naming it, before anything else -----------------
setup_case no-nvim
live "$CASE/run/herdr-pane-w1.p2.sock"
private_path "$work/bin/nvim-mcp"
run_case HERDR_PANE_ID=w1:p2 XDG_RUNTIME_DIR="$CASE/run"
[[ $RC -eq 2 ]] || fail "no-nvim: expected exit 2, got $RC ($(cat "$CASE/err"))"
grep -qxF 'nvim-mcp-connect: nvim is not on PATH, and the resolver needs it' "$CASE/err" ||
  fail "no-nvim: wrong diagnostic ($(cat "$CASE/err"))"
[[ ! -f $CASE/exec ]] || fail 'no-nvim: the server was run anyway'

# --- l) nvim answers the run-dir query with nothing usable -------------------
# An empty XDG_RUNTIME_DIR is one way to get there: Neovim then reports an
# empty stdpath("run") and starts no server at all.
setup_case no-run-dir
: >"$CASE/rundir"
run_case HERDR_PANE_ID=w1:p2
[[ $RC -eq 2 ]] || fail "no-run-dir: expected exit 2, got $RC ($(cat "$CASE/err"))"
grep -qF 'run dir' "$CASE/err" || fail "no-run-dir: the fault does not say what was missing ($(cat "$CASE/err"))"
[[ ! -e $CASE/probed ]] || fail 'no-run-dir: something was probed with no root to look in'
[[ ! -f $CASE/exec ]] || fail 'no-run-dir: the server was run anyway'

printf 'PASS: %s (11 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
