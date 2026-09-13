#!/usr/bin/env bash
set -euo pipefail

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

setup_case alias-parent
mkdir -p "$CASE/open" "$CASE/target/run"
chmod 777 "$CASE/open"
chmod 700 "$CASE/target/run"
ln -s "$CASE/target" "$CASE/open/alias"
RUN="$CASE/open/alias/run"
me term_a
live "$(sock term_a)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 2 ]] || fail "alias-parent: trusted a replaceable alias, exit $RC"
[[ ! -e $CASE/probed && ! -e $CASE/exec ]] || fail 'alias-parent: used an unsafe alias'

setup_case alias-target
mkdir -p "$CASE/open/run" "$CASE/aliases"
chmod 777 "$CASE/open"
chmod 700 "$CASE/open/run"
ln -s "$CASE/open" "$CASE/aliases/alias"
RUN="$CASE/aliases/alias/run"
me term_a
live "$(sock term_a)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 2 ]] || fail "alias-target: trusted a replaceable target, exit $RC"
[[ ! -e $CASE/probed && ! -e $CASE/exec ]] || fail 'alias-target: used an unsafe target'

printf 'PASS: %s (2 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
