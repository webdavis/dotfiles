#!/usr/bin/env bash
# Claude Code Stop hook: pulse Hue green if the session lasted > 5 min.
#
# Hook input: JSON on stdin with { session_id, transcript_path, cwd,
# permission_mode, hook_event_name }. Env vars do NOT carry the session ID.
#
# Paired with claude-user-prompt-start.sh which writes the session start marker.

set -euo pipefail

input=$(cat)
session_id=$(printf '%s' "$input" | jq -r '.session_id // empty' 2>/dev/null || true)
[[ -z $session_id ]] && exit 0

start_file="/tmp/claude-session-${session_id}-start"
[[ -f $start_file ]] || exit 0

elapsed=$(($(date +%s) - $(cat "$start_file")))
rm -f "$start_file"

# Fire the pulse detached (backgrounded subshell) so the Stop hook returns immediately instead of
# blocking ~5s on the pulse; a failed Hue pulse must never surface as a Stop hook error either way.
if ((elapsed >= 300)); then
  ("$HOME/.local/bin/hue-pulse.sh" 0 >/dev/null 2>&1 &)
fi
exit 0
