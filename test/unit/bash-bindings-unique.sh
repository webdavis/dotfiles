#!/usr/bin/env bash
# bash-bindings-unique.sh, no (keymap, key-seq) pair in dot_bash_bindings may
# be bound twice. A later `builtin bind -m <keymap> '"<key-seq>": ...'` silently
# clobbers an earlier one, so a duplicate is always either a typo'd key-seq or a
# lost binding, never intentional.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BINDINGS="$REPO_ROOT/dot_bash_bindings"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -f $BINDINGS ]] || fail "missing file: $BINDINGS"

# Keymap-scoped bind lines come in three shapes:
#   builtin bind -m <keymap> '"<key-seq>": <macro-or-function>'   (single-quoted)
#   builtin bind -m <keymap> "\"<key-seq>\": <macro>"             (double-quoted)
#   builtin bind -m <keymap> '<Keyname>: <function>'              (keyname form)
# The key-seq never contains a double quote, so it sits between the opening
# quote pair and the next ": in the first two shapes.
single_quoted_re="^[[:space:]]*builtin[[:space:]]+bind[[:space:]]+-m[[:space:]]+([[:alnum:]-]+)[[:space:]]+'\"([^\"]+)\":"
double_quoted_re='^[[:space:]]*builtin[[:space:]]+bind[[:space:]]+-m[[:space:]]+([[:alnum:]-]+)[[:space:]]+"\\"([^"]+)\\":'
keyname_re="^[[:space:]]*builtin[[:space:]]+bind[[:space:]]+-m[[:space:]]+([[:alnum:]-]+)[[:space:]]+'(Control|Meta)-(.):"

declare -A seen=()
declare -a duplicates=()
parsed=0
line_number=0
while IFS= read -r line; do
  ((line_number += 1))
  if [[ $line =~ $single_quoted_re || $line =~ $double_quoted_re ]]; then
    keyseq="${BASH_REMATCH[2]}"
  elif [[ $line =~ $keyname_re ]]; then
    # Normalize the keyname form onto escape notation: readline treats
    # Control-u and \C-u as the same key, so they must share one pair slot.
    case "${BASH_REMATCH[2]}" in
      Control) keyseq="\\C-${BASH_REMATCH[3],,}" ;;
      Meta) keyseq="\\M-${BASH_REMATCH[3]}" ;;
    esac
  else
    continue
  fi
  parsed=$((parsed + 1))
  pair="${BASH_REMATCH[1]} ${keyseq}"
  if [[ -n ${seen[$pair]:-} ]]; then
    duplicates+=("($pair) bound at line ${seen[$pair]} and again at line $line_number")
  else
    seen[$pair]=$line_number
  fi
done <"$BINDINGS"

# Guard the parser itself: a regex drifting out of sync with the file must not
# fake a green run. The file carries a couple hundred keymap-scoped binds.
((parsed >= 200)) || fail "parsed only $parsed 'builtin bind -m' lines; parser out of sync with $BINDINGS"

if ((${#duplicates[@]} > 0)); then
  printf 'FAIL: duplicate (keymap, key-seq) bindings in dot_bash_bindings:\n' >&2
  printf '  %s\n' "${duplicates[@]}" >&2
  exit 1
fi

printf 'PASS: %d keymap-scoped bind lines, every (keymap, key-seq) pair unique\n' "$parsed"
