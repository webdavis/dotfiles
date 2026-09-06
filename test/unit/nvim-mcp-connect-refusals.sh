#!/usr/bin/env bash
# nvim-mcp-connect.sh, the REFUSING half: every case that ends with the resolver
# declining to connect, and the guards that run before it looks at anything.
# The resolutions live in nvim-mcp-connect-resolution.sh and the sibling cases
# in nvim-mcp-connect-siblings.sh, so that each file stays inside the
# one-second budget.
#
#   e) a pin nobody answers on   -> exit 3 naming it, never falls through
#   f) a pin that does not exist -> exit 3, no exec
#   g) inside herdr, herdr answers nothing -> exit 3 naming the remedy, nothing probed
#   h) herdr reports a terminal that cannot name a socket -> exit 3, nothing probed
#   i) this pane's socket absent, no sibling -> exit 3 naming the tab and both remedies
#   j) this pane's socket present but its Neovim gone -> exit 3 after probing it
#   k) a probe that never answers -> bounded by the deadline, exit 3
#   l) nvim missing from PATH    -> exit 2, the whole diagnostic, nothing touched
#   m) jq missing from PATH      -> exit 2, the whole diagnostic
#   n) nvim cannot say where its run dir is -> exit 2
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

# --- g) inside herdr, a herdr that answers nothing is a refusal --------------
# No me.json: herdr exits 1 with nothing, as it does for a pane it does not
# know. Inside herdr that is not a case for `--connect auto`, which would start
# a server attached to nothing.
setup_case herdr-silent
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 3 ]] || fail "herdr-silent: expected exit 3, got $RC ($(cat "$CASE/err"))"
grep -qF 'NVIM_MCP_SOCKET' "$CASE/err" || fail "herdr-silent: the refusal does not name the remedy ($(cat "$CASE/err"))"
[[ ! -e $CASE/probed && ! -e $CASE/queried ]] || fail 'herdr-silent: nvim was consulted with no pane to name'
[[ ! -f $CASE/exec ]] || fail 'herdr-silent: it connected anyway'

# --- h) a terminal id that cannot sit in a socket path -----------------------
# An id carrying a slash would derive a path OUTSIDE the run root
# (`<root>/herdr-<session>-x/../../escape.sock` is `<case>/escape.sock`). The
# `..` resolves only when a directory of that first name stands in the root, so
# the case plants one, and the stub answers on the derived spelling too: only
# the pattern check stands between herdr's answer and that socket.
setup_case odd-terminal
me 'x/../../escape'
mkdir "$RUN/herdr-$SESSION-x"
live "$CASE/escape.sock"
printf '%s\n' "$RUN/herdr-$SESSION-x/../../escape.sock" >>"$CASE/live"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 3 ]] || fail "odd-terminal: expected exit 3, got $RC ($(cat "$CASE/err") $(cat "$CASE/exec" 2>/dev/null))"
grep -qF 'escape' "$CASE/probed" 2>/dev/null && fail "odd-terminal: a path was derived from an unsafe id ($(cat "$CASE/probed"))"
[[ ! -f $CASE/exec ]] || fail 'odd-terminal: it connected outside the run root'

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

# --- l) nvim missing: exit 2 naming it, before anything else -----------------
setup_case no-nvim
me term_a
live "$(sock term_a)"
private_path "$JQ_PATH" "$work/bin/herdr" "$work/bin/nvim-mcp"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 2 ]] || fail "no-nvim: expected exit 2, got $RC ($(cat "$CASE/err"))"
grep -qxF 'nvim-mcp-connect: nvim is not on PATH, and the resolver needs it' "$CASE/err" ||
  fail "no-nvim: wrong diagnostic ($(cat "$CASE/err"))"
[[ ! -e $CASE/herdr-argv ]] || fail 'no-nvim: herdr was consulted'
[[ ! -f $CASE/exec ]] || fail 'no-nvim: the server was run anyway'

# --- m) jq missing: exit 2 naming it -----------------------------------------
setup_case no-jq
me term_a
live "$(sock term_a)"
private_path "$work/bin/nvim" "$work/bin/herdr" "$work/bin/nvim-mcp"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 2 ]] || fail "no-jq: expected exit 2, got $RC ($(cat "$CASE/err"))"
grep -qxF 'nvim-mcp-connect: jq is not on PATH, and the resolver needs it' "$CASE/err" ||
  fail "no-jq: wrong diagnostic ($(cat "$CASE/err"))"
[[ ! -f $CASE/exec ]] || fail 'no-jq: the server was run anyway'

# --- n) nvim answers the run-dir query with nothing usable -------------------
# An empty XDG_RUNTIME_DIR is one way to get there: Neovim then reports an
# empty stdpath("run") and starts no server at all.
setup_case no-run-dir
me term_a
: >"$CASE/rundir"
run_case
[[ $RC -eq 2 ]] || fail "no-run-dir: expected exit 2, got $RC ($(cat "$CASE/err"))"
grep -qF 'run dir' "$CASE/err" || fail "no-run-dir: the fault does not say what was missing ($(cat "$CASE/err"))"
[[ ! -e $CASE/probed ]] || fail 'no-run-dir: something was probed with no root to look in'
[[ ! -f $CASE/exec ]] || fail 'no-run-dir: the server was run anyway'

printf 'PASS: %s (10 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
