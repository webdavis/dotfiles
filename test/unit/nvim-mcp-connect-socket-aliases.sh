#!/usr/bin/env bash
set -euo pipefail

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

setup_case pinned-alias
live "$CASE/public.sock"
ln -s "$CASE/public.sock" "$RUN/pin.sock"
printf '%s\n' "$RUN/pin.sock" >>"$CASE/live"
run_case NVIM_MCP_SOCKET="$RUN/pin.sock"
[[ $RC -eq 3 ]] || fail "pinned-alias: accepted a socket symlink, exit $RC"
[[ ! -e $CASE/probed && ! -e $CASE/herdr-argv && ! -e $CASE/exec ]] || fail 'pinned-alias: probed or replaced the pin'

setup_case pane-alias
me term_a
siblings 'w1:t1|term_b|w1:p2'
live "$CASE/public.sock" "$(sock term_b)"
ln -s "$CASE/public.sock" "$(sock term_a)"
sock term_a >>"$CASE/live"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 0 ]] || fail "pane-alias: lost the safe sibling ($(cat "$CASE/err"))"
grep -qxF -- "--connect $(sock term_b)" "$CASE/exec" || fail 'pane-alias: selected the alias'
grep -qxF "$(sock term_a)" "$CASE/probed" && fail 'pane-alias: probed a socket symlink'

setup_case sibling-alias
me term_a
siblings 'w1:t1|term_b|w1:p2' 'w1:t1|term_c|w1:p3'
live "$CASE/public.sock" "$(sock term_c)"
ln -s "$CASE/public.sock" "$(sock term_b)"
sock term_b >>"$CASE/live"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 0 ]] || fail "sibling-alias: counted an alias as a candidate, exit $RC"
grep -qxF -- "--connect $(sock term_c)" "$CASE/exec" || fail 'sibling-alias: selected the alias'
grep -qxF "$(sock term_b)" "$CASE/probed" && fail 'sibling-alias: probed a socket symlink'

printf 'PASS: %s (3 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
