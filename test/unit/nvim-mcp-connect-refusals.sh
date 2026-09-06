#!/usr/bin/env bash
# nvim-mcp-connect.sh, the REFUSING half: every case where a socket exists or
# is named and the resolver still declines to connect. The guards that run
# before any socket is looked at are in nvim-mcp-connect-guards.sh, the
# resolutions in nvim-mcp-connect-resolution.sh and the sibling cases in
# nvim-mcp-connect-siblings*.sh, so that each file stays inside the one-second
# budget.
#
#   e) a pin nobody answers on   -> exit 3 naming it, never falls through
#   f) a pin that does not exist -> exit 3, no exec
#   i) this pane's socket absent, no sibling -> exit 3 naming the tab and both remedies
#   j) this pane's socket present but its Neovim gone -> exit 3 after probing it
#   k) a probe that never answers -> bounded by the deadline, exit 3
#
set -euo pipefail

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

# --- e) a dead pin is refused, not replaced by discovery ---------------------
# This pane's own socket is live, so falling through would have resolved.
setup_case dead-pin
me term_a
make_socket "$RUN/dead.sock"
live "$(sock term_a)"
run_case XDG_RUNTIME_DIR="$RUN" NVIM_MCP_SOCKET="$RUN/dead.sock"
[[ $RC -eq 3 ]] || fail "dead-pin: expected exit 3, got $RC ($(cat "$CASE/err"))"
grep -qF "$RUN/dead.sock" "$CASE/err" || fail "dead-pin: the refusal does not name the pin ($(cat "$CASE/err"))"
grep -qxF "$RUN/dead.sock" "$CASE/probed" || fail 'dead-pin: the pin was never probed'
[[ ! -f $CASE/exec ]] || fail "dead-pin: it connected anyway ($(cat "$CASE/exec"))"

# --- f) a pin that is not there at all ---------------------------------------
setup_case absent-pin
run_case XDG_RUNTIME_DIR="$RUN" NVIM_MCP_SOCKET="$RUN/nowhere.sock"
[[ $RC -eq 3 ]] || fail "absent-pin: expected exit 3, got $RC ($(cat "$CASE/err"))"
[[ ! -f $CASE/exec ]] || fail 'absent-pin: it connected anyway'

# --- i) no socket for this pane and no sibling -------------------------------
setup_case absent-pane
me term_a
siblings 'w1:t1|term_a|w1:p1'
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 3 ]] || fail "absent-pane: expected exit 3, got $RC ($(cat "$CASE/err"))"
grep -qF 'w1:t1' "$CASE/err" || fail "absent-pane: the refusal does not name the tab ($(cat "$CASE/err"))"
grep -qF '<leader>Cc' "$CASE/err" || fail 'absent-pane: the refusal does not say to launch from Neovim'
grep -qF 'NVIM_MCP_SOCKET' "$CASE/err" || fail 'absent-pane: the refusal does not say to pin'
[[ ! -f $CASE/exec ]] || fail 'absent-pane: it connected anyway'

# --- j) this pane's socket is there but its Neovim is gone -------------------
# A crash leaves the socket file behind; nothing accepts on it.
setup_case dead-pane
me term_a
siblings 'w1:t1|term_a|w1:p1'
make_socket "$(sock term_a)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 3 ]] || fail "dead-pane: expected exit 3, got $RC ($(cat "$CASE/err"))"
grep -qxF "$(sock term_a)" "$CASE/probed" || fail 'dead-pane: the pane socket was never probed'
[[ ! -f $CASE/exec ]] || fail 'dead-pane: it connected to a socket nobody answers on'

# --- k) a probe that never answers is bounded --------------------------------
setup_case probe-hangs
me term_a
siblings 'w1:t1|term_a|w1:p1'
make_socket "$(sock term_a)"
sock term_a >"$CASE/hang"
start="$SECONDS"
# The only case that shortens the deadline, because it is the one waiting for
# the deadline to expire.
CASE_DEADLINE=0.2 run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 3 ]] || fail "probe-hangs: expected exit 3, got $RC ($(cat "$CASE/err"))"
(($((SECONDS - start)) < 2)) || fail 'probe-hangs: the probe was not bounded'
[[ ! -f $CASE/exec ]] || fail 'probe-hangs: it connected to a socket that never answered'

printf 'PASS: %s (5 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
