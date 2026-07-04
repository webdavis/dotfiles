#!/usr/bin/env bash
# Create-or-focus a herdr workspace by label.
#
# `herdr workspace create` is NOT idempotent — it spawns a new workspace every
# invocation — and `herdr workspace focus` takes a workspace id, not a label, so
# there is no single built-in create-or-focus command; hence this helper.
#
# (Workspace MRU "last workspace" tracking lives in the herdr-last-workspace
# herdr plugin, which hooks the workspace.focused event — this helper just jumps.)
#
# Usage: herdr-jump.sh <label> <cwd>
set -euo pipefail

label=$1
cwd=$2
herdr_bin=${HERDR_BIN_PATH:-herdr}

# First existing workspace whose label matches, or empty if none. `herdr
# workspace list` already emits JSON; it has no --json flag.
id=$("$herdr_bin" workspace list 2>/dev/null |
  jq -r --arg l "$label" '[.result.workspaces[] | select(.label == $l) | .workspace_id][0] // empty')

if [[ -n $id ]]; then
  "$herdr_bin" workspace focus "$id" >/dev/null
else
  "$herdr_bin" workspace create --cwd "$cwd" --label "$label" --focus >/dev/null
fi
