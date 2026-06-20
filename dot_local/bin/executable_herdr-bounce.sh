#!/usr/bin/env bash
# Bounce to the last workspace you came from (temporal toggle, like tmux's
# last-window). Focuses the id recorded by herdr-jump.sh or the previous bounce,
# then records the workspace you just left so pressing again returns here.
#
# herdr has no native most-recently-used toggle and does not track focus history
# in its session state, so only chord jumps (herdr-jump.sh) and bounces update
# this — switches made through the picker or `goto` are not tracked.
#
# Usage: herdr-bounce.sh
set -euo pipefail

herdr_bin=${HERDR_BIN_PATH:-herdr}
state="${XDG_STATE_HOME:-$HOME/.local/state}/herdr/last-workspace"

target=$(cat "$state" 2>/dev/null || true)
[[ -n $target ]] || exit 0 # nothing recorded yet

list=$("$herdr_bin" workspace list 2>/dev/null)

# `workspace focus` returns exit 0 even on a missing id, so guard by checking the
# list. If the recorded workspace is gone, clear the stale state and bail.
if ! jq -e --arg t "$target" 'any(.result.workspaces[]; .workspace_id == $t)' <<<"$list" >/dev/null; then
  : >"$state"
  exit 0
fi

from=$(jq -r '[.result.workspaces[] | select(.focused) | .workspace_id][0] // empty' <<<"$list")
"$herdr_bin" workspace focus "$target" >/dev/null

# Record where we came from so the next bounce returns here (symmetric toggle).
if [[ -n $from && $from != "$target" ]]; then
  mkdir -p "$(dirname "$state")"
  printf '%s\n' "$from" >"$state"
fi
