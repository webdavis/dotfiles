#!/usr/bin/env bash
set -euo pipefail

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

# --- o) one live sibling is connected to -------------------------------------
setup_case one-sibling
me term_a
siblings 'w1:t1|term_a|w1:p1' 'w1:t1|term_b|w1:p2'
live "$(sock term_b)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 0 ]] || fail "one-sibling: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qxF -- "--connect $(sock term_b)" "$CASE/exec" || fail "one-sibling: wrong socket ($(cat "$CASE/exec" 2>/dev/null))"
[[ "$(cat "$CASE/herdr-argv")" == $'pane current --pane w1:p1\npane list --workspace w1' ]] ||
  fail "one-sibling: herdr was not asked this pane's identity, then its workspace ($(cat "$CASE/herdr-argv"))"

# --- p) own pane wins, the tab is never listed -------------------------------
setup_case own-wins
me term_a
siblings 'w1:t1|term_a|w1:p1' 'w1:t1|term_b|w1:p2'
live "$(sock term_a)" "$(sock term_b)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 0 ]] || fail "own-wins: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qxF -- "--connect $(sock term_a)" "$CASE/exec" ||
  fail "own-wins: a sibling was chosen over the own pane ($(cat "$CASE/exec" 2>/dev/null))"
grep -q 'pane list' "$CASE/herdr-argv" && fail 'own-wins: the tab was listed although the own pane answered'

# --- w) a sibling moved between workspaces is still found by its terminal ----
# herdr renames a pane moved across workspaces (w1:p2 becomes, say, w9:p2) but
# its terminal id and its socket do not change. The listing names the pane by
# its NEW id and the same terminal; the resolver must not care about the id.
setup_case moved-sibling
me term_a
siblings 'w1:t1|term_a|w1:p1' 'w1:t1|term_b|w9:p2'
live "$(sock term_b)"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 0 ]] || fail "moved-sibling: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qxF -- "--connect $(sock term_b)" "$CASE/exec" || fail "moved-sibling: wrong socket ($(cat "$CASE/exec" 2>/dev/null))"

printf 'PASS: %s (3 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
