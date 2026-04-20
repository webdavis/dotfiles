#!/usr/bin/env bash
# Pulse Hue lights green (success) or red (failure) for ~2 seconds, then
# return to a named scene. No save/restore of current state per v2 §7.2.
#
# Usage: hue-pulse.sh <exit_code>
# Exit 0 → green (#00c96d); non-zero → red (#ff657a).
# Silent no-op if openhue isn't installed or the target room isn't found.

set -euo pipefail

exit_code="${1:-0}"

command -v openhue &>/dev/null || exit 0
command -v jq &>/dev/null || exit 0

# Identify the target Hue room.
room_id="$(openhue get room --json 2>/dev/null |
  jq -r '.. | select(.Name? == "3F - Studio") | .Id' 2>/dev/null | head -1)"
[[ -z $room_id ]] && exit 0

# Pulse color.
if [[ $exit_code -eq 0 ]]; then
  color="#00c96d"
else
  color="#ff657a"
fi

openhue set room "$room_id" --on --rgb "$color" --brightness 50 --transition-time 500ms 2>/dev/null || exit 0
sleep 2

# Return to a named scene rather than saving/restoring per-light state.
openhue set scene "Default" 2>/dev/null || true
