#!/usr/bin/env bash
# hue-pulse-lock: two pulses fired at once must serialize (queue) through the lock, never overlap their
# openhue calls. A mock openhue flags any overlap atomically (mkdir); the lock must prevent it and release.
set -uo pipefail
hue="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/dot_local/bin/executable_hue-pulse.sh"
[[ -x $hue ]] || {
  echo "hue-pulse-lock: FAIL -- not executable: $hue" >&2
  exit 1
}
command -v jq >/dev/null 2>&1 || {
  echo "hue-pulse-lock: skipped (no jq)"
  exit 0
}
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# Mock openhue: only the get-room + get-light path is exercised (get-light returns [] so hue-pulse exits
# before the real pulse). get-room enters a critical section guarded by an atomic mkdir; if a second pulse
# is inside it at the same time, the mkdir fails and we record OVERLAP.
cat >"$tmp/openhue" <<MOCK
#!/usr/bin/env bash
if [[ "\$*" == *"get room"* ]]; then
  mkdir "$tmp/section" 2>/dev/null || touch "$tmp/OVERLAP"
  sleep 0.4
  rmdir "$tmp/section" 2>/dev/null || true
  echo '[{"Name":"TEST-ROOM","Id":"room1"}]'
elif [[ "\$*" == *"get light"* ]]; then
  echo '[]'
fi
MOCK
chmod +x "$tmp/openhue"

# Fire two pulses simultaneously (children of THIS shell, so wait actually waits for them).
HUE_PULSE_ROOM=TEST-ROOM TMPDIR="$tmp" PATH="$tmp:$PATH" bash "$hue" 0 &
HUE_PULSE_ROOM=TEST-ROOM TMPDIR="$tmp" PATH="$tmp:$PATH" bash "$hue" 0 &
wait

[[ -e "$tmp/OVERLAP" ]] && {
  echo "hue-pulse-lock: FAIL -- two pulses ran concurrently (lock did not queue them)" >&2
  exit 1
}
[[ -e "$tmp/hue-pulse.lock" ]] && {
  echo "hue-pulse-lock: FAIL -- lock dir leaked (not released on exit)" >&2
  exit 1
}
echo "hue-pulse-lock: OK"
