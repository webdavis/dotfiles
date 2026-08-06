#!/usr/bin/env bash
# Claude Code UserPromptSubmit hook: record session start time on first prompt.
#
# Hook input: JSON on stdin with { session_id, transcript_path, cwd,
# permission_mode, hook_event_name, prompt }.
#
# Paired with claude-stop-pulse.sh which reads the marker to decide whether
# to fire a Hue pulse.

set -euo pipefail

input=$(cat)
session_id=$(printf '%s' "$input" | jq -r '.session_id // empty' 2>/dev/null || true)
[[ -z $session_id ]] && exit 0

start_file="/tmp/claude-session-${session_id}-start"
[[ -f $start_file ]] || date +%s >"$start_file"
exit 0
