#!/usr/bin/env bash
# Create-or-focus a herdr workspace by label, recording the workspace you leave
# so herdr-bounce.sh can jump back to it (temporal "last workspace", like tmux's
# last-window — herdr has no native most-recently-used toggle).
#
# `herdr workspace create` is NOT idempotent — it spawns a new workspace every
# invocation — and `herdr workspace focus` takes a workspace id, not a label, so
# there is no single built-in create-or-focus command; hence this helper.
#
# Usage: herdr-jump.sh <label> <cwd>
set -euo pipefail

label=$1
cwd=$2
herdr_bin=${HERDR_BIN_PATH:-herdr}
state="${XDG_STATE_HOME:-$HOME/.local/state}/herdr/last-workspace"

# One socket call; parse for both the focused workspace and the target label.
list=$("$herdr_bin" workspace list 2>/dev/null)
from=$(jq -r '[.result.workspaces[] | select(.focused) | .workspace_id][0] // empty' <<<"$list")
id=$(jq -r --arg l "$label" '[.result.workspaces[] | select(.label == $l) | .workspace_id][0] // empty' <<<"$list")

# Record the workspace we are leaving, unless we are not actually moving. In the
# create branch id is empty, so a non-empty from always records (new id ≠ from).
if [[ -n $from && $from != "$id" ]]; then
  mkdir -p "$(dirname "$state")"
  printf '%s\n' "$from" >"$state"
fi

if [[ -n $id ]]; then
  "$herdr_bin" workspace focus "$id" >/dev/null
else
  "$herdr_bin" workspace create --cwd "$cwd" --label "$label" --focus >/dev/null
fi
