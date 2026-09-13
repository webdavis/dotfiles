#!/usr/bin/env bash
set -euo pipefail

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

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

# --- x) a run root with a space in its name ----------------------------------
# Candidates are carried in arrays, not one space-delimited string: a socket
# under "run root" must reach exec and the picker whole, never cut at the space.
setup_case spaced-root
RUN="$CASE/run root"
mkdir "$RUN"
chmod 700 "$RUN"
me term_a
siblings 'w1:t1|term_a|w1:p1' 'w1:t1|term_b|w1:p2' 'w1:t1|term_c|w1:p3'
live "$(sock term_b)" "$(sock term_c)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 4 ]] || fail "spaced-root: expected exit 4, got $RC ($(cat "$CASE/err"))"
grep -qF -- "  $(sock term_b)  pane w1:p2  pid 4242" "$CASE/err" ||
  fail "spaced-root: the picker cut or mislabelled the path ($(cat "$CASE/err"))"
grep -qF -- "  $(sock term_c)  pane w1:p3  pid 4242" "$CASE/err" ||
  fail "spaced-root: the picker cut or mislabelled the path ($(cat "$CASE/err"))"
[[ ! -f $CASE/exec ]] || fail "spaced-root: it guessed ($(cat "$CASE/exec"))"

printf 'PASS: %s (4 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
