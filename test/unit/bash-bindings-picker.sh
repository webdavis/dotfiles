#!/usr/bin/env bash
# Generated binding records prepare shell commands and describe Readline actions.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
export HOME="$work"
mkdir -p "$HOME/.config/chord"
cd "$HOME"

printf '%s\n' \
  '# a comment line is not a record' \
  $'alt-q\teditor\tcommand\tbeginning-of-line\tGo to the start.' \
  $'alt-w\teditor\tmacro\t\\C-x0ls\tType a listing.' \
  $'alt-r\tlisting\trun\ttouch ran-the-run-row\tRun a command.' \
  $'alt-f\tlisting\tfunction\t__picker_test_function\tCall a function.' \
  $'alt-i\tgit\tinsert\techo seeded\tSeed the line.' \
  >"$HOME/.config/chord/bindings-menu.tsv"

# The file opens with `bind` calls for its helper macros, which a
# non-interactive shell refuses, so the source's own status says nothing.
# shellcheck source=/dev/null
source "$REPO_ROOT/dot_bash_bindings_functions" 2>/dev/null || true
declare -F __bash_bindings_list_bash_bindings >/dev/null ||
  fail "dot_bash_bindings_functions does not define the picker"

# 1. Without fzf every record is listed, the comment line excluded. Asserted
# before fzf is stubbed, because a stub is a function and `command -v` finds
# one whatever PATH holds.
listing="$(PATH=/usr/bin:/bin __bash_bindings_list_bash_bindings)"
[[ $(printf '%s\n' "$listing" | wc -l) -eq 5 ]] ||
  fail "the fzf-less listing did not show all five records: $listing"
[[ $listing == *"beginning-of-line"* ]] ||
  fail "a readline-command record is missing from the listing"
[[ $listing != *"a comment line"* ]] ||
  fail "the comment line was listed as a record"

__picker_test_function() { touch "$HOME/called-the-function"; }

# fzf stands in for the operator: it picks the record matching $picked.
picked=""
fzf() { grep -- "$picked"; }

# Commands remain editable and do not execute during selection.
for pair in 'alt-r|touch ran-the-run-row' 'alt-f|__picker_test_function' 'alt-i|echo seeded'; do
  picked=${pair%%|*}
  expected=${pair#*|}
  READLINE_LINE=pending READLINE_POINT=3
  __bash_bindings_list_bash_bindings
  [[ $READLINE_LINE == "$expected" && $READLINE_POINT -eq ${#expected} ]] ||
    fail "$picked did not prepare its command"
done
[[ ! -e "$HOME/ran-the-run-row" && ! -e "$HOME/called-the-function" ]] ||
  fail "selecting a command executed it"

for key in alt-q alt-w; do
  message="$(picked="$key" __bash_bindings_list_bash_bindings 2>&1)"
  [[ $message == *"Press $key to use this Readline action"* ]] ||
    fail "$key did not explain how to use its Readline action"
done

printf 'PASS: bash-bindings-picker\n'
