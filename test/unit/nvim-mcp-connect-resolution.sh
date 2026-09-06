#!/usr/bin/env bash
# nvim-mcp-connect.sh, the RESOLVING half: every case that ends with the server
# exec'd against a socket without consulting the tab. The refusals and the
# guards live in nvim-mcp-connect-refusals.sh and the sibling cases in
# nvim-mcp-connect-siblings.sh, so that each file stays inside the one-second
# budget.
#
#   a) a live NVIM_MCP_SOCKET pin wins, and herdr is never asked
#   b) herdr's terminal for this pane names the socket, under XDG_RUNTIME_DIR,
#      with one nvim query (the session hash) and no tab listing
#   c) without XDG_RUNTIME_DIR the run root is the PARENT of what nvim reports
#      as stdpath("run")
#   d) outside herdr, with herdr answering nothing, nvim-mcp's own
#      `--connect auto` is used and nvim is never consulted
#   x) a run root with a space in its name reaches exec whole
#
set -euo pipefail

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

# --- a) a live pin wins over the pane ----------------------------------------
setup_case pin-wins
me term_a
live "$RUN/pinned.sock" "$(sock term_a)"
run_case XDG_RUNTIME_DIR="$RUN" NVIM_MCP_SOCKET="$RUN/pinned.sock"
[[ $RC -eq 0 ]] || fail "pin-wins: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qxF -- "--connect $RUN/pinned.sock" "$CASE/exec" ||
  fail "pin-wins: the server was not run against the pin ($(cat "$CASE/exec" 2>/dev/null))"
[[ ! -e $CASE/herdr-argv ]] || fail 'pin-wins: herdr was asked although a pin was set'

# --- b) herdr's terminal for this pane names the socket ----------------------
# term_65a9c8766b9261 is the shape 0.8.2 reports. The socket carries the
# session hash too, so an editor in another herdr session cannot be reached
# by a name that happens to repeat there.
setup_case own-pane
me term_65a9c8766b9261
live "$(sock term_65a9c8766b9261)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 0 ]] || fail "own-pane: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qxF -- "--connect $RUN/herdr-9a663d-term_65a9c8766b9261.sock" "$CASE/exec" ||
  fail "own-pane: wrong socket ($(cat "$CASE/exec" 2>/dev/null))"
[[ "$(cat "$CASE/herdr-argv")" == 'pane current --current' ]] ||
  fail "own-pane: herdr was asked more than this pane's identity ($(cat "$CASE/herdr-argv"))"
[[ "$(wc -l <"$CASE/queried" | tr -d ' ')" == 1 ]] ||
  fail "own-pane: nvim was asked more than once ($(cat "$CASE/queried" 2>/dev/null))"

# --- c) the run root is asked from nvim and is the parent of its answer ------
# stdpath("run") is per process on 0.12 ($TMPDIR/nvim.<user>/<random>), so the
# shared root is its parent. The stub reports $RUN/a1b2c3.
setup_case run-root
me term_a
live "$(sock term_a)"
run_case
[[ $RC -eq 0 ]] || fail "run-root: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qxF -- "--connect $(sock term_a)" "$CASE/exec" || fail "run-root: wrong socket ($(cat "$CASE/exec" 2>/dev/null))"
[[ -s $CASE/queried ]] || fail 'run-root: nvim was never asked for the run dir'

# --- d) outside herdr, nothing to resolve from falls back to auto ------------
setup_case auto
run_case HERDR_ENV=
[[ $RC -eq 0 ]] || fail "auto: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qxF -- "--connect auto" "$CASE/exec" || fail "auto: wrong argv ($(cat "$CASE/exec" 2>/dev/null))"
[[ ! -e $CASE/probed && ! -e $CASE/queried ]] || fail 'auto: nvim was consulted with nothing to resolve from'

# --- x) a run root with a space in its name reaches exec whole ---------------
# Candidates are carried in arrays, not one space-delimited string: a socket
# under "run root" must reach exec whole, never cut at the space.
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

printf 'PASS: %s (5 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
