#!/usr/bin/env bash
# nvim-mcp-connect.sh, the SIBLING GUARDS: what herdr can do wrong on the
# sibling path without the resolver crashing or connecting to the wrong thing.
# The selections themselves live in nvim-mcp-connect-siblings.sh, so that each
# file stays inside the one-second budget.
#
#   t) the tab listing fails      -> no siblings, exit 3
#   u) herdr hangs                -> bounded by the deadline, exit 3
#   v) a sibling terminal id that cannot name a socket never reaches the filesystem
#
set -euo pipefail

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

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

printf 'PASS: %s (3 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
