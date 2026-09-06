#!/usr/bin/env bash
# nvim-mcp-connect.sh, the SIBLING GUARDS: what herdr can do wrong on the
# sibling path without the resolver crashing or connecting to the wrong thing.
# The selections themselves live in nvim-mcp-connect-siblings.sh, so that each
# file stays inside the one-second budget.
#
#   t) the tab listing fails      -> no siblings, exit 3
#   u) herdr hangs                -> bounded by the deadline, exit 3
#   v) a sibling terminal id that cannot name a socket never reaches the filesystem
#   x) a run root with a space in its name survives selection and the picker whole
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

setup_case spaced-root-one
RUN="$CASE/run root"
mkdir "$RUN"
chmod 700 "$RUN"
me term_a
siblings 'w1:t1|term_a|w1:p1' 'w1:t1|term_b|w1:p2'
live "$(sock term_b)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 0 ]] || fail "spaced-root-one: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qxF -- "--connect $RUN/herdr-$SESSION-term_b.sock" "$CASE/exec" ||
  fail "spaced-root-one: the path reached exec cut ($(cat "$CASE/exec" 2>/dev/null))"

printf 'PASS: %s (4 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
