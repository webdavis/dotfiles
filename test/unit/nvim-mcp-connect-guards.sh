#!/usr/bin/env bash
set -euo pipefail

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

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

# A partial caller identity cannot safely ask --current: herdr falls back to
# the focused pane when HERDR_PANE_ID is absent. Each field is required before
# any query or socket probe, even if a focused editor would answer.
for missing in HERDR_ENV HERDR_PANE_ID HERDR_SOCKET_PATH; do
  setup_case "missing-$missing"
  me term_focused
  live "$(sock term_focused)"
  run_case XDG_RUNTIME_DIR="$RUN" "$missing="
  [[ $RC -eq 3 ]] || fail "missing-$missing: expected exit 3, got $RC ($(cat "$CASE/err"))"
  grep -qF 'NVIM_MCP_SOCKET' "$CASE/err" || fail "missing-$missing: no pin guidance"
  [[ ! -e $CASE/herdr-argv && ! -e $CASE/probed && ! -e $CASE/queried ]] || fail "missing-$missing: queried with incomplete identity"
  [[ ! -f $CASE/exec ]] || fail "missing-$missing: connected by focus"
done

printf 'PASS: %s (7 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
