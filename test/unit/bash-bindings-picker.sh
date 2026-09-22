#!/usr/bin/env bash
# bash-bindings-picker.sh: __bash_bindings_list_bash_bindings reads the records
# `chord render menu` generates and dispatches on the action kind.
#
# WHY THIS EXISTS. The function it replaced scraped the rendered readline
# bind calls and filtered them on vi-insert, so it silently dropped every
# emacs-only and mode-less row and every readline-command row, and it could
# not show a group or a description because the rendering carries neither.
# These asserts pin the two properties a regression would undo: every record
# reaches the picker, and the kind decides what running the selection means.
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

# 2. A run record executes.
picked=alt-r __bash_bindings_list_bash_bindings
[[ -e "$HOME/ran-the-run-row" ]] || fail "the run record did not execute"

# 3. A function record calls the function.
picked=alt-f __bash_bindings_list_bash_bindings
[[ -e "$HOME/called-the-function" ]] || fail "the function record was not called"

# 4. An insert record seeds an editable line, and what comes back runs.
picked=alt-i __bash_bindings_list_bash_bindings <<<"touch edited-the-insert-row" >/dev/null
[[ -e "$HOME/edited-the-insert-row" ]] ||
  fail "the edited insert record did not run"

# 5. A readline command or macro is refused rather than run as a shell command.
for key in alt-q alt-w; do
  if message="$(picked="$key" __bash_bindings_list_bash_bindings 2>&1)"; then
    fail "$key was accepted as a runnable command"
  fi
  [[ $message == *"edits the line rather than running a command"* ]] ||
    fail "$key was refused without saying why: $message"
done

printf 'PASS: bash-bindings-picker\n'
