#!/usr/bin/env bash
# Pulse the studio Hue room deep green (success) or deep red (failure)
# through a bright → 20% → bright → 20% heartbeat cycle (~5 seconds), then
# restore each light to its saved on-state, brightness, and color. Colors
# are addressed in CIE xy at the gamut corners for maximum saturation.
#
# Usage: hue-pulse.sh <exit_code>
#   exit 0  → pulse deep green (xy 0.17, 0.7  — gamut C green corner)
#   exit ≠0 → pulse deep red   (xy 0.6915, 0.3083 — gamut C red corner)
#
# Silent no-op if openhue/jq isn't installed or the target room isn't found.

set -euo pipefail

exit_code="${1:-0}"
room_name="${HUE_PULSE_ROOM:-3F - Studio}"

command -v openhue &>/dev/null || exit 0
command -v jq &>/dev/null || exit 0

# Serialize concurrent pulses so two triggers (e.g. a Stop hook and the long-command notifier firing at
# once) queue instead of interleaving openhue calls and restoring each other's transient state. mkdir is
# atomic; wait up to ~30s for our turn, then give up rather than pile stale pulses up.
lock="${TMPDIR:-/tmp}/hue-pulse.lock"
# Release only if we still own the lock: a live pulse that took it over from us must not
# have its lock clobbered by our exit trap. Inline (not a function) so shellcheck does not
# flag the trap-only body as unreachable.
trap 'if [[ -f "$lock/pid" && "$(cat "$lock/pid" 2>/dev/null || true)" == "$$" ]]; then rm -rf "$lock" 2>/dev/null || true; fi; [[ -n ${state_file:-} ]] && rm -f "$state_file"; true' EXIT
tries=0
while true; do
  if mkdir "$lock" 2>/dev/null; then
    printf '%s\n' "$$" >"$lock/pid"
    break
  fi
  holder="$(cat "$lock/pid" 2>/dev/null || true)"
  if [[ -n $holder ]] && kill -0 "$holder" 2>/dev/null; then
    sleep 0.5 # a live pulse holds the lock; wait our turn
    tries=$((tries + 1))
    if ((tries > 60)); then exit 0; fi
    continue
  fi
  # The holder is dead (crashed / SIGKILLed mid-pulse) or has not published its pid yet.
  # Give a just-created lock a brief grace to write its pid before deciding it is stale, so
  # we never steal from a live holder still starting up; take over only if it is still
  # ownerless after the grace. A stale lock must never suppress every later pulse.
  sleep 0.5
  holder="$(cat "$lock/pid" 2>/dev/null || true)"
  if [[ -z $holder ]] || ! kill -0 "$holder" 2>/dev/null; then
    rm -rf "$lock" 2>/dev/null || true
  fi
  tries=$((tries + 1))
  if ((tries > 60)); then exit 0; fi
done

room_id=$(openhue get room --json 2>/dev/null |
  jq -r --arg name "$room_name" '.. | select(.Name? == $name) | .Id' | head -1 || true)
[[ -z $room_id ]] && exit 0

state_file=$(mktemp)

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
  ' >"$state_file" || true

[[ ! -s $state_file ]] && exit 0

# Pulse pattern: bright → 20% → bright → 20% → restore.
# Four pulse phases (two full lub-DUB cycles) ending on the LOW phase, so
# the restore is a gentle step from "dim color" back to the user's original
# state — not a jarring drop from peak brightness.
#
# Sleeps match the 1.2s transitions so the bulb fully reaches each target
# (no interrupting overlap). 1.2s is long enough for the ramp itself to
# read as smooth wave motion, while the brief API-roundtrip "settle"
# between transitions is short enough not to feel like a hitch.
#
# Color: CIE xy gamut-corner coords (not --rgb) so the colors hit the
# deepest saturation the bulb is physically capable of. Hue clamps RGB to
# its gamut and desaturates aggressively; xy bypasses that conversion.
if [[ $exit_code -eq 0 ]]; then
  px=0.17 # gamut C green corner
  py=0.7
  peak=70 # green washes toward white at full brightness (Bezold-Brücke);
  # 70% lets the green LED primary dominate perception
else
  px=0.6915 # gamut C red corner
  py=0.3083
  peak=100 # red stays saturated at full brightness
fi
pulse_to() {
  # Args: brightness (0-100). 1.2s smooth ramp.
  openhue set room "$room_id" --on -x "$px" -y "$py" \
    --brightness "$1" --transition-time 1200ms 2>/dev/null
}
# First call gates the whole pulse — if openhue is unreachable here, bail
# without attempting further changes or a restore.
pulse_to "$peak" || exit 0
sleep 1.2
pulse_to 20 || true
sleep 1.2
pulse_to "$peak" || true
sleep 1.2
pulse_to 20 || true
sleep 1.2

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

# Best-effort notifier: a failed pulse must never fail the caller (Stop hook /
# long-command notifier). Any openhue hiccup above is swallowed; exit clean.
exit 0
