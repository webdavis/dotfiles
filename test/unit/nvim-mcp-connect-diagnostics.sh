#!/usr/bin/env bash
# The terminal diagnostic uses the resolver's normal selection and refusal paths,
# but never starts the MCP server. All sockets and commands below are private fixtures.
set -euo pipefail

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

setup_case diagnose-pin
CASE_OPTION=--diagnose
live "$RUN/pinned.sock"
run_case NVIM_MCP_SOCKET="$RUN/pinned.sock"
[[ $RC -eq 0 && $(cat "$CASE/out") == "$RUN/pinned.sock" && ! -e $CASE/exec ]] ||
  fail 'diagnose-pin: expected the pin without starting the server'
[[ ! -e $CASE/herdr-argv ]] || fail 'diagnose-pin: queried herdr despite the pin'

setup_case diagnose-own
CASE_OPTION=--diagnose
me term_a
live "$(sock term_a)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 0 && $(cat "$CASE/out") == "$(sock term_a)" && ! -e $CASE/exec ]] ||
  fail 'diagnose-own: expected this pane without starting the server'

setup_case diagnose-sibling
CASE_OPTION=--diagnose
RUN="$CASE/run root"
mkdir "$RUN"
chmod 700 "$RUN"
me term_a
siblings 'w1:t1|term_b|w1:p2'
live "$(sock term_b)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 0 && $(cat "$CASE/out") == "$(sock term_b)" && ! -e $CASE/exec ]] ||
  fail 'diagnose-sibling: expected the complete sibling path without starting the server'

setup_case diagnose-auto
CASE_OPTION=--diagnose
me term_focused
run_case HERDR_ENV= HERDR_PANE_ID= HERDR_SOCKET_PATH=
[[ $RC -eq 0 && $(cat "$CASE/out") == auto && ! -e $CASE/exec ]] ||
  fail 'diagnose-auto: expected the upstream auto selector without starting the server'
[[ ! -e $CASE/herdr-argv && ! -e $CASE/queried ]] || fail 'diagnose-auto: consulted pane discovery'

setup_case diagnose-ambiguous
CASE_OPTION=--diagnose
me term_a
siblings 'w1:t1|term_b|w1:p2' 'w1:t1|term_c|w1:p3'
live "$(sock term_b)" "$(sock term_c)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 4 && ! -s $CASE/out && ! -e $CASE/exec ]] || fail 'diagnose-ambiguous: expected refusal without exec'
grep -qF -- "$(sock term_b)  pane w1:p2  pid 4242" "$CASE/err" || fail 'diagnose-ambiguous: missing pane 2'
grep -qF -- "$(sock term_c)  pane w1:p3  pid 4242" "$CASE/err" || fail 'diagnose-ambiguous: missing pane 3'
grep -qF NVIM_MCP_SOCKET "$CASE/err" || fail 'diagnose-ambiguous: missing pin guidance'

setup_case diagnose-dead-pin
CASE_OPTION=--diagnose
make_socket "$RUN/dead.sock"
run_case NVIM_MCP_SOCKET="$RUN/dead.sock"
[[ $RC -eq 3 && ! -s $CASE/out && ! -e $CASE/exec ]] || fail 'diagnose-dead-pin: expected refusal without exec'
grep -qF 'no Neovim answers' "$CASE/err" || fail 'diagnose-dead-pin: missing reason'

setup_case diagnose-root
CASE_OPTION=--diagnose
me term_a
chmod 755 "$RUN"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 2 && ! -s $CASE/out && ! -e $CASE/exec ]] || fail 'diagnose-root: expected refusal without exec'
grep -qF 0700 "$CASE/err" || fail 'diagnose-root: missing mode guidance'

setup_case diagnose-length
CASE_OPTION=--diagnose
me term_a
RUN="$RUN/$(printf '%090d' 0)"
mkdir "$RUN"
chmod 700 "$RUN"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 5 && ! -s $CASE/out && ! -e $CASE/exec ]] || fail 'diagnose-length: expected refusal without exec'
grep -qF 'unix sockets allow 103' "$CASE/err" || fail 'diagnose-length: missing limit'

printf 'PASS: %s (8 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
