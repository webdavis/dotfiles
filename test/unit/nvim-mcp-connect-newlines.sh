#!/usr/bin/env bash
set -euo pipefail

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

setup_case newline-root
RUN="$CASE/line"$'\n'"break"
mkdir "$RUN"
chmod 700 "$RUN"
me term_a
live "$(sock term_a)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 2 ]] || fail "newline-root: accepted a multiline runtime root, exit $RC"
[[ ! -e $CASE/probed && ! -e $CASE/exec ]] || fail 'newline-root: used the multiline root'

setup_case newline-pin
pin="$RUN/pin"$'\n'"socket"
live "$pin"
run_case NVIM_MCP_SOCKET="$pin"
[[ $RC -eq 3 ]] || fail "newline-pin: accepted a multiline pin, exit $RC"
[[ ! -e $CASE/probed && ! -e $CASE/herdr-argv && ! -e $CASE/exec ]] || fail 'newline-pin: probed or replaced the pin'

setup_case newline-fallback
me term_a
printf '%s/line\nbreak/instance' "$CASE" >"$CASE/rundir"
run_case
[[ $RC -eq 2 ]] || fail "newline-fallback: accepted an ambiguous run report, exit $RC"
[[ ! -e $CASE/probed && ! -e $CASE/exec ]] || fail 'newline-fallback: used a split path'

printf 'PASS: %s (3 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
