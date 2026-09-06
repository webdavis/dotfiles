#!/usr/bin/env bash
# nvim-mcp-connect.sh, the SIBLING half: what happens when no Neovim answers
# for the agent's own pane and the resolver asks herdr which panes share the
# tab. The pin, own-pane and fallback cases live in
# nvim-mcp-connect-resolution.sh and nvim-mcp-connect-refusals.sh, so that each
# file stays inside the one-second budget.
#
#   o) one live sibling           -> connected to, by its terminal's socket
#   p) own pane live too          -> own pane wins, the tab is never listed
#   q) two live siblings          -> picker: exit 4, both enumerated, no guess
#   r) no live sibling            -> exit 3 after probing the dead one
#   s) a live Neovim in ANOTHER tab of the workspace is not a candidate
#   t) the tab listing fails      -> no siblings, exit 3
#   u) herdr hangs                -> bounded by the deadline, exit 3
#   v) a sibling terminal id that cannot name a socket never reaches the filesystem
#   w) a sibling moved between workspaces keeps its terminal, so it is still found
#
set -euo pipefail

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

# --- o) one live sibling is connected to -------------------------------------
setup_case one-sibling
me term_a
siblings 'w1:t1|term_a|w1:p1' 'w1:t1|term_b|w1:p2'
live "$(sock term_b)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 0 ]] || fail "one-sibling: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qxF -- "--connect $(sock term_b)" "$CASE/exec" || fail "one-sibling: wrong socket ($(cat "$CASE/exec" 2>/dev/null))"
[[ "$(cat "$CASE/herdr-argv")" == $'pane current --current\npane list --workspace w1' ]] ||
  fail "one-sibling: herdr was not asked this pane's identity, then its workspace ($(cat "$CASE/herdr-argv"))"

# --- p) own pane wins, the tab is never listed -------------------------------
setup_case own-wins
me term_a
siblings 'w1:t1|term_a|w1:p1' 'w1:t1|term_b|w1:p2'
live "$(sock term_a)" "$(sock term_b)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 0 ]] || fail "own-wins: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qxF -- "--connect $(sock term_a)" "$CASE/exec" ||
  fail "own-wins: a sibling was chosen over the own pane ($(cat "$CASE/exec" 2>/dev/null))"
grep -q 'pane list' "$CASE/herdr-argv" && fail 'own-wins: the tab was listed although the own pane answered'

# --- q) two live siblings are a picker, not a guess --------------------------
setup_case two-siblings
me term_a
siblings 'w1:t1|term_a|w1:p1' 'w1:t1|term_b|w1:p2' 'w1:t1|term_c|w1:p3'
live "$(sock term_b)" "$(sock term_c)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 4 ]] || fail "two-siblings: expected exit 4, got $RC ($(cat "$CASE/err"))"
grep -qF -- "$(sock term_b)  pane w1:p2  pid 4242" "$CASE/err" || fail "two-siblings: w1:p2 is not enumerated ($(cat "$CASE/err"))"
grep -qF -- "$(sock term_c)  pane w1:p3  pid 4242" "$CASE/err" || fail "two-siblings: w1:p3 is not enumerated ($(cat "$CASE/err"))"
grep -qF 'NVIM_MCP_SOCKET' "$CASE/err" || fail 'two-siblings: the picker does not say how to choose'
[[ ! -f $CASE/exec ]] || fail "two-siblings: it guessed ($(cat "$CASE/exec"))"

# --- r) no live sibling refuses after probing --------------------------------
setup_case no-sibling
me term_a
siblings 'w1:t1|term_a|w1:p1' 'w1:t1|term_b|w1:p2'
make_socket "$(sock term_b)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 3 ]] || fail "no-sibling: expected exit 3, got $RC ($(cat "$CASE/err"))"
grep -qxF "$(sock term_b)" "$CASE/probed" || fail 'no-sibling: the dead sibling was never probed'
[[ ! -f $CASE/exec ]] || fail 'no-sibling: it connected anyway'

# --- s) another tab of the workspace is not a candidate ----------------------
setup_case other-tab
me term_a
siblings 'w1:t1|term_a|w1:p1' 'w1:t2|term_c|w1:p5'
live "$(sock term_c)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 3 ]] || fail "other-tab: expected exit 3, got $RC ($(cat "$CASE/err"))"
grep -qxF "$(sock term_c)" "$CASE/probed" 2>/dev/null && fail 'other-tab: a Neovim in another tab was probed'
[[ ! -f $CASE/exec ]] || fail 'other-tab: it connected across tabs'

# --- t) a failing tab listing is no siblings ---------------------------------
setup_case list-fails
me term_a
siblings 'w1:t1|term_a|w1:p1' 'w1:t1|term_b|w1:p2'
live "$(sock term_b)"
: >"$CASE/herdr-list-fail"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 3 ]] || fail "list-fails: expected exit 3, got $RC ($(cat "$CASE/err"))"
[[ ! -f $CASE/exec ]] || fail 'list-fails: it connected on a herdr failure'

# --- u) a hanging herdr is bounded --------------------------------------------
setup_case herdr-hangs
me term_a
: >"$CASE/herdr-hang"
start="$SECONDS"
CASE_DEADLINE=0.2 run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 3 ]] || fail "herdr-hangs: expected exit 3, got $RC ($(cat "$CASE/err"))"
(($((SECONDS - start)) < 2)) || fail 'herdr-hangs: the herdr call was not bounded'
[[ ! -f $CASE/exec ]] || fail 'herdr-hangs: it connected anyway'

# --- v) a sibling terminal id that cannot name a socket ----------------------
# An id carrying a slash would derive a path OUTSIDE the run root
# (`<root>/herdr-<session>-x/../../escape.sock` is `<case>/escape.sock`), and
# a Neovim answering there must not be reached through it. The `..` resolves
# only when a directory of that first name stands in the root, so the case
# plants one, and the stub answers on the derived spelling too: only the
# pattern check stands between the id and the socket.
setup_case odd-sibling
me term_a
siblings 'w1:t1|term_a|w1:p1' 'w1:t1|x/../../escape|w1:p2'
mkdir "$RUN/herdr-$SESSION-x"
live "$CASE/escape.sock"
printf '%s\n' "$RUN/herdr-$SESSION-x/../../escape.sock" >>"$CASE/live"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 3 ]] || fail "odd-sibling: expected exit 3, got $RC ($(cat "$CASE/err") $(cat "$CASE/exec" 2>/dev/null))"
grep -qF 'escape' "$CASE/probed" 2>/dev/null && fail "odd-sibling: a path was derived from an unsafe id ($(cat "$CASE/probed"))"
[[ ! -f $CASE/exec ]] || fail "odd-sibling: it connected outside the run root ($(cat "$CASE/exec"))"

# --- w) a sibling moved between workspaces is still found by its terminal ----
# herdr renames a pane moved across workspaces (w1:p2 becomes, say, w9:p2) but
# its terminal id and its socket do not change. The listing names the pane by
# its NEW id and the same terminal; the resolver must not care about the id.
setup_case moved-sibling
me term_a
siblings 'w1:t1|term_a|w1:p1' 'w1:t1|term_b|w9:p2'
live "$(sock term_b)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 0 ]] || fail "moved-sibling: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qxF -- "--connect $(sock term_b)" "$CASE/exec" || fail "moved-sibling: wrong socket ($(cat "$CASE/exec" 2>/dev/null))"

printf 'PASS: %s (9 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
