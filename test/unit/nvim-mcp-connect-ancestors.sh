#!/usr/bin/env bash
set -euo pipefail

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

for mode in 777 770; do
  setup_case "ancestor-$mode"
  RUN="$CASE/open/inner/run"
  mkdir -p "$RUN"
  chmod 700 "$RUN" "$CASE/open/inner"
  chmod "$mode" "$CASE/open"
  me term_a
  live "$(sock term_a)"
  run_case XDG_RUNTIME_DIR="$RUN"
  [[ $RC -eq 2 ]] || fail "ancestor-$mode: trusted a replaceable root, exit $RC"
  [[ ! -e $CASE/probed && ! -e $CASE/exec ]] || fail "ancestor-$mode: used an unsafe socket"
done

setup_case sticky-parent
mkdir -p "$CASE/sticky/run"
chmod 1777 "$CASE/sticky"
chmod 700 "$CASE/sticky/run"
RUN="$CASE/sticky/run"
me term_a
live "$(sock term_a)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 0 ]] || fail "sticky-parent: refused a protected owned root ($(cat "$CASE/err"))"
grep -qxF -- "--connect $(sock term_a)" "$CASE/exec" || fail 'sticky-parent: changed the socket spelling'

setup_case safe-alias
mkdir "$CASE/aliases"
ln -s "$CASE" "$CASE/aliases/alias"
RUN="$CASE/aliases/alias/run"
me term_a
live "$(sock term_a)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 0 ]] || fail "safe-alias: refused a protected alias ($(cat "$CASE/err"))"
grep -qxF -- "--connect $(sock term_a)" "$CASE/exec" || fail 'safe-alias: changed the socket spelling'

printf 'PASS: %s (4 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
