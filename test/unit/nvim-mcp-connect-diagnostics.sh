#!/usr/bin/env bash
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

printf 'PASS: %s (4 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
