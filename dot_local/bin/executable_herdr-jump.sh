#!/usr/bin/env bash
# Create-or-focus a herdr workspace by label.
#
# `herdr workspace create` is NOT idempotent — it spawns a new workspace every
# invocation — and `herdr workspace focus` takes a workspace id, not a label. So
# the quick-jump chords and the `h` alias call this helper to get tmux-like
# "switch to workspace X, creating it only if absent" behavior.
#
# Usage: herdr-jump.sh <label> <cwd>
set -euo pipefail

label=$1
cwd=$2
herdr_bin=${HERDR_BIN_PATH:-herdr}

# First existing workspace whose label matches, or empty if none.
# `herdr workspace list` already emits JSON; it has no --json flag.
id=$("$herdr_bin" workspace list 2>/dev/null |
  jq -r --arg l "$label" '[.result.workspaces[] | select(.label == $l) | .workspace_id][0] // empty')

if [[ -n $id ]]; then
  "$herdr_bin" workspace focus "$id" >/dev/null
else
  "$herdr_bin" workspace create --cwd "$cwd" --label "$label" --focus >/dev/null
fi
