#!/usr/bin/env bash
# Pulse the configured Hue rooms green (exit code 0) or red (any other exit
# code), two heartbeat cycles, then restore every light to how it was.
#
# Usage: hue-pulse.sh <exit_code>
#
# Silent no-op if openhue/jq is missing or no configured room is found.

set -euo pipefail

# Pure decisions (which colour, which restore arguments) live in the shared
# helper so tests call them directly.
# shellcheck source=dot_local/libexec/pns/helpers/event.sh
source "${PNS_HELPERS_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/helpers}/event.sh"

exit_code="${1:-0}"
# Newline-separated: room names contain spaces. All rooms go in ONE openhue
# call so they flash together instead of staggering.
rooms_raw="${HUE_PULSE_ROOMS:-$'3F - Studio\n2F - Kitchen'}"

command -v openhue &>/dev/null || exit 0
command -v jq &>/dev/null || exit 0

# Only one pulse may run at a time, or two triggers would interleave openhue
# calls and restore each other's transient state. The lock is /usr/bin/lockf
# on a held fd: the kernel releases it on any exit, so it cannot go stale. A
# concurrent pulse is skipped, not queued; a dropped duplicate is invisible.
# The lockfile lives under $HOME because a stripped hook environment can fall
# back to the shared /tmp, where another local account could squat the path.
lock_dir="$HOME/.local/state"
mkdir -p "$lock_dir" 2>/dev/null || exit 0
lock="$lock_dir/hue-pulse.lockf"
# Clean up only our own temp state on exit; the kernel owns lock release.
trap '[[ -n ${state_file:-} ]] && rm -f "$state_file"; true' EXIT
if [[ -x /usr/bin/lockf ]]; then
  exec 9>>"$lock" 2>/dev/null || exit 0
  /usr/bin/lockf -s -t 0 9 || exit 0
fi

# A configured room that no longer exists is skipped, never fatal: a rename in
# the Hue app must not take the other rooms down with it.
rooms_json=$(openhue get room --json 2>/dev/null || true)
room_ids=()
while IFS= read -r room_name; do
  [[ -n $room_name ]] || continue
  room_id=$(printf '%s' "$rooms_json" |
    jq -r --arg name "$room_name" '.. | select(.Name? == $name) | .Id' | head -1 || true)
  [[ -n $room_id ]] && room_ids+=("$room_id")
done <<<"$rooms_raw"
[[ ${#room_ids[@]} -eq 0 ]] && exit 0

state_file=$(mktemp)

# Snapshot every light so the restore can put it back exactly.
# TSV columns: id  on(true|false)  brightness  mode(ct|xy)  v1  v2
openhue get light --json 2>/dev/null |
  jq -r --argjson rooms "$(printf '%s\n' "${room_ids[@]}" | jq -R . | jq -s .)" '
    .[] |
    select(.Parent.Parent.Id as $p | $rooms | index($p)) |
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

# Two heartbeat cycles ending on the LOW phase, so the restore is a gentle
# step up rather than a drop from peak brightness. Each sleep matches its
# transition time so a ramp finishes before the next one starts.
read -r px py peak <<<"$(pns_pulse_color "$exit_code")"
pulse_to() {
  # Args: brightness (0-100). 1.2s smooth ramp.
  openhue set room "${room_ids[@]}" --on -x "$px" -y "$py" \
    --brightness "$1" --transition-time 1200ms 2>/dev/null
}
# First call gates the whole pulse, if openhue is unreachable here, bail
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
  mapfile -t restore_args < <(pns_restore_args "$on_state" "$bri" "$mode" "$v1" "$v2")
  openhue set light "$lid" "${restore_args[@]}" 2>/dev/null || true
done <"$state_file"

# A failed pulse must never fail the caller.
exit 0
