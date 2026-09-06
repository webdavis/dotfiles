#!/usr/bin/env bash
# nvim-mcp-connect.sh, the GUARDS: every case decided before any socket is
# looked at. The refusals over a named socket live in
# nvim-mcp-connect-refusals.sh, so that each file stays inside the one-second
# budget.
#
#   g) inside herdr, herdr answers nothing -> exit 3 naming the remedy, nothing probed
#   h) herdr reports a terminal that cannot name a socket -> exit 3, nothing probed
#   l) nvim missing from PATH    -> exit 2, the whole diagnostic, nothing touched
#   m) jq missing from PATH      -> exit 2, the whole diagnostic
#   n) nvim cannot say where its run dir is -> exit 2
#   o) a run root other accounts can read -> exit 2 naming owner and mode, nothing probed
#   p) a socket path over the unix limit -> exit 5 naming length and limit, nothing probed
#
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

# --- o) a run root that is not this user's private directory ----------------
# Neovim falls back to <temp>/nvim.<random> when nvim.<user> is mis-owned, and
# <temp> can be a shared /tmp: a socket there is one any account can pre-create.
setup_case loose-root
me term_a
live "$(sock term_a)"
chmod 755 "$RUN"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 2 ]] || fail "loose-root: expected exit 2, got $RC ($(cat "$CASE/err"))"
grep -qF '0700' "$CASE/err" || fail "loose-root: the fault does not name the mode ($(cat "$CASE/err"))"
grep -qF "$RUN" "$CASE/err" || fail "loose-root: the fault does not name the root ($(cat "$CASE/err"))"
[[ ! -e $CASE/probed ]] || fail 'loose-root: a socket in an untrusted root was probed'
[[ ! -f $CASE/exec ]] || fail 'loose-root: it connected anyway'

# --- p) a socket path longer than a unix socket allows -----------------------
# sun_path is 104 bytes on macOS (108 on Linux), NUL included. A root deep
# enough pushes the name past it; the bind would fail with a bare "invalid
# argument" on the editor's side and a probe here would find nothing, so the
# resolver says what happened instead of refusing as if no Neovim existed.
setup_case long-root
RUN="$CASE/run/$(printf 'x%.0s' {1..60})"
mkdir -p "$RUN"
chmod 700 "$RUN"
me term_a
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 5 ]] || fail "long-root: expected exit 5, got $RC ($(cat "$CASE/err"))"
grep -qF "$(printf '%s' "$(sock term_a)" | wc -c | tr -d ' ') bytes" "$CASE/err" ||
  fail "long-root: the message does not name the length ($(cat "$CASE/err"))"
grep -qF 'allow 103' "$CASE/err" || fail "long-root: the message does not name the limit ($(cat "$CASE/err"))"
[[ ! -e $CASE/probed ]] || fail 'long-root: a path over the limit was probed'
[[ ! -f $CASE/exec ]] || fail 'long-root: it connected anyway'

printf 'PASS: %s (7 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
