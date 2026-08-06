#!/usr/bin/env bash
# Pulse the configured Hue rooms deep green (success) or deep red (failure)
# through a bright → 20% → bright → 20% heartbeat cycle (~5 seconds), then
# restore each light to its saved on-state, brightness, and color. Colors
# are addressed in CIE xy at the gamut corners for maximum saturation.
#
# Usage: hue-pulse.sh <exit_code>
#   exit 0  → pulse deep green (xy 0.17, 0.7, gamut C green corner)
#   exit ≠0 → pulse deep red   (xy 0.6915, 0.3083, gamut C red corner)
#
# Silent no-op if openhue/jq isn't installed or no configured room is found.

set -euo pipefail

# The DECISION CORE, same split relay.sh uses: which colour an exit code means
# and which arguments put a snapshotted light back are pure functions, testable
# without spawning this script or stubbing openhue.
# shellcheck source=dot_local/libexec/pns/helpers/event.sh
source "${PNS_HELPERS_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/helpers}/event.sh"

exit_code="${1:-0}"
# NEWLINE-separated, because room names contain spaces ("3F - Studio") and no
# other separator can be split on safely. openhue accepts up to ten rooms in a
# single `set room`, so every configured room changes in ONE request and they
# read as one signal rather than a stagger.
rooms_raw="${HUE_PULSE_ROOMS:-$'3F - Studio\n2F - Kitchen'}"

command -v openhue &>/dev/null || exit 0
command -v jq &>/dev/null || exit 0

# Serialize concurrent pulses so two triggers (e.g. a Stop hook and the long-command notifier firing at
# once) never interleave openhue calls and restore each other's transient state. Use the KERNEL lock
# /usr/bin/lockf on a held fd: the kernel releases it on ANY exit (normal or crash), so no stale-lock
# class exists and a wedged prior pulse can never suppress every later one. A pulse is short-lived and
# purely cosmetic, so contention simply skips this pulse quietly and exits 0 rather than queueing (the
# most-recent trigger's state is what matters; a dropped duplicate pulse is invisible). Non-darwin hosts
# (no /usr/bin/lockf) proceed unlocked. Absolute path because a stripped PATH may not carry /usr/bin.
# The lockfile is a distinct regular-file anchor (not the old mkdir dir); any leftover dir at the old
# path is harmless TMPDIR cruft that clears on reboot. (House precedent: homebrew-weekly-upgrade.sh and
# update-skills.sh guard with this same kernel-lock shape.)
# Anchored under $HOME, not $TMPDIR: an unset TMPDIR (a stripped launchd or
# hook environment) fell back to the shared sticky /tmp, where another local
# account can pre-create the path and hold or redirect the lock.
lock_dir="$HOME/.local/state"
mkdir -p "$lock_dir" 2>/dev/null || exit 0
lock="$lock_dir/hue-pulse.lockf"
# Clean up only our own temp state on exit; the kernel owns lock release.
trap '[[ -n ${state_file:-} ]] && rm -f "$state_file"; true' EXIT
if [[ -x /usr/bin/lockf ]]; then
  exec 9>>"$lock" 2>/dev/null || exit 0
  /usr/bin/lockf -s -t 0 9 || exit 0
fi

# One room listing, resolved against every configured name. A name that does
# not exist is SKIPPED rather than fatal: a room renamed in the Hue app must
# not take the other rooms' pulse down with it.
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

# Snapshot each light in the room: id, on-state, brightness, color mode, and
# the color value(s), either mirek (color temp) or CIE xy.
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

# Pulse pattern: bright → 20% → bright → 20% → restore.
# Four pulse phases (two full lub-DUB cycles) ending on the LOW phase, so
# the restore is a gentle step from "dim color" back to the user's original
# state, not a jarring drop from peak brightness.
#
# Sleeps match the 1.2s transitions so the bulb fully reaches each target
# (no interrupting overlap). 1.2s is long enough for the ramp itself to
# read as smooth wave motion, while the brief API-roundtrip "settle"
# between transitions is short enough not to feel like a hitch.
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

# Best-effort notifier: a failed pulse must never fail the caller (Stop hook /
# long-command notifier). Any openhue hiccup above is swallowed; exit clean.
exit 0
