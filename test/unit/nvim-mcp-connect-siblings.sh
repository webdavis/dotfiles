#!/usr/bin/env bash
# nvim-mcp-connect.sh, the SIBLING half: what happens when no Neovim answers
# for the agent's own pane and the resolver asks herdr which panes share the
# tab. The pin, own-pane and fallback cases live in
# nvim-mcp-connect-resolution.sh and nvim-mcp-connect-refusals.sh, so that each
# file stays inside the one-second budget.
#
#   m) one live sibling        -> connected to, by pane socket name
#   n) own pane live too       -> own pane wins, herdr is never asked
#   o) two live siblings       -> picker: exit 4, both enumerated, no guess
#   p) no live sibling         -> exit 3 naming both remedies
#   q) herdr missing from PATH -> no siblings, exit 3, never a crash
#   r) herdr fails             -> no siblings, exit 3
#   s) herdr hangs             -> bounded by the deadline, exit 3
#   t) a sibling id that cannot name a socket never reaches the filesystem
#
set -euo pipefail

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

# --- m) one live sibling is connected to -------------------------------------
setup_case one-sibling
write_layout w1:t1 w1:p1 w1:p2
live "$CASE/run/herdr-pane-w1.p2.sock"
run_case HERDR_PANE_ID=w1:p1 XDG_RUNTIME_DIR="$CASE/run"
[[ $RC -eq 0 ]] || fail "one-sibling: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qxF -- "--connect $CASE/run/herdr-pane-w1.p2.sock" "$CASE/exec" ||
  fail "one-sibling: wrong socket ($(cat "$CASE/exec" 2>/dev/null))"
grep -qxF 'pane layout --pane w1:p1' "$CASE/herdr-argv" ||
  fail "one-sibling: herdr was not asked for THIS pane's layout ($(cat "$CASE/herdr-argv" 2>/dev/null))"

# --- n) own pane wins, herdr is never consulted ------------------------------
setup_case own-wins
write_layout w1:t1 w1:p1 w1:p2
live "$CASE/run/herdr-pane-w1.p1.sock" "$CASE/run/herdr-pane-w1.p2.sock"
run_case HERDR_PANE_ID=w1:p1 XDG_RUNTIME_DIR="$CASE/run"
[[ $RC -eq 0 ]] || fail "own-wins: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qxF -- "--connect $CASE/run/herdr-pane-w1.p1.sock" "$CASE/exec" ||
  fail "own-wins: a sibling was chosen over the own pane ($(cat "$CASE/exec" 2>/dev/null))"
[[ ! -e $CASE/herdr-argv ]] || fail 'own-wins: herdr was consulted although the own pane answered'

# --- o) two live siblings are a picker, not a guess --------------------------
setup_case two-siblings
write_layout w1:t1 w1:p1 w1:p2 w1:p3
live "$CASE/run/herdr-pane-w1.p2.sock" "$CASE/run/herdr-pane-w1.p3.sock"
run_case HERDR_PANE_ID=w1:p1 XDG_RUNTIME_DIR="$CASE/run"
[[ $RC -eq 4 ]] || fail "two-siblings: expected exit 4, got $RC ($(cat "$CASE/err"))"
grep -qF "$CASE/run/herdr-pane-w1.p2.sock" "$CASE/err" || fail 'two-siblings: w1:p2 is not enumerated'
grep -qF "$CASE/run/herdr-pane-w1.p3.sock" "$CASE/err" || fail 'two-siblings: w1:p3 is not enumerated'
grep -qF 'pid 4242' "$CASE/err" || fail "two-siblings: the enumeration carries no pid ($(cat "$CASE/err"))"
grep -qF 'NVIM_MCP_SOCKET' "$CASE/err" || fail 'two-siblings: the picker does not say how to choose'
[[ ! -f $CASE/exec ]] || fail "two-siblings: it guessed ($(cat "$CASE/exec"))"

# --- p) no live sibling refuses with both remedies ---------------------------
setup_case no-sibling
write_layout w1:t1 w1:p1 w1:p2
make_socket "$CASE/run/herdr-pane-w1.p2.sock"
run_case HERDR_PANE_ID=w1:p1 XDG_RUNTIME_DIR="$CASE/run"
[[ $RC -eq 3 ]] || fail "no-sibling: expected exit 3, got $RC ($(cat "$CASE/err"))"
grep -qF '<leader>Cc' "$CASE/err" || fail 'no-sibling: the refusal does not say to launch from Neovim'
grep -qF 'NVIM_MCP_SOCKET' "$CASE/err" || fail 'no-sibling: the refusal does not say to pin'
grep -qxF "$CASE/run/herdr-pane-w1.p2.sock" "$CASE/probed" || fail 'no-sibling: the dead sibling was never probed'
[[ ! -f $CASE/exec ]] || fail 'no-sibling: it connected anyway'

# --- q) herdr missing is no siblings, never a crash --------------------------
setup_case no-herdr
private_path "$work/bin/nvim" "$work/bin/nvim-mcp"
run_case HERDR_PANE_ID=w1:p1 XDG_RUNTIME_DIR="$CASE/run"
[[ $RC -eq 3 ]] || fail "no-herdr: expected exit 3, got $RC ($(cat "$CASE/err"))"
grep -qF 'NVIM_MCP_SOCKET' "$CASE/err" || fail "no-herdr: not the refusal ($(cat "$CASE/err"))"
[[ ! -f $CASE/exec ]] || fail 'no-herdr: it connected anyway'

# --- r) a failing herdr is no siblings ----------------------------------------
setup_case herdr-fails
write_layout w1:t1 w1:p1 w1:p2
live "$CASE/run/herdr-pane-w1.p2.sock"
: >"$CASE/herdr-fail"
run_case HERDR_PANE_ID=w1:p1 XDG_RUNTIME_DIR="$CASE/run"
[[ $RC -eq 3 ]] || fail "herdr-fails: expected exit 3, got $RC ($(cat "$CASE/err"))"
[[ ! -f $CASE/exec ]] || fail 'herdr-fails: it connected on a herdr failure'

# --- s) a hanging herdr is bounded --------------------------------------------
setup_case herdr-hangs
write_layout w1:t1 w1:p1 w1:p2
: >"$CASE/herdr-hang"
start="$SECONDS"
CASE_DEADLINE=0.2 run_case HERDR_PANE_ID=w1:p1 XDG_RUNTIME_DIR="$CASE/run"
[[ $RC -eq 3 ]] || fail "herdr-hangs: expected exit 3, got $RC ($(cat "$CASE/err"))"
(($((SECONDS - start)) < 2)) || fail 'herdr-hangs: the herdr call was not bounded'
[[ ! -f $CASE/exec ]] || fail 'herdr-hangs: it connected anyway'

# --- t) a sibling id that cannot name a socket never reaches the filesystem --
# An id carrying a slash would derive a path OUTSIDE the run root
# (`<root>/herdr-pane-w1.p9/../../escape.sock` is `<case>/escape.sock`), and a
# Neovim answering there must not be reached through it. The `..` resolves only
# when a directory of that first name stands in the root, so the case plants
# one, and the stub answers on the derived spelling too: only the pattern check
# stands between the id and the socket.
setup_case odd-sibling
write_layout w1:t1 w1:p1 'w1:p9/../../escape'
mkdir "$CASE/run/herdr-pane-w1.p9"
live "$CASE/escape.sock"
printf '%s\n' "$CASE/run/herdr-pane-w1.p9/../../escape.sock" >>"$CASE/live"
run_case HERDR_PANE_ID=w1:p1 XDG_RUNTIME_DIR="$CASE/run"
[[ $RC -eq 3 ]] || fail "odd-sibling: expected exit 3, got $RC ($(cat "$CASE/err") $(cat "$CASE/exec" 2>/dev/null))"
grep -qF 'escape' "$CASE/probed" 2>/dev/null && fail "odd-sibling: a path was derived from an unsafe id ($(cat "$CASE/probed"))"
[[ ! -f $CASE/exec ]] || fail "odd-sibling: it connected outside the run root ($(cat "$CASE/exec"))"

printf 'PASS: %s (8 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
