#!/usr/bin/env bash
# Pulse the studio Hue room green (success) or red (failure) through a
# bright → dim → bright cycle (~3 seconds total), then restore each light
# to its saved on-state, brightness, and color.
#
# Usage: hue-pulse.sh <exit_code>
#   exit 0  → pulse green (#00c96d)
#   exit ≠0 → pulse red   (#ff657a)
#
# Silent no-op if openhue/jq isn't installed or the target room isn't found.

set -euo pipefail

exit_code="${1:-0}"
room_name="${HUE_PULSE_ROOM:-3F - Studio}"

command -v openhue &>/dev/null || exit 0
command -v jq &>/dev/null || exit 0

room_id=$(openhue get room --json 2>/dev/null |
  jq -r --arg name "$room_name" '.. | select(.Name? == $name) | .Id' | head -1)
[[ -z $room_id ]] && exit 0

state_file=$(mktemp)
trap 'rm -f "$state_file"' EXIT

# Snapshot each light in the room: id, on-state, brightness, color mode, and
# the color value(s) — either mirek (color temp) or CIE xy.
# TSV columns: id  on(true|false)  brightness  mode(ct|xy)  v1  v2
openhue get light --json 2>/dev/null |
  jq -r --arg room "$room_id" '
    .[] |
    select(.Parent.Parent.Id == $room) |
    [
      .Id,
      (.HueData.on.on | tostring),
      ((.HueData.dimming.brightness // 100) | tostring),
      (if .HueData.color_temperature.mirek_valid == true then "ct" else "xy" end),
      (if .HueData.color_temperature.mirek_valid == true then (.HueData.color_temperature.mirek | tostring) else (.HueData.color.xy.x | tostring) end),
      (if .HueData.color_temperature.mirek_valid == true then "" else (.HueData.color.xy.y | tostring) end)
    ] | @tsv
  ' >"$state_file"

[[ ! -s $state_file ]] && exit 0

# Pulse: bright → dim → bright in the status color, then restore.
# Two cycles so the user's peripheral vision actually registers it.
if [[ $exit_code -eq 0 ]]; then
  color="#00c96d"
else
  color="#ff657a"
fi
# First call gates the whole pulse — if openhue is unreachable here, bail
# without attempting further changes or a restore.
openhue set room "$room_id" --on --rgb "$color" --brightness 60 --transition-time 400ms 2>/dev/null || exit 0
sleep 1
openhue set room "$room_id" --on --rgb "$color" --brightness 10 --transition-time 400ms 2>/dev/null || true
sleep 1
openhue set room "$room_id" --on --rgb "$color" --brightness 60 --transition-time 400ms 2>/dev/null || true
sleep 1

# Restore each light.
while IFS=$'\t' read -r lid on_state bri mode v1 v2; do
  if [[ $on_state == "true" ]]; then
    if [[ $mode == "ct" ]]; then
      openhue set light "$lid" --on --brightness "$bri" -t "$v1" --transition-time 500ms 2>/dev/null || true
    else
      openhue set light "$lid" --on --brightness "$bri" -x "$v1" -y "$v2" --transition-time 500ms 2>/dev/null || true
    fi
  else
    openhue set light "$lid" --off --transition-time 500ms 2>/dev/null || true
  fi
done <"$state_file"
